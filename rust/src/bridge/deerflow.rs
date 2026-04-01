use anyhow::{Context, Result};
use serde_json::Value;
use tracing::{debug, info, warn};

use crate::config::Settings;

/// Bridge to DeerFlow LangGraph backend via HTTP API.
/// Replaces the old PythonBridge subprocess approach.
pub struct DeerFlowBridge {
    client: reqwest::Client,
    base_url: String,
}

impl DeerFlowBridge {
    pub fn new(settings: &Settings) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .connect_timeout(std::time::Duration::from_secs(10))
            .pool_max_idle_per_host(5)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            client,
            base_url: settings.deerflow_url.clone(),
        }
    }

    /// Check if DeerFlow backend is alive.
    pub async fn health_check(&self) -> Result<bool> {
        let url = format!("{}/api/health", self.base_url);
        match self.client.get(&url).timeout(std::time::Duration::from_secs(5)).send().await {
            Ok(resp) => Ok(resp.status().is_success()),
            Err(e) => {
                warn!("DeerFlow health check failed: {e}");
                Ok(false)
            }
        }
    }

    /// Create a new thread for a conversation.
    async fn create_thread(&self) -> Result<String> {
        let url = format!("{}/api/chat/thread", self.base_url);
        let resp: Value = self
            .client
            .post(&url)
            .json(&serde_json::json!({}))
            .send()
            .await
            .context("Failed to create DeerFlow thread")?
            .json()
            .await
            .context("Invalid thread response")?;

        let thread_id = resp["thread_id"]
            .as_str()
            .unwrap_or_else(|| resp["id"].as_str().unwrap_or("default"))
            .to_string();

        debug!("Created DeerFlow thread: {thread_id}");
        Ok(thread_id)
    }

    /// Send a message to DeerFlow and get the full response (non-streaming).
    /// Creates a thread automatically.
    pub async fn chat(&self, message: &str) -> Result<DeerFlowResponse> {
        let thread_id = self.create_thread().await.unwrap_or_else(|_| "default".into());
        self.chat_in_thread(&thread_id, message).await
    }

    /// Fast chat: bypasses DeerFlow agent, calls LLM directly via the adapter.
    /// Much faster for simple conversations (no planning/research overhead).
    pub async fn chat_fast(&self, message: &str) -> Result<DeerFlowResponse> {
        let thread_id = self.create_thread().await.unwrap_or_else(|_| "default".into());
        let url = format!("{}/api/chat/fast", self.base_url);

        let body = serde_json::json!({
            "messages": [{"role": "user", "content": message}],
            "thread_id": thread_id,
        });

        info!("DeerFlow fast request: {}", message.chars().take(100).collect::<String>());

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .timeout(std::time::Duration::from_secs(120))
            .send()
            .await
            .context("Failed to send fast chat request")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("DeerFlow fast returned {status}: {body}");
        }

        info!("DeerFlow fast: reading response...");
        let text = resp.text().await.context("Failed to read fast response")?;
        info!("DeerFlow fast: got {} bytes", text.len());

        // Parse as JSON (adapter returns plain JSON, not SSE)
        let final_answer = if let Ok(json) = serde_json::from_str::<Value>(&text) {
            json["content"]
                .as_str()
                .or_else(|| json["output"].as_str())
                .unwrap_or("")
                .to_string()
        } else {
            // Fallback: try SSE format
            let mut answer = String::new();
            for line in text.lines() {
                if let Some(data) = line.strip_prefix("data: ") {
                    if data == "[DONE]" { break; }
                    if let Ok(event) = serde_json::from_str::<Value>(data) {
                        if let Some(c) = event["content"].as_str() {
                            answer.push_str(c);
                        }
                    }
                }
            }
            if answer.is_empty() { text } else { answer }
        };

        Ok(DeerFlowResponse {
            thread_id,
            answer: final_answer,
            updates: vec![],
        })
    }

    /// Send a message within an existing thread.
    pub async fn chat_in_thread(&self, thread_id: &str, message: &str) -> Result<DeerFlowResponse> {
        let url = format!("{}/api/chat/stream", self.base_url);

        let body = serde_json::json!({
            "messages": [{"role": "user", "content": message}],
            "thread_id": thread_id,
            "auto_accepted_plan": true,
            "max_plan_iterations": 3,
            "max_step_num": 15,
        });

        debug!("DeerFlow request to {url}: {}", message.chars().take(100).collect::<String>());

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .context("Failed to send DeerFlow chat request")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("DeerFlow returned {status}: {body}");
        }

        // Parse SSE stream and collect the final answer
        let text = resp.text().await.context("Failed to read DeerFlow response")?;
        let mut final_answer = String::new();
        let mut updates = Vec::new();

        for line in text.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                if data == "[DONE]" {
                    break;
                }
                if let Ok(event) = serde_json::from_str::<Value>(data) {
                    // Extract content from SSE event
                    let node = event["node"].as_str()
                        .or_else(|| event["langgraph_node"].as_str())
                        .unwrap_or("");
                    let content = event["content"].as_str()
                        .or_else(|| event["data"]["content"].as_str())
                        .or_else(|| event["output"].as_str());
                    if let Some(text) = content {
                        if !text.is_empty() {
                            match node {
                                "reporter" | "final_answer" | "end" => final_answer.push_str(text),
                                _ => updates.push(format!("[{node}] {text}")),
                            }
                        }
                    }
                    if let Some(messages) = event["messages"].as_array() {
                        for msg in messages {
                            if msg["role"].as_str() == Some("assistant") {
                                if let Some(c) = msg["content"].as_str() {
                                    final_answer.push_str(c);
                                }
                            }
                        }
                    }
                }
            }
        }

        // If no SSE markers found, try parsing as plain JSON
        if final_answer.is_empty() && updates.is_empty() {
            if let Ok(json) = serde_json::from_str::<Value>(&text) {
                final_answer = json["content"]
                    .as_str()
                    .or_else(|| json["output"].as_str())
                    .or_else(|| json["result"].as_str())
                    .unwrap_or("")
                    .to_string();
            }
            if final_answer.is_empty() {
                final_answer = text;
            }
        }

        Ok(DeerFlowResponse {
            thread_id: thread_id.to_string(),
            answer: final_answer,
            updates,
        })
    }

    /// Fast idea planning using the working /api/chat/fast endpoint.
    pub async fn plan_idea_fast(&self, idea: &str) -> Result<DeerFlowResponse> {
        let prompt = format!(
            "You are a tool-builder AI. Analyze this idea and create a build plan.\n\n\
             Respond with:\n\
             PROJECT_NAME: <kebab-case-name>\n\
             SUMMARY: <one line summary of what the tool does>\n\n\
             Then a detailed implementation plan including:\n\
             - Technology stack (prefer Python for easy packaging)\n\
             - File structure\n\
             - Key features and logic\n\
             - Required dependencies/libraries\n\n\
             Idea: {idea}"
        );
        self.chat_fast(&prompt).await
    }

    /// Generate code for a project using the /api/build/generate endpoint.
    /// This uses a dedicated code-gen endpoint (no web tools, higher token limit).
    pub async fn build_generate(&self, project_name: &str, plan: &str) -> Result<DeerFlowResponse> {
        let url = format!("{}/api/build/generate", self.base_url);
        let body = serde_json::json!({
            "project_name": project_name,
            "plan": plan,
        });

        info!("build_generate: generating code for '{project_name}'");

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .timeout(std::time::Duration::from_secs(180))
            .send()
            .await
            .context("Failed to send build generate request")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Build generate returned {status}: {body}");
        }

        let text = resp.text().await?;
        let final_answer = if let Ok(json) = serde_json::from_str::<Value>(&text) {
            json["content"]
                .as_str()
                .unwrap_or("")
                .to_string()
        } else {
            text
        };

        info!("build_generate: got {} bytes of code", final_answer.len());

        Ok(DeerFlowResponse {
            thread_id: String::new(),
            answer: final_answer,
            updates: vec![],
        })
    }

    /// Convenience: send a research query to DeerFlow.
    pub async fn research(&self, query: &str) -> Result<DeerFlowResponse> {
        let prompt = format!(
            "Research the following topic and provide a comprehensive answer:\n\n{query}"
        );
        self.chat(&prompt).await
    }
}

/// Response from DeerFlow API
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DeerFlowResponse {
    pub thread_id: String,
    pub answer: String,
    pub updates: Vec<String>,
}

impl DeerFlowResponse {
    /// Extract project name from a planning response.
    pub fn extract_project_name(&self) -> Option<String> {
        for line in self.answer.lines() {
            if let Some(name) = line.strip_prefix("PROJECT_NAME:") {
                let name = name.trim().to_lowercase().replace(' ', "-");
                if !name.is_empty() {
                    return Some(name);
                }
            }
        }
        None
    }

    /// Extract plan summary from a planning response.
    pub fn extract_summary(&self) -> String {
        for line in self.answer.lines() {
            if let Some(summary) = line.strip_prefix("SUMMARY:") {
                return summary.trim().to_string();
            }
        }
        // Fallback: first 300 chars
        self.answer.chars().take(300).collect()
    }

    /// Extract file blocks from code generation response.
    /// Supports two formats:
    /// 1. ```filename: <path>\n<content>\n```  (triple backtick wrapped)
    /// 2. filename: <path>\n<content>  (plain, separated by next filename: or EOF)
    pub fn extract_files(&self) -> Vec<(String, String)> {
        let mut files = Vec::new();

        // Try format 1: ```filename: blocks
        let mut current_file: Option<String> = None;
        let mut current_content = String::new();
        let mut in_block = false;
        let mut found_backtick_format = false;

        for line in self.answer.lines() {
            if line.starts_with("```filename:") || line.starts_with("```Filename:") {
                if let Some(path) = line.split(':').nth(1) {
                    let path = path.trim().to_string();
                    if !path.is_empty() {
                        current_file = Some(path);
                        current_content.clear();
                        in_block = true;
                        found_backtick_format = true;
                    }
                }
            } else if line == "```" && in_block {
                if let Some(ref path) = current_file {
                    files.push((path.clone(), current_content.clone()));
                }
                current_file = None;
                current_content.clear();
                in_block = false;
            } else if in_block {
                if !current_content.is_empty() {
                    current_content.push('\n');
                }
                current_content.push_str(line);
            }
        }

        if found_backtick_format && !files.is_empty() {
            return files;
        }

        // Try format 2: plain "filename: <path>" lines
        files.clear();
        current_file = None;
        current_content.clear();

        for line in self.answer.lines() {
            let trimmed = line.trim();
            if (trimmed.starts_with("filename:") || trimmed.starts_with("Filename:"))
                && !trimmed.contains("```")
            {
                // Save previous file if any
                if let Some(ref path) = current_file {
                    let content = current_content.trim().to_string();
                    if !content.is_empty() {
                        files.push((path.clone(), content));
                    }
                }
                // Start new file
                if let Some(path) = trimmed.split(':').nth(1) {
                    let path = path.trim().to_string();
                    if !path.is_empty() {
                        current_file = Some(path);
                        current_content.clear();
                    }
                }
            } else if current_file.is_some() {
                // Skip code fence markers like ```python, ```
                if trimmed == "```" || (trimmed.starts_with("```") && !trimmed.contains("filename:")) {
                    continue;
                }
                if !current_content.is_empty() {
                    current_content.push('\n');
                }
                current_content.push_str(line);
            }
        }
        // Don't forget the last file
        if let Some(ref path) = current_file {
            let content = current_content.trim().to_string();
            if !content.is_empty() {
                files.push((path.clone(), content));
            }
        }

        files
    }
}
