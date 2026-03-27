pub mod cron;

use anyhow::Result;
use crate::AppState;

pub async fn start(state: AppState) -> Result<tokio::task::JoinHandle<()>> {
    let handle = tokio::spawn(async move {
        if let Err(e) = cron::run_scheduler(state).await {
            tracing::error!("Scheduler error: {e:#}");
        }
    });
    Ok(handle)
}
