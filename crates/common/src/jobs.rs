use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const BACKFILL_STREAM: &str = "jobs.backfill";
pub const AI_CLASSIFY_STREAM: &str = "jobs.ai_classify";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackfillJob {
    pub job_id: Uuid,
    pub guild_id: String,
    pub requested_by_user_id: Option<String>,
    pub days_requested: i32,
    pub start_at: DateTime<Utc>,
    pub end_at: DateTime<Utc>,
    pub requested_at: DateTime<Utc>,
    pub trigger_source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiClassifyJob {
    pub observation_id: i64,
    pub guild_id: String,
    pub channel_id: String,
}

impl AiClassifyJob {
    pub fn new(
        observation_id: i64,
        guild_id: impl Into<String>,
        channel_id: impl Into<String>,
    ) -> Self {
        Self {
            observation_id,
            guild_id: guild_id.into(),
            channel_id: channel_id.into(),
        }
    }
}

impl BackfillJob {
    pub fn new(
        guild_id: impl Into<String>,
        requested_by_user_id: Option<String>,
        days_requested: i32,
        start_at: DateTime<Utc>,
        end_at: DateTime<Utc>,
        trigger_source: impl Into<String>,
    ) -> Self {
        Self {
            job_id: Uuid::new_v4(),
            guild_id: guild_id.into(),
            requested_by_user_id,
            days_requested,
            start_at,
            end_at,
            requested_at: Utc::now(),
            trigger_source: trigger_source.into(),
        }
    }
}
