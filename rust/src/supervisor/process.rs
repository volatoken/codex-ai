use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

#[derive(Clone, Debug)]
pub struct ToolProcess {
    pub name: String,
    pub container_id: String,
    pub started_at: chrono::DateTime<chrono::Utc>,
}

/// Supervises running tool processes/containers.
#[derive(Clone)]
pub struct ProcessSupervisor {
    processes: Arc<RwLock<HashMap<String, ToolProcess>>>,
}

impl ProcessSupervisor {
    pub fn new() -> Self {
        Self {
            processes: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a new running tool.
    pub async fn register(&self, name: String, container_id: String) {
        let process = ToolProcess {
            name: name.clone(),
            container_id,
            started_at: chrono::Utc::now(),
        };
        self.processes.write().await.insert(name.clone(), process);
        info!("Supervisor: registered tool '{name}'");
    }

    /// Stop a running tool.
    pub async fn stop(&self, name: &str) -> anyhow::Result<()> {
        let process = self
            .processes
            .read()
            .await
            .get(name)
            .cloned();

        if let Some(proc) = process {
            let output = tokio::process::Command::new("docker")
                .args(["stop", &proc.container_id])
                .output()
                .await?;

            if output.status.success() {
                self.processes.write().await.remove(name);
                info!("Supervisor: stopped tool '{name}'");
            } else {
                warn!("Supervisor: failed to stop '{name}'");
            }
        }
        Ok(())
    }

    /// Restart a running tool.
    pub async fn restart(&self, name: &str) -> anyhow::Result<()> {
        let process = self
            .processes
            .read()
            .await
            .get(name)
            .cloned();

        if let Some(proc) = process {
            let output = tokio::process::Command::new("docker")
                .args(["restart", &proc.container_id])
                .output()
                .await?;

            if !output.status.success() {
                warn!("Supervisor: failed to restart '{name}'");
            } else {
                info!("Supervisor: restarted tool '{name}'");
            }
        }
        Ok(())
    }

    /// Get logs from a running tool.
    pub async fn logs(&self, name: &str, lines: u32) -> anyhow::Result<String> {
        let process = self
            .processes
            .read()
            .await
            .get(name)
            .cloned();

        if let Some(proc) = process {
            let output = tokio::process::Command::new("docker")
                .args(["logs", "--tail", &lines.to_string(), &proc.container_id])
                .output()
                .await?;
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Ok(format!("Tool '{name}' not found"))
        }
    }

    /// List all running tools.
    pub async fn list(&self) -> Vec<ToolProcess> {
        self.processes.read().await.values().cloned().collect()
    }

    /// Check health of all running tools.
    pub async fn health_check(&self) {
        let processes = self.processes.read().await.clone();
        for (name, proc) in &processes {
            let output = tokio::process::Command::new("docker")
                .args(["inspect", "--format", "{{.State.Running}}", &proc.container_id])
                .output()
                .await;

            match output {
                Ok(o) => {
                    let running = String::from_utf8_lossy(&o.stdout)
                        .trim()
                        .to_string();
                    if running != "true" {
                        warn!("Tool '{name}' is not running! Attempting restart...");
                        let _ = self.restart(name).await;
                    }
                }
                Err(e) => warn!("Health check failed for '{name}': {e}"),
            }
        }
    }
}
