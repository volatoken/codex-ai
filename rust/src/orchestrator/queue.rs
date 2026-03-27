use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::{Mutex, Semaphore};
use tracing::info;

use super::builder::ParallelBuilder;
use super::ram_guard::RamGuard;
use crate::supervisor::ProcessSupervisor;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BuildJob {
    pub project_name: String,
    pub plan: serde_json::Value,
    pub status: JobStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum JobStatus {
    Queued,
    Planning,
    Coding,
    Testing,
    Building,
    Deploying,
    Running,
    Failed(String),
}

#[derive(Clone)]
pub struct BuildQueue {
    queue: Arc<Mutex<VecDeque<BuildJob>>>,
    builder: ParallelBuilder,
    /// Only 1 Docker build at a time
    docker_semaphore: Arc<Semaphore>,
}

impl BuildQueue {
    pub fn new(ram_guard: RamGuard, supervisor: ProcessSupervisor) -> Self {
        let builder = ParallelBuilder::new(ram_guard, supervisor);
        Self {
            queue: Arc::new(Mutex::new(VecDeque::new())),
            builder,
            docker_semaphore: Arc::new(Semaphore::new(1)),
        }
    }

    /// Add a new job to the queue.
    pub async fn enqueue(&self, project_name: String, plan: serde_json::Value) -> Result<()> {
        let job = BuildJob {
            project_name: project_name.clone(),
            plan,
            status: JobStatus::Queued,
        };
        self.queue.lock().await.push_back(job);
        info!("Build enqueued: {project_name}");

        // Spawn processing in background
        let queue = self.queue.clone();
        let builder = self.builder.clone();
        let sem = self.docker_semaphore.clone();

        tokio::spawn(async move {
            if let Err(e) = process_next(queue, builder, sem).await {
                tracing::error!("Build failed: {e:#}");
            }
        });

        Ok(())
    }

    /// Get list of all jobs and their statuses.
    pub async fn list_jobs(&self) -> Vec<BuildJob> {
        self.queue.lock().await.iter().cloned().collect()
    }
}

async fn process_next(
    queue: Arc<Mutex<VecDeque<BuildJob>>>,
    builder: ParallelBuilder,
    docker_sem: Arc<Semaphore>,
) -> Result<()> {
    let job = {
        let mut q = queue.lock().await;
        q.pop_front()
    };

    let Some(mut job) = job else {
        return Ok(());
    };

    info!("Processing build: {}", job.project_name);

    // Phase 1: Planning + Coding (via Python workers)
    job.status = JobStatus::Planning;
    builder.run_planning(&job).await?;

    job.status = JobStatus::Coding;
    builder.run_coding(&job).await?;

    // Phase 2: Testing
    job.status = JobStatus::Testing;
    builder.run_testing(&job).await?;

    // Phase 3: Docker build (semaphore-gated)
    job.status = JobStatus::Building;
    let _permit = docker_sem.acquire().await?;
    builder.run_docker_build(&job).await?;
    drop(_permit);

    // Phase 4: Deploy
    job.status = JobStatus::Deploying;
    builder.run_deploy(&job).await?;

    job.status = JobStatus::Running;
    info!("Build complete: {}", job.project_name);

    Ok(())
}
