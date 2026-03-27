use anyhow::Result;
use tracing::info;

mod config;
mod gateway;
mod orchestrator;
mod bridge;
mod scheduler;
mod supervisor;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "codex_ai=info".into()),
        )
        .init();

    dotenvy::dotenv().ok();

    let settings = config::Settings::from_env()?;
    info!("Codex AI starting...");
    info!("RAM limit: {}MB", settings.total_ram_mb);

    // Initialize shared state
    let state = AppState::new(settings).await?;

    // Start subsystems
    let ram_guard = orchestrator::ram_guard::RamGuard::new(state.settings.total_ram_mb);
    info!("RAM Guard initialized — available: {}MB", ram_guard.available_mb());

    let supervisor = supervisor::ProcessSupervisor::new();
    let build_queue = orchestrator::queue::BuildQueue::new(ram_guard, supervisor.clone());
    let scheduler_handle = scheduler::start(state.clone()).await?;

    info!("Starting Telegram bot...");
    gateway::bot::run(state, build_queue).await?;

    scheduler_handle.abort();
    Ok(())
}

#[derive(Clone)]
pub struct AppState {
    pub settings: config::Settings,
    pub topic_manager: gateway::topics::TopicManager,
}

impl AppState {
    async fn new(settings: config::Settings) -> Result<Self> {
        let topic_manager = gateway::topics::TopicManager::new(
            settings.telegram_bot_token.clone(),
            settings.telegram_group_id,
        );
        Ok(Self { settings, topic_manager })
    }
}
