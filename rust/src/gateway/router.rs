use anyhow::Result;
use teloxide::prelude::*;
use teloxide::types::{ChatAction, ChatId, InputFile, Message, MessageId, ThreadId};
use tracing::{info, warn};
use std::io::Write;

use crate::AppState;
use crate::bridge::deerflow::DeerFlowBridge;
use crate::config::Settings;
use crate::orchestrator::queue::BuildQueue;

/// Write a line to the debug log file (bypasses tracing buffering).
fn debug_log(msg: &str) {
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("logs/gateway-debug.log")
    {
        let ts = chrono::Utc::now().format("%H:%M:%S%.3f");
        let _ = writeln!(f, "[{ts}] {msg}");
    }
}

/// Helper to get thread_id for replies. Returns None for General topic.
fn reply_thread(msg: &Message) -> Option<ThreadId> {
    msg.thread_id
}

/// Route incoming messages based on which forum topic they arrived in.
pub async fn route_message(
    bot: Bot,
    msg: &Message,
    text: &str,
    user_id: i64,
    thread_id: Option<i32>,
    state: &AppState,
    build_queue: &BuildQueue,
) -> Result<()> {
    let topics = &state.topic_manager;

    // Detect topic by thread_id
    let topic_name = thread_id
        .and_then(|tid| topics.topic_name(tid))
        .unwrap_or_else(|| "general".into());

    match topic_name.as_str() {
        "ideas" => handle_idea(bot, msg, text, user_id, state, build_queue).await,
        "research" => handle_research(bot, msg, text, user_id, state).await,
        "dashboard" => handle_dashboard(bot, msg, text, state).await,
        "tool-management" => handle_tool_management(bot, msg, text, state).await,
        name if name.starts_with("tool-") => {
            handle_tool_topic(bot, msg, text, name, state).await
        }
        "general" | "agent-logs" => {
            handle_general(bot, msg, text, state).await
        }
        _ => {
            info!("Unrouted message in topic: {topic_name}");
            Ok(())
        }
    }
}

/// Handle new idea submission in #ideas topic
async fn handle_idea(
    bot: Bot,
    msg: &Message,
    text: &str,
    user_id: i64,
    state: &AppState,
    _build_queue: &BuildQueue,
) -> Result<()> {
    let trimmed = text.trim();

    // Handle /approve command — triggers build for the user's pending idea
    if trimmed.starts_with("/approve") {
        return handle_approve(bot, msg, user_id, state).await;
    }

    // Handle /reject command — cancels pending idea
    if trimmed.starts_with("/reject") {
        state.pending_ideas.lock().await.remove(&user_id);
        send_reply(&bot, msg, "❌ Ý tưởng đã bị hủy.").await?;
        return Ok(());
    }

    // Process new idea — generate a build plan
    let placeholder_id = send_typing_placeholder(&bot, msg, "🧠 Đang phân tích ý tưởng...").await?;

    let bridge = DeerFlowBridge::new(&state.settings);

    match bridge.plan_idea_fast(text).await {
        Ok(response) => {
            let project_name = response
                .extract_project_name()
                .unwrap_or_else(|| "unnamed-tool".into());
            let summary = response.extract_summary();

            // Store pending idea for this user
            state.pending_ideas.lock().await.insert(
                user_id,
                crate::PendingIdea {
                    project_name: project_name.clone(),
                    plan_text: response.answer.clone(),
                    idea_text: text.to_string(),
                    user_id,
                },
            );

            let reply = format!(
                "📋 Plan: {project_name}\n\n\
                 {summary}\n\n\
                 Gửi /approve để bắt đầu build hoặc /reject để hủy."
            );
            let _ = edit_message(&bot, msg, placeholder_id, &reply).await;
        }
        Err(e) => {
            let _ = edit_message(
                &bot,
                msg,
                placeholder_id,
                &format!("❌ Lỗi phân tích: {e:#}"),
            )
            .await;
        }
    }
    Ok(())
}

/// Handle /approve — start building the user's pending idea
async fn handle_approve(
    bot: Bot,
    msg: &Message,
    user_id: i64,
    state: &AppState,
) -> Result<()> {
    let idea = state.pending_ideas.lock().await.remove(&user_id);

    let Some(idea) = idea else {
        send_reply(
            &bot,
            msg,
            "⚠️ Không có ý tưởng nào đang chờ. Gửi ý tưởng trước rồi /approve.",
        )
        .await?;
        return Ok(());
    };

    let placeholder_id = send_typing_placeholder(
        &bot,
        msg,
        &format!("🚀 Bắt đầu build {}...", idea.project_name),
    )
    .await?;

    // Spawn background build task
    let bot2 = bot.clone();
    let chat_id = msg.chat.id;
    let thread_id = reply_thread(msg);
    let settings = state.settings.clone();
    let project_name = idea.project_name.clone();
    let plan_text = idea.plan_text.clone();

    tokio::spawn(async move {
        match run_build_and_send(
            bot2.clone(),
            chat_id,
            thread_id,
            placeholder_id,
            settings,
            project_name,
            plan_text,
        )
        .await
        {
            Ok(()) => {}
            Err(e) => {
                warn!("Build failed: {e:#}");
                let _ = bot2
                    .edit_message_text(
                        chat_id,
                        placeholder_id,
                        format!("❌ Build thất bại: {e:#}"),
                    )
                    .await;
            }
        }
    });

    Ok(())
}

/// Execute the full build pipeline: generate code → save files → package → send via Telegram
async fn run_build_and_send(
    bot: Bot,
    chat_id: ChatId,
    thread_id: Option<ThreadId>,
    placeholder_id: MessageId,
    settings: Settings,
    project_name: String,
    plan_text: String,
) -> Result<()> {
    let bridge = DeerFlowBridge::new(&settings);
    let project_dir = format!("workspace/projects/{}", project_name);

    // Phase 1: Generate code
    notify(&bot, chat_id, placeholder_id, &format!(
        "📝 [{project_name}] Phase 1/3: Đang tạo code..."
    )).await;

    let response = bridge.build_generate(&project_name, &plan_text).await?;

    let files = response.extract_files();
    if files.is_empty() {
        anyhow::bail!(
            "AI không tạo được file code nào. Thử gửi ý tưởng chi tiết hơn."
        );
    }

    // Save files to workspace
    tokio::fs::create_dir_all(&project_dir).await?;
    for (filename, content) in &files {
        let file_path = format!("{}/{}", project_dir, filename);
        if let Some(dir) = std::path::Path::new(&file_path).parent() {
            tokio::fs::create_dir_all(dir).await?;
        }
        tokio::fs::write(&file_path, content).await?;
        info!("[{project_name}] Saved: {filename}");
    }

    // Phase 2: Package into zip
    notify(&bot, chat_id, placeholder_id, &format!(
        "📦 [{project_name}] Phase 2/3: Đang đóng gói ({} files)...",
        files.len()
    )).await;

    let zip_path = format!("workspace/projects/{}.zip", project_name);

    // Use PowerShell Compress-Archive on Windows
    let ps_src = project_dir.replace('/', "\\");
    let ps_dst = zip_path.replace('/', "\\");
    let output = tokio::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "if (Test-Path '{}') {{ Remove-Item '{}' -Force }}; Compress-Archive -Path '{}\\*' -DestinationPath '{}'",
                ps_dst, ps_dst, ps_src, ps_dst
            ),
        ])
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Đóng gói zip thất bại: {stderr}");
    }

    // Phase 3: Try PyInstaller for Python projects (optional, graceful fallback)
    let has_python = files.iter().any(|(name, _)| name.ends_with(".py"));
    let mut exe_path: Option<String> = None;

    if has_python {
        notify(&bot, chat_id, placeholder_id, &format!(
            "🔨 [{project_name}] Phase 3/3: Đang build .exe..."
        )).await;

        // Find main entry point
        let main_file = files
            .iter()
            .find(|(name, _)| name == "main.py" || name.ends_with("/main.py"))
            .or_else(|| files.iter().find(|(name, _)| name.ends_with(".py")))
            .map(|(name, _)| name.clone());

        if let Some(main_file) = main_file {
            let main_path = format!("{}/{}", project_dir, main_file);
            let dist_dir = format!("{}/dist", project_dir);

            // Install deps first if requirements.txt exists
            let req_path = format!("{}/requirements.txt", project_dir);
            if tokio::fs::metadata(&req_path).await.is_ok() {
                let _ = tokio::process::Command::new("pip")
                    .args(["install", "-r", &req_path, "-q"])
                    .output()
                    .await;
            }

            let exe_result = tokio::process::Command::new("pyinstaller")
                .args([
                    "--onefile",
                    "--clean",
                    "--distpath",
                    &dist_dir,
                    "--workpath",
                    &format!("{}/build", project_dir),
                    "--specpath",
                    &project_dir,
                    &main_path,
                ])
                .output()
                .await;

            match exe_result {
                Ok(o) if o.status.success() => {
                    // Find the generated .exe
                    let exe_name = std::path::Path::new(&main_file)
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string()
                        + ".exe";
                    let exe_full = format!("{}/{}", dist_dir, exe_name);
                    if tokio::fs::metadata(&exe_full).await.is_ok() {
                        exe_path = Some(exe_full);
                        info!("[{project_name}] PyInstaller .exe created: {exe_name}");
                    }
                }
                Ok(o) => {
                    let stderr = String::from_utf8_lossy(&o.stderr);
                    info!("[{project_name}] PyInstaller failed (sending zip instead): {}", stderr.chars().take(200).collect::<String>());
                }
                Err(e) => {
                    info!("[{project_name}] PyInstaller not available (sending zip instead): {e}");
                }
            }
        }
    } else {
        notify(&bot, chat_id, placeholder_id, &format!(
            "📦 [{project_name}] Phase 3/3: Hoàn tất đóng gói..."
        )).await;
    }

    // Send files to Telegram
    notify(&bot, chat_id, placeholder_id, &format!(
        "✅ [{project_name}] Build xong! Đang gửi file..."
    )).await;

    // Send .exe if available
    if let Some(ref exe) = exe_path {
        let caption = format!("🛠 {}.exe — Tool hoàn chỉnh", project_name);
        let mut req = bot.send_document(chat_id, InputFile::file(exe));
        req = req.caption(&caption);
        if let Some(tid) = thread_id {
            req = req.message_thread_id(tid);
        }
        req.await?;
    }

    // Always send the zip (source code)
    let zip_caption = if exe_path.is_some() {
        format!("📦 {}.zip — Source code đầy đủ", project_name)
    } else {
        format!("📦 {}.zip — Source code (chạy python main.py)", project_name)
    };
    let mut req = bot.send_document(chat_id, InputFile::file(&zip_path));
    req = req.caption(&zip_caption);
    if let Some(tid) = thread_id {
        req = req.message_thread_id(tid);
    }
    req.await?;

    // Final summary
    let file_list: String = files
        .iter()
        .map(|(name, _)| format!("  📄 {name}"))
        .collect::<Vec<_>>()
        .join("\n");
    let exe_note = if exe_path.is_some() {
        "🛠 File .exe đã được tạo bằng PyInstaller"
    } else if has_python {
        "📦 Gửi source code .zip (PyInstaller không khả dụng)"
    } else {
        "📦 Gửi source code .zip"
    };
    let final_msg = format!(
        "✅ {project_name} build thành công!\n\n\
         📁 Files ({} files):\n{file_list}\n\n\
         {exe_note}",
        files.len()
    );
    let _ = bot
        .edit_message_text(chat_id, placeholder_id, &final_msg)
        .await;

    info!("[{project_name}] Build pipeline complete — {} files sent", files.len());
    Ok(())
}

/// Edit a placeholder message from a background task (uses ChatId directly)
async fn notify(bot: &Bot, chat_id: ChatId, message_id: MessageId, text: &str) {
    let _ = bot.edit_message_text(chat_id, message_id, text).await;
}

/// Handle research requests in #research topic
async fn handle_research(
    bot: Bot,
    msg: &Message,
    text: &str,
    _user_id: i64,
    state: &AppState,
) -> Result<()> {
    let placeholder_id = send_typing_placeholder(&bot, msg, "🔍 Đang tìm kiếm...").await?;

    let bridge = DeerFlowBridge::new(&state.settings);

    match bridge.research(text).await {
        Ok(response) => {
            let answer = if response.answer.len() > 4000 {
                format!("📝 {}...\n\n(truncated)", &response.answer[..4000])
            } else {
                format!("📝 {}", response.answer)
            };
            let _ = edit_message(&bot, msg, placeholder_id, &answer).await;
        }
        Err(e) => {
            let _ = edit_message(&bot, msg, placeholder_id, &format!("❌ Lỗi: {e:#}")).await;
        }
    }
    Ok(())
}

/// Handle dashboard requests
async fn handle_dashboard(
    bot: Bot,
    msg: &Message,
    _text: &str,
    _state: &AppState,
) -> Result<()> {
    let ram_info = {
        let mut sys = sysinfo::System::new();
        sys.refresh_memory();
        let used = sys.used_memory() / 1024 / 1024;
        let total = sys.total_memory() / 1024 / 1024;
        format!("{used}MB / {total}MB")
    };

    let dashboard = format!(
        "📊 **Codex AI Dashboard**\n\
         ━━━━━━━━━━━━━━━━━━━━\n\
         🖥 RAM: {ram_info}\n\
         🛠 Running tools: 0\n\
         📋 Queue: 0 pending\n\
         ✅ Completed: 0",
    );
    send_reply(&bot, msg, &dashboard).await?;
    Ok(())
}

/// Handle tool management commands
async fn handle_tool_management(
    bot: Bot,
    msg: &Message,
    text: &str,
    _state: &AppState,
) -> Result<()> {
    let reply = match text.trim() {
        "/list" => "🛠 No tools deployed yet.".to_string(),
        cmd if cmd.starts_with("/stop ") => {
            let name = cmd.strip_prefix("/stop ").unwrap_or("").trim();
            format!("⏹ Stopping tool: {name}")
        }
        cmd if cmd.starts_with("/restart ") => {
            let name = cmd.strip_prefix("/restart ").unwrap_or("").trim();
            format!("🔄 Restarting tool: {name}")
        }
        cmd if cmd.starts_with("/logs ") => {
            let name = cmd.strip_prefix("/logs ").unwrap_or("").trim();
            format!("📜 Fetching logs for: {name}")
        }
        _ => "Commands: /list, /stop <name>, /restart <name>, /logs <name>".to_string(),
    };

    send_reply(&bot, msg, &reply).await?;
    Ok(())
}

/// Send typing action + placeholder message. Returns the placeholder MessageId for later editing.
async fn send_typing_placeholder(bot: &Bot, msg: &Message, placeholder: &str) -> Result<MessageId> {
    // Send "typing..." action for visual feedback
    let _ = bot.send_chat_action(msg.chat.id, ChatAction::Typing).await;
    // Send placeholder message
    let mut req = bot.send_message(msg.chat.id, placeholder);
    if let Some(tid) = reply_thread(msg) {
        req = req.message_thread_id(tid);
    }
    let sent = req.await?;
    Ok(sent.id)
}

/// Edit an existing message with new text.
async fn edit_message(bot: &Bot, msg: &Message, message_id: MessageId, text: &str) -> Result<()> {
    bot.edit_message_text(msg.chat.id, message_id, text).await?;
    Ok(())
}

/// Helper: send a message, optionally in a thread.
async fn send_reply(bot: &Bot, msg: &Message, text: &str) -> Result<()> {
    let mut req = bot.send_message(msg.chat.id, text);
    if let Some(tid) = reply_thread(msg) {
        req = req.message_thread_id(tid);
    }
    req.await?;
    Ok(())
}

/// Handle messages in general / unassigned topics — use fast LLM for quick responses.
async fn handle_general(
    bot: Bot,
    msg: &Message,
    text: &str,
    state: &AppState,
) -> Result<()> {
    debug_log(&format!("handle_general START: {}", text.chars().take(80).collect::<String>()));
    info!("handle_general: calling chat_fast for: {}", text.chars().take(80).collect::<String>());

    // Send immediate typing indicator + placeholder
    let placeholder_id = send_typing_placeholder(&bot, msg, "⏳ Đang xử lý...").await?;

    let bridge = DeerFlowBridge::new(&state.settings);

    match bridge.chat_fast(text).await {
        Ok(response) => {
            debug_log(&format!("handle_general OK: {} chars", response.answer.len()));
            info!("handle_general: got response ({} chars)", response.answer.len());
            let answer = if response.answer.len() > 4000 {
                format!("{}...\n\n(truncated)", &response.answer[..4000])
            } else {
                response.answer
            };
            if !answer.is_empty() {
                // Edit the placeholder with the actual response
                match edit_message(&bot, msg, placeholder_id, &answer).await {
                    Ok(_) => debug_log("handle_general: EDITED placeholder with response"),
                    Err(e) => {
                        debug_log(&format!("handle_general: EDIT ERROR: {e:#}, sending new message"));
                        // Fallback: send as new message if edit fails
                        let _ = send_reply(&bot, msg, &answer).await;
                    }
                }
            } else {
                warn!("handle_general: empty response from LLM");
                debug_log("handle_general: EMPTY response");
                let _ = edit_message(&bot, msg, placeholder_id, "🤖 Không có phản hồi từ AI.").await;
            }
        }
        Err(e) => {
            debug_log(&format!("handle_general ERROR: {e:#}"));
            warn!("handle_general: chat_fast error: {e:#}");
            let _ = edit_message(&bot, msg, placeholder_id, &format!("❌ Lỗi: {e:#}")).await;
        }
    }
    Ok(())
}

/// Handle messages in tool-specific topics
async fn handle_tool_topic(
    _bot: Bot,
    _msg: &Message,
    text: &str,
    topic_name: &str,
    _state: &AppState,
) -> Result<()> {
    let tool_name = topic_name.strip_prefix("tool-").unwrap_or(topic_name);
    info!("Message in tool topic '{tool_name}': {text}");
    Ok(())
}
