use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{debug, info, warn};

use crate::config::Settings;

/// DeerFlow API thread/run response types
#[derive(Debug, Deserialize)]
struct ThreadResponse {
    thread_id: String,
}

#[derive(Debug, Serialize)]
struct RunRequest {
    input: RunInput,
    #[serde(skip_serializing_if = "Option::is_none")]
    config: Option<RunConfig>,
}

#[derive(Debug, Serialize)]
struct RunInput {
    messages: Vec<ChatMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
struct RunConfig {
    configurable: ConfigurableParams,
}

#[derive(Debug, Serialize)]
struct ConfigurableParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    thread_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_plan_iterations: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_step_num: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    auto_accepted_plan: Option<bool>,
}

/// Parsed SSE event from DeerFlow streaming response
#[derive(Debug)]
pub enum StreamEvent {
    /// Intermediate update (tool calls, agent reasoning, etc.)
    Update { node: String, content: String },
    /// Final answer from the agent
    FinalAnswer(String),
    /// Error from the server
    Error(String),
}

/// Bridge to DeerFlow LangGraph backend via HTTP API.
/// Replaces the old PythonBridge subprocess approach.
pub struct DeerFlowBridge {
    client: reqwest::Client,
    base_url: String,
}

impl DeerFlowBridge {
    pub fn new(settings: &Settings) -> Self {
        Self {
            client: reqwest::Client::new(),
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
                    self.extract_content(&event, &mut final_answer, &mut updates);
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

    /// Send a message and stream updates via callback.
    pub async fn chat_streaming<F>(
        &self,
        message: &str,
        on_update: F,
    ) -> Result<DeerFlowResponse>
    where
        F: Fn(StreamEvent) + Send + 'static,
    {
        let thread_id = self.create_thread().await.unwrap_or_else(|_| "default".into());

        let url = format!("{}/api/chat/stream", self.base_url);
        let body = serde_json::json!({
            "messages": [{"role": "user", "content": message}],
            "thread_id": thread_id,
            "auto_accepted_plan": true,
            "max_plan_iterations": 3,
            "max_step_num": 15,
        });

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .context("Failed to send DeerFlow streaming request")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("DeerFlow returned {status}: {body}");
        }

        let text = resp.text().await?;
        let mut final_answer = String::new();
        let mut updates = Vec::new();

        for line in text.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                if data == "[DONE]" {
                    break;
                }
                if let Ok(event) = serde_json::from_str::<Value>(data) {
                    // Extract node name for update routing
                    let node = event["node"]
                        .as_str()
                        .or_else(|| event["langgraph_node"].as_str())
                        .unwrap_or("unknown")
                        .to_string();

                    let content = event["content"]
                        .as_str()
                        .or_else(|| event["data"]["content"].as_str())
                        .unwrap_or("")
                        .to_string();

                    if !content.is_empty() {
                        on_update(StreamEvent::Update {
                            node: node.clone(),
                            content: content.clone(),
                        });
                        updates.push(content.clone());
                    }

                    self.extract_content(&event, &mut final_answer, &mut updates);
                }
            }
        }

        if final_answer.is_empty() && !updates.is_empty() {
            final_answer = updates.last().cloned().unwrap_or_default();
        }

        on_update(StreamEvent::FinalAnswer(final_answer.clone()));

        Ok(DeerFlowResponse {
            thread_id,
            answer: final_answer,
            updates,
        })
    }

    /// Extract content from a DeerFlow SSE event JSON
    fn extract_content(&self, event: &Value, final_answer: &mut String, updates: &mut Vec<String>) {
        // DeerFlow sends events with different structures depending on the node
        // Common patterns: {"node": "...", "content": "..."} or nested in data
        let node = event["node"]
            .as_str()
            .or_else(|| event["langgraph_node"].as_str())
            .unwrap_or("");

        let content = event["content"]
            .as_str()
            .or_else(|| event["data"]["content"].as_str())
            .or_else(|| event["output"].as_str());

        if let Some(text) = content {
            if !text.is_empty() {
                match node {
                    "reporter" | "final_answer" | "end" => {
                        final_answer.push_str(text);
                    }
                    _ => {
                        updates.push(format!("[{node}] {text}"));
                        info!("DeerFlow [{node}]: {}", text.chars().take(120).collect::<String>());
                    }
                }
            }
        }

        // Also check for messages array format
        if let Some(messages) = event["messages"].as_array() {
            for msg in messages {
                if let Some(c) = msg["content"].as_str() {
                    let role = msg["role"].as_str().unwrap_or("assistant");
                    if role == "assistant" {
                        final_answer.push_str(c);
                    }
                }
            }
        }
    }

    /// Convenience: send an "idea" to DeerFlow for planning.
    pub async fn process_idea(&self, idea: &str, user_id: i64) -> Result<DeerFlowResponse> {
        let prompt = format!(
            "You are a tool-builder AI. Analyze this idea and create a detailed build plan.\n\n\
             Idea: {idea}\n\n\
             User ID: {user_id}\n\n\
             Respond with:\n\
             1. A short project name (kebab-case, e.g. 'price-tracker')\n\
             2. A summary of what the tool does\n\
             3. Technology stack recommendations\n\
             4. Implementation steps\n\
             5. Expected resource requirements (RAM, CPU, storage)\n\n\
             Format your response as:\n\
             PROJECT_NAME: <name>\n\
             SUMMARY: <summary>\n\
             Then the detailed plan."
        );
        self.chat(&prompt).await
    }

    /// Convenience: send a research query to DeerFlow.
    pub async fn research(&self, query: &str) -> Result<DeerFlowResponse> {
        let prompt = format!(
            "Research the following topic and provide a comprehensive answer:\n\n{query}"
        );
        self.chat(&prompt).await
    }

    /// Convenience: ask DeerFlow to generate code for a project.
    pub async fn generate_code(
        &self,
        thread_id: &str,
        project_name: &str,
        plan: &Value,
    ) -> Result<DeerFlowResponse> {
        let prompt = format!(
            "You are a tool-builder AI. Generate the complete source code for project '{project_name}'.\n\n\
             Build Plan:\n{plan}\n\n\
             Requirements:\n\
             - Generate ALL files needed (source code, Dockerfile, requirements/dependencies, config)\n\
             - Each file should be complete and production-ready\n\
             - Include proper error handling and logging\n\
             - Include a Dockerfile for containerized deployment\n\
             - Output each file as:\n\
             ```filename: <path>\n<content>\n```",
            plan = serde_json::to_string_pretty(plan).unwrap_or_default()
        );
        self.chat_in_thread(thread_id, &prompt).await
    }

    /// Convenience: ask DeerFlow to test/review generated code.
    pub async fn review_and_test(
        &self,
        thread_id: &str,
        project_name: &str,
    ) -> Result<DeerFlowResponse> {
        let prompt = format!(
            "Review and test the code you generated for project '{project_name}'.\n\n\
             1. Check for bugs, security issues, and missing error handling\n\
             2. Run the tests if any were generated\n\
             3. Fix any issues found\n\
             4. Confirm the code is production-ready\n\n\
             If you find issues, output the corrected files using the same format:\n\
             ```filename: <path>\n<content>\n```\n\n\
             End with: TEST_RESULT: PASS or TEST_RESULT: FAIL"
        );
        self.chat_in_thread(thread_id, &prompt).await
    }
}

/// Response from DeerFlow API
#[derive(Debug, Clone)]
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

    /// Check if test result passed.
    pub fn test_passed(&self) -> bool {
        self.answer.contains("TEST_RESULT: PASS")
    }

    /// Extract file blocks from code generation response.
    /// Looks for ```filename: <path>\n<content>\n``` blocks.
    pub fn extract_files(&self) -> Vec<(String, String)> {
        let mut files = Vec::new();
        let mut current_file: Option<String> = None;
        let mut current_content = String::new();
        let mut in_block = false;

        for line in self.answer.lines() {
            if line.starts_with("```filename:") || line.starts_with("```Filename:") {
                // Start of a file block
                if let Some(path) = line.split(':').nth(1) {
                    current_file = Some(path.trim().to_string());
                    current_content.clear();
                    in_block = true;
                }
            } else if line == "```" && in_block {
                // End of a file block
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

        files
    }
}
