use anyhow::Result;
use teloxide::prelude::*;
use tracing::info;

use crate::AppState;
use crate::orchestrator::queue::BuildQueue;
use super::router;

pub async fn run(state: AppState, build_queue: BuildQueue) -> Result<()> {
    let bot = Bot::new(&state.settings.telegram_bot_token);

    let handler = Update::filter_message().endpoint(
        move |bot: Bot, msg: Message, state: AppState, bq: BuildQueue| async move {
            if let Err(e) = handle_message(bot.clone(), msg, &state, &bq).await {
                tracing::error!("Error handling message: {e:#}");
            }
            respond(())
        },
    );

    info!("Bot is running. Listening for messages...");

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![state, build_queue])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    Ok(())
}

async fn handle_message(
    bot: Bot,
    msg: Message,
    state: &AppState,
    build_queue: &BuildQueue,
) -> Result<()> {
    // Only process messages in the configured group
    if msg.chat.id.0 != state.settings.telegram_group_id {
        return Ok(());
    }

    // Extract thread/topic id (convert ThreadId(MessageId(i32)) to i32)
    let thread_id = msg.thread_id.map(|t| t.0.0);
    let text = match msg.text() {
        Some(t) => t.to_string(),
        None => return Ok(()),
    };

    let user_id = msg
        .from
        .as_ref()
        .map(|u| u.id.0 as i64)
        .unwrap_or(0);

    // File-based debug log (bypasses tracing buffering)
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open("logs/gateway-debug.log") {
        use std::io::Write;
        let ts = chrono::Utc::now().format("%H:%M:%S%.3f");
        let _ = writeln!(f, "[{ts}] MSG user={user_id} topic={thread_id:?}: {}", text.chars().take(80).collect::<String>());
    }

    info!(
        "Message from user {user_id} in topic {:?}: {}",
        thread_id,
        text.chars().take(80).collect::<String>()
    );

    router::route_message(bot, &msg, &text, user_id, thread_id, state, build_queue).await
}
