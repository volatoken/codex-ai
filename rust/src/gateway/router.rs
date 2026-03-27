use anyhow::Result;
use teloxide::prelude::*;
use teloxide::types::{Message, MessageId, ThreadId};
use tracing::info;

use crate::AppState;
use crate::bridge::deerflow::DeerFlowBridge;
use crate::orchestrator::queue::BuildQueue;

/// Helper to get thread_id for replies, defaulting to General topic.
fn reply_thread(msg: &Message) -> ThreadId {
    msg.thread_id.unwrap_or(ThreadId(MessageId(0)))
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
    build_queue: &BuildQueue,
) -> Result<()> {
    bot.send_message(msg.chat.id, "🧠 Received your idea! Sending to DeerFlow for analysis...")
        .message_thread_id(reply_thread(msg))
        .await?;

    let bridge = DeerFlowBridge::new(&state.settings);

    match bridge.process_idea(text, user_id).await {
        Ok(response) => {
            let project_name = response
                .extract_project_name()
                .unwrap_or_else(|| "unnamed".into());
            let summary = response.extract_summary();

            let reply = format!(
                "📋 Plan: {project_name}\n\n{summary}\n\n\
                 Reply /approve to start building or /reject to cancel."
            );
            bot.send_message(msg.chat.id, &reply)
                .message_thread_id(reply_thread(msg))
                .await?;

            // If user approves, enqueue build
            if text.starts_with("/approve") {
                let plan = serde_json::json!({
                    "project_name": project_name,
                    "plan_text": response.answer,
                    "thread_id": response.thread_id,
                });
                build_queue
                    .enqueue(project_name.to_string(), plan)
                    .await?;
                bot.send_message(msg.chat.id, "🚀 Build enqueued!")
                    .message_thread_id(reply_thread(msg))
                    .await?;
            }
        }
        Err(e) => {
            bot.send_message(msg.chat.id, format!("❌ DeerFlow error: {e:#}"))
                .message_thread_id(reply_thread(msg))
                .await?;
        }
    }
    Ok(())
}

/// Handle research requests in #research topic
async fn handle_research(
    bot: Bot,
    msg: &Message,
    text: &str,
    _user_id: i64,
    state: &AppState,
) -> Result<()> {
    bot.send_message(msg.chat.id, "🔍 Sending to DeerFlow for research...")
        .message_thread_id(reply_thread(msg))
        .await?;

    let bridge = DeerFlowBridge::new(&state.settings);

    match bridge.research(text).await {
        Ok(response) => {
            // Telegram message limit is 4096 chars
            let answer = if response.answer.len() > 4000 {
                format!("{}...\n\n(truncated)", &response.answer[..4000])
            } else {
                response.answer
            };
            bot.send_message(msg.chat.id, format!("📝 {answer}"))
                .message_thread_id(reply_thread(msg))
                .await?;
        }
        Err(e) => {
            bot.send_message(msg.chat.id, format!("❌ DeerFlow error: {e:#}"))
                .message_thread_id(reply_thread(msg))
                .await?;
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
    bot.send_message(msg.chat.id, dashboard)
        .message_thread_id(reply_thread(msg))
        .await?;
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

    bot.send_message(msg.chat.id, reply)
        .message_thread_id(reply_thread(msg))
        .await?;
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
