use anyhow::{Context, Result};

#[derive(Clone, Debug)]
pub struct Settings {
    pub telegram_bot_token: String,
    pub telegram_group_id: i64,
    pub telegram_admin_user_id: i64,

    pub llm_provider: String,
    pub llm_api_key: String,
    pub llm_base_url: String,
    pub llm_model: String,

    pub python_bin: String,
    pub total_ram_mb: u64,
}

impl Settings {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            telegram_bot_token: std::env::var("TELEGRAM_BOT_TOKEN")
                .context("TELEGRAM_BOT_TOKEN required")?,
            telegram_group_id: std::env::var("TELEGRAM_GROUP_ID")
                .context("TELEGRAM_GROUP_ID required")?
                .parse()
                .context("TELEGRAM_GROUP_ID must be integer")?,
            telegram_admin_user_id: std::env::var("TELEGRAM_ADMIN_USER_ID")
                .unwrap_or_default()
                .parse()
                .unwrap_or(0),

            llm_provider: std::env::var("LLM_PROVIDER").unwrap_or_else(|_| "openrouter".into()),
            llm_api_key: std::env::var("LLM_API_KEY").context("LLM_API_KEY required")?,
            llm_base_url: std::env::var("LLM_BASE_URL")
                .unwrap_or_else(|_| "https://openrouter.ai/api/v1".into()),
            llm_model: std::env::var("LLM_MODEL")
                .unwrap_or_else(|_| "anthropic/claude-sonnet-4-20250514".into()),

            python_bin: std::env::var("PYTHON_BIN").unwrap_or_else(|_| "python".into()),
            total_ram_mb: std::env::var("TOTAL_RAM_MB")
                .unwrap_or_else(|_| "8192".into())
                .parse()
                .unwrap_or(8192),
        })
    }
}
