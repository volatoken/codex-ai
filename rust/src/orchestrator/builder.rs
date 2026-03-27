use anyhow::Result;
use tracing::info;

use super::queue::BuildJob;
use super::ram_guard::RamGuard;
use crate::bridge::python::PythonBridge;
use crate::config::Settings;
use crate::supervisor::ProcessSupervisor;

/// Manages parallel execution of build phases with RAM awareness.
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

    /// Run planning phase via Python worker
    pub async fn run_planning(&self, job: &BuildJob) -> Result<()> {
        info!("[{}] Phase: Planning", job.project_name);
        // RAM check: planning needs ~200MB
        self.ram_guard.wait_for(200).await;

        let settings = Settings::from_env()?;
        let bridge = PythonBridge::new(&settings);
        let request = serde_json::json!({
            "action": "plan",
            "payload": {
                "project_name": job.project_name,
                "plan": job.plan,
            }
        });

        let response = bridge.call(request).await?;
        info!("[{}] Planning complete", job.project_name);

        // Save plan to workspace
        let plan_path = format!("workspace/projects/{}/plan.json", job.project_name);
        tokio::fs::create_dir_all(format!("workspace/projects/{}", job.project_name)).await?;
        tokio::fs::write(&plan_path, serde_json::to_string_pretty(&response)?).await?;

        Ok(())
    }

    /// Run coding phase via Python worker
    pub async fn run_coding(&self, job: &BuildJob) -> Result<()> {
        info!("[{}] Phase: Coding", job.project_name);
        self.ram_guard.wait_for(300).await;

        let settings = Settings::from_env()?;
        let bridge = PythonBridge::new(&settings);
        let request = serde_json::json!({
            "action": "code",
            "payload": {
                "project_name": job.project_name,
                "plan": job.plan,
            }
        });

        let response = bridge.call(request).await?;
        info!("[{}] Coding complete", job.project_name);

        // Save generated code
        if let Some(files) = response["result"]["files"].as_object() {
            for (filename, content) in files {
                let file_path = format!(
                    "workspace/projects/{}/src/{}",
                    job.project_name, filename
                );
                if let Some(dir) = std::path::Path::new(&file_path).parent() {
                    tokio::fs::create_dir_all(dir).await?;
                }
                if let Some(code) = content.as_str() {
                    tokio::fs::write(&file_path, code).await?;
                }
            }
        }

        Ok(())
    }

    /// Run testing phase via Python worker
    pub async fn run_testing(&self, job: &BuildJob) -> Result<()> {
        info!("[{}] Phase: Testing", job.project_name);
        self.ram_guard.wait_for(200).await;

        let settings = Settings::from_env()?;
        let bridge = PythonBridge::new(&settings);
        let request = serde_json::json!({
            "action": "test",
            "payload": {
                "project_name": job.project_name,
            }
        });

        let response = bridge.call(request).await?;
        let passed = response["result"]["passed"].as_bool().unwrap_or(false);

        if !passed {
            anyhow::bail!(
                "Tests failed for {}: {}",
                job.project_name,
                response["result"]["error"].as_str().unwrap_or("unknown")
            );
        }

        info!("[{}] Tests passed", job.project_name);
        Ok(())
    }

    /// Run Docker build (heavy — RAM gated)
    pub async fn run_docker_build(&self, job: &BuildJob) -> Result<()> {
        info!("[{}] Phase: Docker Build", job.project_name);
        // Docker build needs ~1GB
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
