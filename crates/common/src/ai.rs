use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};

// ── Guild settings ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AiGuildSettings {
    pub guild_id: String,
    pub advisor_mode_enabled: bool,
    pub approval_required: bool,
    pub owner_dm_enabled: bool,
    pub live_pulse_enabled: bool,
    pub live_pulse_interval_minutes: i32,
    pub real_time_alerts_enabled: bool,
    pub daily_briefing_enabled: bool,
    pub weekly_report_enabled: bool,
    pub retention_days: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateAiGuildSettings {
    pub advisor_mode_enabled: Option<bool>,
    pub approval_required: Option<bool>,
    pub owner_dm_enabled: Option<bool>,
    pub live_pulse_enabled: Option<bool>,
    pub live_pulse_interval_minutes: Option<i32>,
    pub real_time_alerts_enabled: Option<bool>,
    pub daily_briefing_enabled: Option<bool>,
    pub weekly_report_enabled: Option<bool>,
    pub retention_days: Option<i32>,
}

// ── Observation ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AiMessageObservation {
    pub id: i64,
    pub guild_id: String,
    pub channel_id: String,
    pub message_id: String,
    pub author_id: String,
    pub occurred_at: DateTime<Utc>,
    pub content_redacted: Option<String>,
    pub content_fingerprint: Option<String>,
    pub redaction_status: String,
    pub redaction_version: Option<String>,
    pub language: Option<String>,
    pub is_question: bool,
    pub is_feedback: bool,
    pub is_support_request: bool,
    pub sentiment: Option<String>,
    pub urgency: Option<String>,
    pub category: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewAiMessageObservation {
    pub guild_id: String,
    pub channel_id: String,
    pub message_id: String,
    pub author_id: String,
    pub occurred_at: DateTime<Utc>,
    pub content_redacted: Option<String>,
    pub content_fingerprint: Option<String>,
    pub redaction_status: &'static str,
    pub redaction_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiObservationClassification {
    pub is_question: bool,
    pub is_feedback: bool,
    pub is_support_request: bool,
    pub sentiment: String,
    pub urgency: String,
    pub category: Option<String>,
}

// ── Live pulse ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LivePulseResponse {
    pub window_start: DateTime<Utc>,
    pub window_end: DateTime<Utc>,
    pub window_minutes: i32,
    pub total_observations: i64,
    pub classified_count: i64,
    pub question_count: i64,
    pub feedback_count: i64,
    pub support_count: i64,
    pub positive_sentiment_count: i64,
    pub negative_sentiment_count: i64,
    pub neutral_sentiment_count: i64,
    pub high_urgency_count: i64,
}

// ── Store ─────────────────────────────────────────────────────────────────────

#[async_trait]
pub trait AiStore: Send + Sync {
    async fn ensure_schema(&self) -> Result<()>;
    async fn get_guild_settings(&self, guild_id: &str) -> Result<Option<AiGuildSettings>>;
    async fn upsert_guild_settings(
        &self,
        guild_id: &str,
        update: &UpdateAiGuildSettings,
    ) -> Result<AiGuildSettings>;
    /// Returns true only when guild AI is enabled AND the channel has content capture on.
    async fn is_content_capture_enabled(
        &self,
        guild_id: &str,
        channel_id: &str,
    ) -> Result<bool>;
    async fn insert_observation(&self, obs: &NewAiMessageObservation) -> Result<i64>;
    async fn update_observation_classification(
        &self,
        id: i64,
        classification: &AiObservationClassification,
    ) -> Result<()>;
    async fn get_observation(&self, id: i64) -> Result<Option<AiMessageObservation>>;
    async fn live_pulse_stats(&self, guild_id: &str, window_minutes: i32)
        -> Result<LivePulseResponse>;
}

#[derive(Clone)]
pub struct PostgresAiStore {
    pool: PgPool,
}

impl PostgresAiStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AiStore for PostgresAiStore {
    async fn ensure_schema(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS ai_guild_settings (
                guild_id TEXT PRIMARY KEY,
                advisor_mode_enabled BOOLEAN NOT NULL DEFAULT TRUE,
                approval_required BOOLEAN NOT NULL DEFAULT TRUE,
                owner_dm_enabled BOOLEAN NOT NULL DEFAULT FALSE,
                live_pulse_enabled BOOLEAN NOT NULL DEFAULT TRUE,
                live_pulse_interval_minutes INTEGER NOT NULL DEFAULT 60,
                real_time_alerts_enabled BOOLEAN NOT NULL DEFAULT TRUE,
                daily_briefing_enabled BOOLEAN NOT NULL DEFAULT TRUE,
                weekly_report_enabled BOOLEAN NOT NULL DEFAULT TRUE,
                retention_days INTEGER NOT NULL DEFAULT 30,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );
            "#,
        )
        .execute(&self.pool)
        .await
        .context("failed to create ai_guild_settings table")?;

        // Drop legacy ai_enabled column if it exists from a previous schema version.
        sqlx::query(
            "ALTER TABLE ai_guild_settings DROP COLUMN IF EXISTS ai_enabled",
        )
        .execute(&self.pool)
        .await
        .context("failed to drop legacy ai_enabled column")?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS ai_channel_settings (
                guild_id TEXT NOT NULL,
                channel_id TEXT NOT NULL,
                monitoring_enabled BOOLEAN NOT NULL DEFAULT TRUE,
                content_analysis_enabled BOOLEAN NOT NULL DEFAULT FALSE,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                PRIMARY KEY (guild_id, channel_id)
            );
            "#,
        )
        .execute(&self.pool)
        .await
        .context("failed to create ai_channel_settings table")?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS ai_message_observations (
                id BIGSERIAL PRIMARY KEY,
                guild_id TEXT NOT NULL,
                channel_id TEXT NOT NULL,
                message_id TEXT NOT NULL,
                author_id TEXT NOT NULL,
                occurred_at TIMESTAMPTZ NOT NULL,
                content_redacted TEXT NULL,
                content_fingerprint TEXT NULL,
                redaction_status TEXT NOT NULL DEFAULT 'not_captured',
                redaction_version TEXT NULL,
                language TEXT NULL,
                is_question BOOLEAN NOT NULL DEFAULT FALSE,
                is_feedback BOOLEAN NOT NULL DEFAULT FALSE,
                is_support_request BOOLEAN NOT NULL DEFAULT FALSE,
                sentiment TEXT NULL,
                urgency TEXT NULL,
                category TEXT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                UNIQUE (guild_id, message_id)
            );
            "#,
        )
        .execute(&self.pool)
        .await
        .context("failed to create ai_message_observations table")?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_ai_obs_guild_occurred
                ON ai_message_observations (guild_id, occurred_at DESC);
            "#,
        )
        .execute(&self.pool)
        .await
        .context("failed to create ai_observations index")?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_ai_obs_unclassified
                ON ai_message_observations (guild_id, created_at)
                WHERE sentiment IS NULL AND content_redacted IS NOT NULL;
            "#,
        )
        .execute(&self.pool)
        .await
        .context("failed to create ai_observations unclassified index")?;

        Ok(())
    }

    async fn get_guild_settings(&self, guild_id: &str) -> Result<Option<AiGuildSettings>> {
        sqlx::query_as::<_, AiGuildSettings>(
            "SELECT * FROM ai_guild_settings WHERE guild_id = $1",
        )
        .bind(guild_id)
        .fetch_optional(&self.pool)
        .await
        .context("failed to fetch ai_guild_settings")
    }

    async fn upsert_guild_settings(
        &self,
        guild_id: &str,
        update: &UpdateAiGuildSettings,
    ) -> Result<AiGuildSettings> {
        // Ensure row exists with defaults before applying the patch.
        sqlx::query(
            "INSERT INTO ai_guild_settings (guild_id) VALUES ($1) ON CONFLICT (guild_id) DO NOTHING",
        )
        .bind(guild_id)
        .execute(&self.pool)
        .await
        .context("failed to ensure ai_guild_settings row")?;

        sqlx::query(
            r#"
            UPDATE ai_guild_settings SET
                advisor_mode_enabled        = COALESCE($1, advisor_mode_enabled),
                approval_required           = COALESCE($2, approval_required),
                owner_dm_enabled            = COALESCE($3, owner_dm_enabled),
                live_pulse_enabled          = COALESCE($4, live_pulse_enabled),
                live_pulse_interval_minutes = COALESCE($5, live_pulse_interval_minutes),
                real_time_alerts_enabled    = COALESCE($6, real_time_alerts_enabled),
                daily_briefing_enabled      = COALESCE($7, daily_briefing_enabled),
                weekly_report_enabled       = COALESCE($8, weekly_report_enabled),
                retention_days              = COALESCE($9, retention_days),
                updated_at                  = NOW()
            WHERE guild_id = $10
            "#,
        )
        .bind(update.advisor_mode_enabled)
        .bind(update.approval_required)
        .bind(update.owner_dm_enabled)
        .bind(update.live_pulse_enabled)
        .bind(update.live_pulse_interval_minutes)
        .bind(update.real_time_alerts_enabled)
        .bind(update.daily_briefing_enabled)
        .bind(update.weekly_report_enabled)
        .bind(update.retention_days)
        .bind(guild_id)
        .execute(&self.pool)
        .await
        .context("failed to update ai_guild_settings")?;

        self.get_guild_settings(guild_id)
            .await?
            .context("ai_guild_settings row missing after upsert")
    }

    async fn is_content_capture_enabled(
        &self,
        guild_id: &str,
        channel_id: &str,
    ) -> Result<bool> {
        let row: Option<(bool,)> = sqlx::query_as(
            r#"
            SELECT TRUE
            FROM ai_guild_settings g
            JOIN ai_channel_settings c
              ON c.guild_id = g.guild_id AND c.channel_id = $2
            WHERE g.guild_id = $1
              AND c.content_analysis_enabled = TRUE
              AND c.monitoring_enabled = TRUE
            "#,
        )
        .bind(guild_id)
        .bind(channel_id)
        .fetch_optional(&self.pool)
        .await
        .context("failed to check content capture enabled")?;

        Ok(row.is_some())
    }

    async fn insert_observation(&self, obs: &NewAiMessageObservation) -> Result<i64> {
        let row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO ai_message_observations (
                guild_id, channel_id, message_id, author_id, occurred_at,
                content_redacted, content_fingerprint, redaction_status, redaction_version
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (guild_id, message_id) DO NOTHING
            RETURNING id
            "#,
        )
        .bind(&obs.guild_id)
        .bind(&obs.channel_id)
        .bind(&obs.message_id)
        .bind(&obs.author_id)
        .bind(obs.occurred_at)
        .bind(&obs.content_redacted)
        .bind(&obs.content_fingerprint)
        .bind(obs.redaction_status)
        .bind(&obs.redaction_version)
        .fetch_one(&self.pool)
        .await
        .context("failed to insert ai_message_observation")?;

        Ok(row.0)
    }

    async fn update_observation_classification(
        &self,
        id: i64,
        c: &AiObservationClassification,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE ai_message_observations
            SET is_question = $1, is_feedback = $2, is_support_request = $3,
                sentiment = $4, urgency = $5, category = $6
            WHERE id = $7
            "#,
        )
        .bind(c.is_question)
        .bind(c.is_feedback)
        .bind(c.is_support_request)
        .bind(&c.sentiment)
        .bind(&c.urgency)
        .bind(&c.category)
        .bind(id)
        .execute(&self.pool)
        .await
        .context("failed to update observation classification")?;

        Ok(())
    }

    async fn get_observation(&self, id: i64) -> Result<Option<AiMessageObservation>> {
        sqlx::query_as::<_, AiMessageObservation>(
            "SELECT * FROM ai_message_observations WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .context("failed to fetch ai_message_observation")
    }

    async fn live_pulse_stats(
        &self,
        guild_id: &str,
        window_minutes: i32,
    ) -> Result<LivePulseResponse> {
        #[derive(FromRow)]
        struct StatsRow {
            total_observations: i64,
            classified_count: i64,
            question_count: i64,
            feedback_count: i64,
            support_count: i64,
            positive_sentiment_count: i64,
            negative_sentiment_count: i64,
            neutral_sentiment_count: i64,
            high_urgency_count: i64,
        }

        let window_start = Utc::now() - Duration::minutes(window_minutes as i64);

        let row = sqlx::query_as::<_, StatsRow>(
            r#"
            SELECT
                COUNT(*) AS total_observations,
                COUNT(*) FILTER (WHERE sentiment IS NOT NULL) AS classified_count,
                COUNT(*) FILTER (WHERE is_question) AS question_count,
                COUNT(*) FILTER (WHERE is_feedback) AS feedback_count,
                COUNT(*) FILTER (WHERE is_support_request) AS support_count,
                COUNT(*) FILTER (WHERE sentiment = 'positive') AS positive_sentiment_count,
                COUNT(*) FILTER (WHERE sentiment = 'negative') AS negative_sentiment_count,
                COUNT(*) FILTER (WHERE sentiment = 'neutral') AS neutral_sentiment_count,
                COUNT(*) FILTER (WHERE urgency = 'high') AS high_urgency_count
            FROM ai_message_observations
            WHERE guild_id = $1 AND occurred_at >= $2
            "#,
        )
        .bind(guild_id)
        .bind(window_start)
        .fetch_one(&self.pool)
        .await
        .context("failed to fetch live pulse stats")?;

        Ok(LivePulseResponse {
            window_start,
            window_end: Utc::now(),
            window_minutes,
            total_observations: row.total_observations,
            classified_count: row.classified_count,
            question_count: row.question_count,
            feedback_count: row.feedback_count,
            support_count: row.support_count,
            positive_sentiment_count: row.positive_sentiment_count,
            negative_sentiment_count: row.negative_sentiment_count,
            neutral_sentiment_count: row.neutral_sentiment_count,
            high_urgency_count: row.high_urgency_count,
        })
    }
}
