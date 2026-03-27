use anyhow::{Context, Result};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tracing::{debug, error, info};

use crate::config::Settings;

/// Bridge to Python worker processes.
/// Spawns a Python subprocess, sends JSON on stdin, reads JSON from stdout.
pub struct PythonBridge {
    python_bin: String,
    worker_script: String,
    env_vars: Vec<(String, String)>,
}

impl PythonBridge {
    pub fn new(settings: &Settings) -> Self {
        Self {
            python_bin: settings.python_bin.clone(),
            worker_script: "python/src/worker.py".into(),
            env_vars: vec![
                ("LLM_PROVIDER".into(), settings.llm_provider.clone()),
                ("LLM_API_KEY".into(), settings.llm_api_key.clone()),
                ("LLM_BASE_URL".into(), settings.llm_base_url.clone()),
                ("LLM_MODEL".into(), settings.llm_model.clone()),
            ],
        }
    }

    /// Call a Python worker with a JSON request and get a JSON response.
    pub async fn call(&self, request: Value) -> Result<Value> {
        let request_str =
            serde_json::to_string(&request).context("Failed to serialize request")?;

        debug!("Python bridge call: {}", request_str.chars().take(200).collect::<String>());

        let mut child = Command::new(&self.python_bin)
            .arg(&self.worker_script)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .envs(self.env_vars.iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .spawn()
            .context("Failed to spawn Python worker")?;

        // Write request to stdin
        let mut stdin = child.stdin.take().context("Failed to open stdin")?;
        stdin
            .write_all(request_str.as_bytes())
            .await
            .context("Failed to write to stdin")?;
        stdin
            .write_all(b"\n")
            .await
            .context("Failed to write newline")?;
        drop(stdin); // Close stdin to signal end of input

        // Read response from stdout
        let stdout = child.stdout.take().context("Failed to open stdout")?;
        let mut reader = BufReader::new(stdout);

        let mut response_lines = Vec::new();
        let mut line = String::new();
        while reader.read_line(&mut line).await? > 0 {
            let trimmed = line.trim().to_string();
            if !trimmed.is_empty() {
                // Check if it's an update vs final response
                if let Ok(val) = serde_json::from_str::<Value>(&trimmed) {
                    if val["type"].as_str() == Some("update") {
                        info!("Python update: {}", val["message"].as_str().unwrap_or(""));
                    } else {
                        response_lines.push(trimmed);
                    }
                }
            }
            line.clear();
        }

        // Wait for process to complete
        let status = child.wait().await?;

        if !status.success() {
            // Read stderr for error info
            let stderr = child.stderr.take();
            let err_msg = if let Some(stderr) = stderr {
                let mut err_reader = BufReader::new(stderr);
                let mut err = String::new();
                err_reader.read_line(&mut err).await.ok();
                err
            } else {
                "Unknown error".into()
            };
            error!("Python worker failed: {err_msg}");
            anyhow::bail!("Python worker exited with error: {err_msg}");
        }

        // Parse the last response line as the final result
        let response_str = response_lines
            .last()
            .context("No response from Python worker")?;

        let response: Value =
            serde_json::from_str(response_str).context("Invalid JSON from Python worker")?;

        Ok(response)
    }

    /// Call Python worker with streaming updates via callback.
    pub async fn call_streaming<F>(
        &self,
        request: Value,
        on_update: F,
    ) -> Result<Value>
    where
        F: Fn(String) + Send + 'static,
    {
        let request_str = serde_json::to_string(&request)?;

        let mut child = Command::new(&self.python_bin)
            .arg(&self.worker_script)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .envs(self.env_vars.iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .spawn()?;

        let mut stdin = child.stdin.take().context("No stdin")?;
        stdin.write_all(request_str.as_bytes()).await?;
        stdin.write_all(b"\n").await?;
        drop(stdin);

        let stdout = child.stdout.take().context("No stdout")?;
        let mut reader = BufReader::new(stdout);
        let mut final_response = None;
        let mut line = String::new();

        while reader.read_line(&mut line).await? > 0 {
            let trimmed = line.trim().to_string();
            if let Ok(val) = serde_json::from_str::<Value>(&trimmed) {
                if val["type"].as_str() == Some("update") {
                    on_update(val["message"].as_str().unwrap_or("").to_string());
                } else {
                    final_response = Some(val);
                }
            }
            line.clear();
        }

        child.wait().await?;
        final_response.context("No final response from Python worker")
    }
}
