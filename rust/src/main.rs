use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn};

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
    info!("Codex AI starting... (Rust Gateway + DeerFlow Backend)");
    info!("RAM limit: {}MB", settings.total_ram_mb);
    info!("DeerFlow URL: {}", settings.deerflow_url);

    // Check DeerFlow backend connectivity
    let df_bridge = bridge::deerflow::DeerFlowBridge::new(&settings);
    match df_bridge.health_check().await {
        Ok(true) => info!("DeerFlow backend is healthy"),
        Ok(false) => warn!("DeerFlow backend returned unhealthy — builds will fail until it's up"),
        Err(e) => warn!("Cannot reach DeerFlow backend: {e} — make sure it's running"),
    }

    // Initialize shared state
    let state = AppState::new(settings).await?;

    // Ensure system forum topics exist (creates them if missing)
    if let Err(e) = state.topic_manager.ensure_system_topics().await {
        warn!("Failed to ensure system topics: {e:#}");
    }

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
    pub pending_ideas: Arc<Mutex<HashMap<i64, PendingIdea>>>,
}

/// An idea awaiting /approve from the user.
#[derive(Clone, Debug)]
pub struct PendingIdea {
    pub project_name: String,
    pub plan_text: String,
    pub idea_text: String,
    pub user_id: i64,
}

impl AppState {
    async fn new(settings: config::Settings) -> Result<Self> {
        let topic_manager = gateway::topics::TopicManager::new(
            settings.telegram_bot_token.clone(),
            settings.telegram_group_id,
        );
        Ok(Self {
            settings,
            topic_manager,
            pending_ideas: Arc::new(Mutex::new(HashMap::new())),
        })
    }
}
