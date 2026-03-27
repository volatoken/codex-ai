use anyhow::Result;
use tracing::{info, warn};

use super::queue::BuildJob;
use super::ram_guard::RamGuard;
use crate::bridge::deerflow::DeerFlowBridge;
use crate::config::Settings;
use crate::supervisor::ProcessSupervisor;

/// Manages build phases using DeerFlow AI backend with RAM awareness.
#[derive(Clone)]
pub struct ParallelBuilder {
    ram_guard: std::sync::Arc<RamGuard>,
    supervisor: ProcessSupervisor,
}

impl ParallelBuilder {
    pub fn new(ram_guard: RamGuard, supervisor: ProcessSupervisor) -> Self {
        Self {
            ram_guard: std::sync::Arc::new(ram_guard),
            supervisor,
        }
    }

    /// Run planning phase via DeerFlow
    pub async fn run_planning(&self, job: &mut BuildJob) -> Result<()> {
        info!("[{}] Phase: Planning via DeerFlow", job.project_name);
        self.ram_guard.wait_for(200).await;

        let settings = Settings::from_env()?;
        let bridge = DeerFlowBridge::new(&settings);

        let response = bridge
            .process_idea(
                &serde_json::to_string(&job.plan).unwrap_or_default(),
                0,
            )
            .await?;

        info!("[{}] Planning complete", job.project_name);

        // Store thread_id for subsequent phases (maintains conversation context)
        job.thread_id = Some(response.thread_id.clone());

        // Save plan to workspace
        let plan_path = format!("workspace/projects/{}/plan.md", job.project_name);
        tokio::fs::create_dir_all(format!("workspace/projects/{}", job.project_name)).await?;
        tokio::fs::write(&plan_path, &response.answer).await?;

        Ok(())
    }

    /// Run coding phase via DeerFlow (uses same thread for context continuity)
    pub async fn run_coding(&self, job: &BuildJob) -> Result<()> {
        info!("[{}] Phase: Coding via DeerFlow", job.project_name);
        self.ram_guard.wait_for(300).await;

        let settings = Settings::from_env()?;
        let bridge = DeerFlowBridge::new(&settings);

        let thread_id = job.thread_id.as_deref().unwrap_or("default");
        let response = bridge
            .generate_code(thread_id, &job.project_name, &job.plan)
            .await?;

        info!("[{}] Coding complete", job.project_name);

        // Extract and save generated files
        let files = response.extract_files();
        if files.is_empty() {
            warn!("[{}] No files extracted from DeerFlow response, saving raw output", job.project_name);
            let raw_path = format!("workspace/projects/{}/deerflow_output.md", job.project_name);
            tokio::fs::write(&raw_path, &response.answer).await?;
        } else {
            for (filename, content) in &files {
                let file_path = format!("workspace/projects/{}/{}", job.project_name, filename);
                if let Some(dir) = std::path::Path::new(&file_path).parent() {
                    tokio::fs::create_dir_all(dir).await?;
                }
                tokio::fs::write(&file_path, content).await?;
                info!("[{}] Written: {}", job.project_name, filename);
            }
        }

        Ok(())
    }

    /// Run testing/review phase via DeerFlow (self-fix loop)
    pub async fn run_testing(&self, job: &BuildJob) -> Result<()> {
        info!("[{}] Phase: Testing via DeerFlow", job.project_name);
        self.ram_guard.wait_for(200).await;

        let settings = Settings::from_env()?;
        let bridge = DeerFlowBridge::new(&settings);
        let thread_id = job.thread_id.as_deref().unwrap_or("default");

        // DeerFlow has built-in self-fix loops — it will review, test, and fix
        let response = bridge.review_and_test(thread_id, &job.project_name).await?;

        if !response.test_passed() {
            // Save the review feedback
            let review_path = format!("workspace/projects/{}/review.md", job.project_name);
            tokio::fs::write(&review_path, &response.answer).await?;
            anyhow::bail!(
                "DeerFlow review failed for {}: see review.md for details",
                job.project_name
            );
        }

        // Apply any corrected files
        let fixes = response.extract_files();
        for (filename, content) in &fixes {
            let file_path = format!("workspace/projects/{}/{}", job.project_name, filename);
            if let Some(dir) = std::path::Path::new(&file_path).parent() {
                tokio::fs::create_dir_all(dir).await?;
            }
            tokio::fs::write(&file_path, content).await?;
            info!("[{}] Fixed: {}", job.project_name, filename);
        }

        info!("[{}] Tests passed", job.project_name);
        Ok(())
    }

    /// Run Docker build (heavy — RAM gated)
    pub async fn run_docker_build(&self, job: &BuildJob) -> Result<()> {
        info!("[{}] Phase: Docker Build", job.project_name);
        self.ram_guard.wait_for(1024).await;

        let project_dir = format!("workspace/projects/{}", job.project_name);
        let image_tag = format!("codex-tool-{}", job.project_name);

        let output = tokio::process::Command::new("docker")
            .args(["build", "-t", &image_tag, "."])
            .current_dir(&project_dir)
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Docker build failed: {stderr}");
        }

        info!("[{}] Docker image built: {image_tag}", job.project_name);
        Ok(())
    }

    /// Deploy the built tool
    pub async fn run_deploy(&self, job: &BuildJob) -> Result<()> {
        info!("[{}] Phase: Deploy", job.project_name);

        let image_tag = format!("codex-tool-{}", job.project_name);
        let container_name = format!("codex-{}", job.project_name);

        // Stop existing container if any
        let _ = tokio::process::Command::new("docker")
            .args(["rm", "-f", &container_name])
            .output()
            .await;

        // Run new container
        let output = tokio::process::Command::new("docker")
            .args([
                "run",
                "-d",
                "--name",
                &container_name,
                "--restart",
                "unless-stopped",
                "--memory",
                "512m",
                &image_tag,
            ])
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Deploy failed: {stderr}");
        }

        let container_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
        self.supervisor
            .register(job.project_name.clone(), container_id)
            .await;

        info!("[{}] Deployed successfully", job.project_name);
        Ok(())
    }
}
