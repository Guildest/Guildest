use std::env;

use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct Settings {
    pub discord_token: String,
    pub discord_application_id: u64,
    pub discord_api_base_url: String,
    pub discord_client_secret: String,
    pub discord_enable_guild_members_intent: bool,
    pub database_url: String,
    pub redis_url: String,
    pub api_bind_addr: String,
    pub gateway_metrics_bind_addr: String,
    pub public_api_base_url: String,
    pub public_api_allowed_origin: String,
    pub public_site_url: String,
    pub worker_backfill_page_delay_ms: u64,
    pub worker_backfill_channel_concurrency: usize,
    pub worker_metrics_bind_addr: String,
    pub worker_consumer_prefix: String,
    pub openrouter_api_key: Option<String>,
    pub ai_classify_model: String,
    pub ai_synthesis_model: String,
    pub discord_enable_message_content_intent: bool,
    pub resend_api_key: Option<String>,
    pub resend_from_email: String,
    pub guildest_email_to: Vec<String>,
    pub rust_log: String,
}

impl Settings {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            discord_token: env::var("DISCORD_TOKEN").context("missing DISCORD_TOKEN")?,
            discord_application_id: env::var("DISCORD_APPLICATION_ID")
                .context("missing DISCORD_APPLICATION_ID")?
                .parse()
                .context("invalid DISCORD_APPLICATION_ID")?,
            discord_api_base_url: env::var("DISCORD_API_BASE_URL")
                .unwrap_or_else(|_| "https://discord.com/api/v10".to_string()),
            discord_client_secret: env::var("DISCORD_CLIENT_SECRET")
                .context("missing DISCORD_CLIENT_SECRET")?,
            discord_enable_guild_members_intent: env::var("DISCORD_ENABLE_GUILD_MEMBERS_INTENT")
                .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
                .unwrap_or(false),
            database_url: env::var("DATABASE_URL").context("missing DATABASE_URL")?,
            redis_url: env::var("REDIS_URL").context("missing REDIS_URL")?,
            api_bind_addr: env::var("API_BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string()),
            gateway_metrics_bind_addr: env::var("GATEWAY_METRICS_BIND_ADDR")
                .unwrap_or_else(|_| "127.0.0.1:9092".to_string()),
            public_api_base_url: env::var("PUBLIC_API_BASE_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:8080".to_string()),
            public_api_allowed_origin: env::var("PUBLIC_API_ALLOWED_ORIGIN")
                .unwrap_or_else(|_| "*".to_string()),
            public_site_url: env::var("PUBLIC_SITE_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:3000".to_string()),
            worker_backfill_page_delay_ms: env::var("WORKER_BACKFILL_PAGE_DELAY_MS")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(25),
            worker_backfill_channel_concurrency: env::var("WORKER_BACKFILL_CHANNEL_CONCURRENCY")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(4),
            worker_metrics_bind_addr: env::var("WORKER_METRICS_BIND_ADDR")
                .unwrap_or_else(|_| "127.0.0.1:9091".to_string()),
            worker_consumer_prefix: env::var("WORKER_CONSUMER_PREFIX")
                .unwrap_or_else(|_| "guildest-worker".to_string()),
            openrouter_api_key: env::var("OPENROUTER_API_KEY").ok(),
            ai_classify_model: env::var("AI_CLASSIFY_MODEL")
                .unwrap_or_else(|_| "stepfun/step-3.5-flash".to_string()),
            ai_synthesis_model: env::var("AI_SYNTHESIS_MODEL")
                .unwrap_or_else(|_| "minimax/minimax-m2.7".to_string()),
            discord_enable_message_content_intent: env::var(
                "DISCORD_ENABLE_MESSAGE_CONTENT_INTENT",
            )
            .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
            .unwrap_or(false),
            resend_api_key: env::var("RESEND_API_KEY").ok(),
            resend_from_email: env::var("RESEND_FROM_EMAIL")
                .unwrap_or_else(|_| "Guildest <onboarding@resend.dev>".to_string()),
            guildest_email_to: env::var("GUILDEST_EMAIL_TO")
                .unwrap_or_else(|_| "hi@guildest.com".to_string())
                .split(',')
                .map(|email| email.trim().to_string())
                .filter(|email| !email.is_empty())
                .collect(),
            rust_log: env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()),
        })
    }
}
