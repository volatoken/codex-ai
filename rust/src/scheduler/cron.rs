use anyhow::Result;
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::info;

use crate::AppState;

/// Run the cron scheduler for periodic tasks.
pub async fn run_scheduler(state: AppState) -> Result<()> {
    let scheduler = JobScheduler::new().await?;

    // Health check every 5 minutes
    let state_clone = state.clone();
    scheduler
        .add(Job::new_async("0 */5 * * * *", move |_uuid, _lock| {
            let _state = state_clone.clone();
            Box::pin(async move {
                info!("Health check tick");
                // Check running containers, report to dashboard
            })
        })?)
        .await?;

    // RAM report every 15 minutes
    scheduler
        .add(Job::new_async("0 */15 * * * *", |_uuid, _lock| {
            Box::pin(async {
                let mut sys = sysinfo::System::new();
                sys.refresh_memory();
                let used = sys.used_memory() / 1024 / 1024;
                let total = sys.total_memory() / 1024 / 1024;
                info!("RAM usage: {used}MB / {total}MB");
            })
        })?)
        .await?;

    scheduler.start().await?;
    info!("Scheduler started");

    // Keep scheduler alive
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
    }
}
