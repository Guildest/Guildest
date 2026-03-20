use anyhow::{Context, Result};
use async_trait::async_trait;
use sqlx::PgPool;

use crate::events::EventEnvelope;

#[async_trait]
pub trait RawEventStore: Send + Sync {
    async fn ensure_schema(&self) -> Result<()>;
    async fn insert(&self, event: &EventEnvelope) -> Result<i64>;
}

#[derive(Clone)]
pub struct PostgresRawEventStore {
    pool: PgPool,
}

impl PostgresRawEventStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

#[async_trait]
impl RawEventStore for PostgresRawEventStore {
    async fn ensure_schema(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS raw_events (
                id BIGSERIAL PRIMARY KEY,
                event_id UUID NOT NULL UNIQUE,
                event_name TEXT NOT NULL,
                guild_id TEXT NOT NULL,
                channel_id TEXT NULL,
                user_id TEXT NULL,
                occurred_at TIMESTAMPTZ NOT NULL,
                received_at TIMESTAMPTZ NOT NULL,
                schema_version INTEGER NOT NULL,
                payload_json JSONB NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );
            "#,
        )
        .execute(&self.pool)
        .await
        .context("failed to create raw_events table")?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_raw_events_guild_occurred_at
                ON raw_events (guild_id, occurred_at DESC);
            "#,
        )
        .execute(&self.pool)
        .await
        .context("failed to create raw_events guild index")?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_raw_events_name_occurred_at
                ON raw_events (event_name, occurred_at DESC);
            "#,
        )
        .execute(&self.pool)
        .await
        .context("failed to create raw_events name index")?;

        Ok(())
    }

    async fn insert(&self, event: &EventEnvelope) -> Result<i64> {
        let payload = serde_json::to_value(event).context("failed to serialize raw event")?;
        let row_id = sqlx::query_scalar(
            r#"
            INSERT INTO raw_events (
                event_id,
                event_name,
                guild_id,
                channel_id,
                user_id,
                occurred_at,
                received_at,
                schema_version,
                payload_json
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING id
            "#,
        )
        .bind(event.event_id)
        .bind(&event.event_name)
        .bind(&event.guild_id)
        .bind(&event.channel_id)
        .bind(&event.user_id)
        .bind(event.occurred_at)
        .bind(event.received_at)
        .bind(event.version)
        .bind(payload)
        .fetch_one(&self.pool)
        .await
        .context("failed to insert raw event")?;

        Ok(row_id)
    }
}
