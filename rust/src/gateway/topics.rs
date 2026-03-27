use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

/// System forum topic names
const SYSTEM_TOPICS: &[(&str, &str)] = &[
    ("ideas", "💡 Ideas"),
    ("research", "🔍 Research"),
    ("dashboard", "📊 Dashboard"),
    ("tool-management", "🛠 Tool Management"),
    ("agent-logs", "🤖 Agent Logs"),
];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TopicInfo {
    pub thread_id: i32,
    pub name: String,
    pub display_name: String,
}

#[derive(Clone)]
pub struct TopicManager {
    bot_token: String,
    group_id: i64,
    /// Maps thread_id -> topic info
    topics: Arc<RwLock<HashMap<i32, TopicInfo>>>,
    /// Maps name -> thread_id
    name_index: Arc<RwLock<HashMap<String, i32>>>,
    data_path: PathBuf,
}

impl TopicManager {
    pub fn new(bot_token: String, group_id: i64) -> Self {
        let data_path = PathBuf::from("data/topics.json");
        let mgr = Self {
            bot_token,
            group_id,
            topics: Arc::new(RwLock::new(HashMap::new())),
            name_index: Arc::new(RwLock::new(HashMap::new())),
            data_path,
        };
        // Load saved topics asynchronously will be called via ensure_system_topics
        mgr
    }

    /// Ensure all system topics exist, creating them if needed.
    pub async fn ensure_system_topics(&self) -> Result<()> {
        self.load_from_disk().await;

        for (name, display_name) in SYSTEM_TOPICS {
            let exists = self.name_index.read().await.contains_key(*name);
            if !exists {
                info!("Creating system topic: {}", display_name);
                match self.create_topic(display_name).await {
                    Ok(thread_id) => {
                        self.register(thread_id, name.to_string(), display_name.to_string())
                            .await;
                    }
                    Err(e) => {
                        tracing::warn!("Failed to create topic {}: {e:#}", display_name);
                    }
                }
            }
        }

        self.save_to_disk().await;
        Ok(())
    }

    /// Create a new forum topic via Telegram API
    async fn create_topic(&self, name: &str) -> Result<i32> {
        let client = reqwest::Client::new();
        let url = format!(
            "https://api.telegram.org/bot{}/createForumTopic",
            self.bot_token
        );
        let resp = client
            .post(&url)
            .json(&serde_json::json!({
                "chat_id": self.group_id,
                "name": name,
            }))
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        let thread_id = resp["result"]["message_thread_id"]
            .as_i64()
            .ok_or_else(|| anyhow::anyhow!("No thread_id in response: {resp}"))?
            as i32;

        Ok(thread_id)
    }

    /// Create a topic for a new tool project
    pub async fn create_tool_topic(&self, tool_name: &str) -> Result<i32> {
        let display = format!("🔧 {tool_name}");
        let thread_id = self.create_topic(&display).await?;
        let name = format!("tool-{tool_name}");
        self.register(thread_id, name, display).await;
        self.save_to_disk().await;
        Ok(thread_id)
    }

    /// Register a topic mapping
    async fn register(&self, thread_id: i32, name: String, display_name: String) {
        let info = TopicInfo {
            thread_id,
            name: name.clone(),
            display_name,
        };
        self.topics.write().await.insert(thread_id, info);
        self.name_index.write().await.insert(name, thread_id);
    }

    /// Get topic name by thread_id
    pub fn topic_name(&self, thread_id: i32) -> Option<String> {
        // Use try_read to avoid blocking; return None if locked
        self.topics
            .try_read()
            .ok()
            .and_then(|map| map.get(&thread_id).map(|t| t.name.clone()))
    }

    /// Get thread_id by topic name
    pub async fn thread_id_for(&self, name: &str) -> Option<i32> {
        self.name_index.read().await.get(name).copied()
    }

    async fn load_from_disk(&self) {
        if let Ok(data) = tokio::fs::read_to_string(&self.data_path).await {
            if let Ok(topics) = serde_json::from_str::<Vec<TopicInfo>>(&data) {
                let mut map = self.topics.write().await;
                let mut idx = self.name_index.write().await;
                for t in topics {
                    idx.insert(t.name.clone(), t.thread_id);
                    map.insert(t.thread_id, t);
                }
                info!("Loaded {} topics from disk", map.len());
            }
        }
    }

    async fn save_to_disk(&self) {
        let topics: Vec<TopicInfo> = self.topics.read().await.values().cloned().collect();
        if let Ok(json) = serde_json::to_string_pretty(&topics) {
            let _ = tokio::fs::create_dir_all("data").await;
            let _ = tokio::fs::write(&self.data_path, json).await;
        }
    }
}
