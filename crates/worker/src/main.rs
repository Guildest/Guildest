use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    net::SocketAddr,
    sync::{Arc, OnceLock},
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use axum::{
    Router, extract::State, http::StatusCode as HttpStatusCode, response::IntoResponse,
    routing::get,
};
use chrono::{DateTime, Days, NaiveDate, Utc};
use common::{
    config::Settings,
    events::{EventEnvelope, EventPayload},
    jobs::{BACKFILL_STREAM, BackfillJob},
    queue::{EventQueue, QueueDelivery, QueuedEventRef, RedisEventQueue},
    store::{PostgresRawEventStore, RawEventStore},
};
use prometheus::{
    Encoder, Histogram, HistogramOpts, HistogramVec, IntCounterVec, IntGaugeVec, Registry,
    TextEncoder,
};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool, postgres::PgPoolOptions};
use tokio::sync::{Mutex, Notify};
use tokio::{signal, task};
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

#[derive(Clone)]
struct WorkerContext {
    backfill_channel_concurrency: usize,
    backfill_page_delay: Duration,
    discord_api_base_url: String,
    discord_http: Client,
    discord_limiter: Arc<DiscordRequestLimiter>,
    discord_token: String,
    pool: PgPool,
    queue: RedisEventQueue,
    store: PostgresRawEventStore,
}

struct DiscordRequestLimiter {
    notify: Notify,
    state: Mutex<DiscordRequestLimiterState>,
}

struct DiscordRequestLimiterState {
    active_requests: usize,
    backoff_until: Option<Instant>,
    current_limit: usize,
    max_limit: usize,
    min_limit: usize,
    success_streak: usize,
}

struct DiscordRequestPermit {
    limiter: Arc<DiscordRequestLimiter>,
}

impl DiscordRequestLimiter {
    fn new(max_limit: usize) -> Self {
        Self {
            notify: Notify::new(),
            state: Mutex::new(DiscordRequestLimiterState {
                active_requests: 0,
                backoff_until: None,
                current_limit: max_limit.max(1),
                max_limit: max_limit.max(1),
                min_limit: 1,
                success_streak: 0,
            }),
        }
    }

    async fn acquire(self: &Arc<Self>) -> DiscordRequestPermit {
        loop {
            let wait_duration = {
                let mut state = self.state.lock().await;
                if let Some(backoff_until) = state.backoff_until {
                    let now = Instant::now();
                    if backoff_until > now {
                        Some(backoff_until.saturating_duration_since(now))
                    } else {
                        state.backoff_until = None;
                        None
                    }
                } else if state.active_requests < state.current_limit {
                    state.active_requests += 1;
                    return DiscordRequestPermit {
                        limiter: Arc::clone(self),
                    };
                } else {
                    None
                }
            };

            if let Some(duration) = wait_duration {
                tokio::time::sleep(duration).await;
            } else {
                self.notify.notified().await;
            }
        }
    }

    async fn on_success(&self) {
        let mut state = self.state.lock().await;
        state.success_streak += 1;
        if state.current_limit < state.max_limit && state.success_streak >= state.current_limit * 8
        {
            state.current_limit += 1;
            state.success_streak = 0;
            self.notify.notify_waiters();
        }
    }

    async fn on_rate_limit(&self, retry_after: Duration) {
        let mut state = self.state.lock().await;
        state.success_streak = 0;
        state.current_limit = (state.current_limit / 2).max(state.min_limit);
        let next_backoff = Instant::now() + retry_after;
        state.backoff_until = Some(match state.backoff_until {
            Some(current) if current > next_backoff => current,
            _ => next_backoff,
        });
        self.notify.notify_waiters();
    }

    async fn release(&self) {
        let mut state = self.state.lock().await;
        if state.active_requests > 0 {
            state.active_requests -= 1;
        }
        drop(state);
        self.notify.notify_waiters();
    }
}

impl Drop for DiscordRequestPermit {
    fn drop(&mut self) {
        let limiter = Arc::clone(&self.limiter);
        tokio::spawn(async move {
            limiter.release().await;
        });
    }
}

struct WorkerMetrics {
    backfill_job_duration_seconds: HistogramVec,
    dead_lettered_deliveries_total: IntCounterVec,
    discord_request_duration_seconds: HistogramVec,
    discord_response_size_bytes: HistogramVec,
    discord_requests_total: IntCounterVec,
    messages_indexed_per_backfill_job: Histogram,
    queue_dead_letter_depth: IntGaugeVec,
    queue_oldest_dead_letter_age_seconds: IntGaugeVec,
    queue_oldest_ready_age_seconds: IntGaugeVec,
    queue_pending_messages: IntGaugeVec,
    queue_ready_messages: IntGaugeVec,
    queue_scheduled_retry_depth: IntGaugeVec,
    queue_scheduled_retry_overdue_seconds: IntGaugeVec,
    retried_deliveries_total: IntCounterVec,
    registry: Registry,
}

#[derive(Debug)]
struct IndexedMessage {
    attachment_count: i32,
    author_id: String,
    channel_id: String,
    content_length: i32,
    guild_id: String,
    is_bot: bool,
    is_reply: bool,
    message_id: String,
    occurred_at: DateTime<Utc>,
    source: &'static str,
}

#[derive(Debug, FromRow)]
struct InsertedIndexedMessageRow {
    attachment_count: i32,
    author_id: String,
    channel_id: String,
    content_length: i32,
    guild_id: String,
    is_bot: bool,
    is_reply: bool,
    message_id: String,
    occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
struct BackfillRebuildScope {
    activity_end_at: DateTime<Utc>,
    activity_start_at: DateTime<Utc>,
    activity_start_date: NaiveDate,
    activity_end_date: NaiveDate,
    cohort_start_date: NaiveDate,
    cohort_end_date: NaiveDate,
    guild_id: String,
}

impl BackfillRebuildScope {
    fn new(
        guild_id: String,
        activity_start_at: DateTime<Utc>,
        activity_end_at: DateTime<Utc>,
    ) -> Self {
        let activity_start_date = date_for(activity_start_at);
        let activity_end_date = date_for(activity_end_at);
        let cohort_start_date = activity_start_date
            .checked_sub_days(Days::new(36))
            .unwrap_or(activity_start_date);

        Self {
            activity_end_at,
            activity_start_at,
            activity_start_date,
            activity_end_date,
            cohort_start_date,
            cohort_end_date: activity_end_date,
            guild_id,
        }
    }
}

#[derive(Debug, FromRow)]
struct ActiveVoiceSessionRow {
    channel_id: String,
    started_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct DiscordAuthor {
    bot: Option<bool>,
    id: String,
}

#[derive(Debug, Clone, Deserialize)]
struct DiscordChannel {
    id: String,
    name: Option<String>,
    #[serde(rename = "type")]
    kind: i32,
}

#[derive(Debug, Deserialize)]
struct DiscordMessage {
    attachments: Vec<serde_json::Value>,
    author: DiscordAuthor,
    content: String,
    id: String,
    message_reference: Option<serde_json::Value>,
    referenced_message: Option<serde_json::Value>,
    timestamp: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct PreviousChannelMessageRow {
    author_id: String,
    occurred_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct ChannelActivityAggregateRow {
    message_count: i64,
    replies: i64,
    response_samples: i64,
    response_seconds_total: i64,
    unique_senders: i64,
}

#[derive(Debug, FromRow)]
struct MemberChannelRow {
    channel_id: String,
}

#[derive(Debug, Deserialize)]
struct DiscordRateLimitResponse {
    retry_after: f64,
}

#[derive(Debug, Serialize)]
struct DeadLetterDelivery {
    attempts: i64,
    delivery_id: String,
    error: String,
    failed_at: DateTime<Utc>,
    payload: String,
    retry_key: String,
    source_stream: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ScheduledRetryDelivery {
    attempts: i64,
    delivery: QueueDelivery,
    retry_key: String,
    scheduled_at: DateTime<Utc>,
}

const MAX_DELIVERY_RETRIES: i64 = 3;
const DELIVERY_RETRY_TTL_SECONDS: u64 = 60 * 60;
const DELIVERY_RETRY_BASE_DELAY_MS: u64 = 2_000;
const RETRY_SCHEDULER_BATCH_SIZE: usize = 50;
const RETRY_SCHEDULER_POLL_MS: u64 = 500;

#[tokio::main]
async fn main() -> Result<()> {
    let settings = Settings::from_env()?;
    init_tracing(&settings.rust_log);

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&settings.database_url)
        .await
        .context("failed to connect to postgres")?;

    ensure_analytics_schema(&pool).await?;

    let queue = RedisEventQueue::new(&settings.redis_url)?;
    let store = PostgresRawEventStore::new(pool.clone());
    for stream in stream_names() {
        queue.create_consumer_group(stream, stream).await?;
    }

    let ctx = Arc::new(WorkerContext {
        backfill_channel_concurrency: settings.worker_backfill_channel_concurrency.max(1),
        backfill_page_delay: Duration::from_millis(settings.worker_backfill_page_delay_ms),
        discord_api_base_url: settings.discord_api_base_url.clone(),
        discord_http: Client::new(),
        discord_limiter: Arc::new(DiscordRequestLimiter::new(
            settings.worker_backfill_channel_concurrency.max(1),
        )),
        discord_token: settings.discord_token,
        pool,
        queue,
        store,
    });
    let mut tasks = Vec::new();
    let metrics_addr: SocketAddr = settings
        .worker_metrics_bind_addr
        .parse()
        .context("invalid WORKER_METRICS_BIND_ADDR")?;
    tasks.push(task::spawn(async move {
        if let Err(error) = run_metrics_server(metrics_addr).await {
            error!(?error, "metrics server exited");
        }
    }));

    for stream in stream_names() {
        let task_ctx = Arc::clone(&ctx);
        let consumer = format!(
            "{}-{}",
            settings.worker_consumer_prefix,
            stream.replace('.', "-")
        );
        let stream_name = stream.to_string();

        tasks.push(task::spawn(async move {
            if let Err(error) = run_stream_worker(task_ctx, stream_name, consumer).await {
                error!(?error, "stream worker exited");
            }
        }));

        let retry_ctx = Arc::clone(&ctx);
        let retry_stream = stream.to_string();
        tasks.push(task::spawn(async move {
            if let Err(error) = run_retry_scheduler(retry_ctx, retry_stream).await {
                error!(?error, "retry scheduler exited");
            }
        }));
    }

    signal::ctrl_c()
        .await
        .context("failed to listen for shutdown signal")?;
    info!("shutdown signal received");

    for task in tasks {
        task.abort();
    }

    Ok(())
}

async fn run_stream_worker(
    ctx: Arc<WorkerContext>,
    stream: String,
    consumer: String,
) -> Result<()> {
    loop {
        if let Err(error) = refresh_queue_metrics(&ctx.queue, &stream).await {
            error!(stream = %stream, ?error, "failed to refresh queue metrics");
        }

        let deliveries = ctx
            .queue
            .consume(&[stream.as_str()], &stream, &consumer, 50, 5_000)
            .await?;

        if deliveries.is_empty() {
            tokio::time::sleep(Duration::from_millis(250)).await;
            continue;
        }

        for delivery in deliveries {
            match process_delivery(&ctx, &delivery).await {
                Ok(()) => {
                    ctx.queue
                        .ack(&delivery.stream, &stream, &delivery.id)
                        .await?;
                }
                Err(error) => {
                    error!(stream = %delivery.stream, delivery_id = %delivery.id, ?error, "failed to process delivery");
                    if let Err(dead_letter_error) =
                        handle_failed_delivery(&ctx.queue, &delivery, &error).await
                    {
                        error!(
                            stream = %delivery.stream,
                            delivery_id = %delivery.id,
                            ?dead_letter_error,
                            "failed to handle failed delivery"
                        );
                        continue;
                    }
                    ctx.queue
                        .ack(&delivery.stream, &stream, &delivery.id)
                        .await?;
                }
            }
        }

        if let Err(error) = refresh_queue_metrics(&ctx.queue, &stream).await {
            error!(stream = %stream, ?error, "failed to refresh queue metrics");
        }
    }
}

async fn run_retry_scheduler(ctx: Arc<WorkerContext>, stream: String) -> Result<()> {
    loop {
        if let Err(error) = publish_due_retries(&ctx.queue, &stream).await {
            error!(stream = %stream, ?error, "failed to publish due retries");
        }

        tokio::time::sleep(Duration::from_millis(RETRY_SCHEDULER_POLL_MS)).await;
    }
}

async fn process_delivery(ctx: &WorkerContext, delivery: &QueueDelivery) -> Result<()> {
    if delivery.stream == BACKFILL_STREAM {
        let job: BackfillJob =
            serde_json::from_str(&delivery.payload).context("failed to decode backfill job")?;
        process_backfill_job(ctx, &job).await?;
        return Ok(());
    }

    let envelope = if let Ok(event_ref) = serde_json::from_str::<QueuedEventRef>(&delivery.payload)
    {
        ctx.store
            .find_by_id(event_ref.raw_event_id)
            .await?
            .with_context(|| {
                format!(
                    "raw event {} missing for queued delivery {}",
                    event_ref.raw_event_id, delivery.id
                )
            })?
    } else {
        serde_json::from_str(&delivery.payload).context("failed to decode event envelope")?
    };
    process_event(ctx, &envelope).await
}

async fn handle_failed_delivery(
    queue: &RedisEventQueue,
    delivery: &QueueDelivery,
    error: &anyhow::Error,
) -> Result<()> {
    let retry_key = retry_state_key(delivery)?;
    let attempts = queue
        .incr_with_ttl(&retry_key, DELIVERY_RETRY_TTL_SECONDS)
        .await?;

    if attempts <= MAX_DELIVERY_RETRIES {
        schedule_retry_delivery(queue, delivery.clone(), &retry_key, attempts).await?;
        return Ok(());
    }

    dead_letter_delivery(queue, delivery, error, &retry_key, attempts).await?;
    queue.del_key(&retry_key).await?;
    Ok(())
}

async fn schedule_retry_delivery(
    queue: &RedisEventQueue,
    delivery: QueueDelivery,
    retry_key: &str,
    attempts: i64,
) -> Result<()> {
    let delay = retry_backoff_delay(attempts);
    let scheduled_at = Utc::now()
        + chrono::Duration::from_std(delay).unwrap_or_else(|_| chrono::Duration::seconds(2));
    let payload = serde_json::to_string(&ScheduledRetryDelivery {
        attempts,
        delivery: delivery.clone(),
        retry_key: retry_key.to_string(),
        scheduled_at,
    })
    .context("failed to encode scheduled retry delivery")?;
    queue
        .schedule_message(
            &scheduled_retry_set_name(&delivery.stream),
            &payload,
            scheduled_at.timestamp_millis(),
        )
        .await?;
    if let Err(refresh_error) = refresh_queue_metrics(queue, &delivery.stream).await {
        error!(
            stream = %delivery.stream,
            ?refresh_error,
            "failed to refresh queue metrics after scheduling retry"
        );
    }
    Ok(())
}

async fn dead_letter_delivery(
    queue: &RedisEventQueue,
    delivery: &QueueDelivery,
    error: &anyhow::Error,
    retry_key: &str,
    attempts: i64,
) -> Result<()> {
    let payload = serde_json::to_string(&DeadLetterDelivery {
        attempts,
        delivery_id: delivery.id.clone(),
        error: format!("{error:#}"),
        failed_at: Utc::now(),
        payload: delivery.payload.clone(),
        retry_key: retry_key.to_string(),
        source_stream: delivery.stream.clone(),
    })
    .context("failed to encode dead-letter payload")?;
    let dead_letter_stream = dead_letter_stream_name(&delivery.stream);
    queue.publish(&dead_letter_stream, &payload).await?;
    observe_dead_lettered_delivery(&delivery.stream);
    if let Err(error) = refresh_queue_metrics(queue, &delivery.stream).await {
        error!(stream = %delivery.stream, ?error, "failed to refresh queue metrics after dead-letter");
    }
    Ok(())
}

async fn process_event(ctx: &WorkerContext, envelope: &EventEnvelope) -> Result<()> {
    match &envelope.payload {
        EventPayload::GuildAvailable(payload) => {
            sqlx::query(
                r#"
                INSERT INTO guild_inventory (
                    guild_id,
                    guild_name,
                    owner_id,
                    member_count,
                    is_active,
                    last_seen_at
                )
                VALUES ($1, $2, $3, $4, TRUE, $5)
                ON CONFLICT (guild_id)
                DO UPDATE SET
                    guild_name = EXCLUDED.guild_name,
                    owner_id = EXCLUDED.owner_id,
                    member_count = EXCLUDED.member_count,
                    is_active = TRUE,
                    last_seen_at = EXCLUDED.last_seen_at
                "#,
            )
            .bind(&payload.guild_id)
            .bind(&payload.name)
            .bind(&payload.owner_id)
            .bind(payload.member_count)
            .bind(envelope.occurred_at)
            .execute(&ctx.pool)
            .await
            .context("failed to upsert guild inventory")?;
            refresh_public_stats_cache(&ctx.pool).await?;

            if payload.is_new {
                enqueue_install_backfill(ctx, &payload.guild_id, Some(payload.owner_id.clone()))
                    .await?;
            }
        }
        EventPayload::GuildRemoved(payload) => {
            sqlx::query(
                r#"
                INSERT INTO guild_inventory (
                    guild_id,
                    guild_name,
                    owner_id,
                    member_count,
                    is_active,
                    last_seen_at
                )
                VALUES ($1, '', '', 0, FALSE, $2)
                ON CONFLICT (guild_id)
                DO UPDATE SET
                    is_active = FALSE,
                    last_seen_at = EXCLUDED.last_seen_at
                "#,
            )
            .bind(&payload.guild_id)
            .bind(envelope.occurred_at)
            .execute(&ctx.pool)
            .await
            .context("failed to mark guild inactive")?;
            refresh_public_stats_cache(&ctx.pool).await?;
        }
        EventPayload::MemberJoined(payload) => {
            sqlx::query(
                r#"
                INSERT INTO member_lifecycle (
                    guild_id,
                    member_id,
                    joined_at,
                    first_seen_at,
                    first_role_at,
                    is_pending
                )
                VALUES ($1, $2, $3, $4, $5, $6)
                ON CONFLICT (guild_id, member_id)
                DO UPDATE SET
                    joined_at = COALESCE(member_lifecycle.joined_at, EXCLUDED.joined_at),
                    first_seen_at = LEAST(member_lifecycle.first_seen_at, EXCLUDED.first_seen_at),
                    first_role_at = COALESCE(member_lifecycle.first_role_at, EXCLUDED.first_role_at),
                    is_pending = EXCLUDED.is_pending
                "#,
            )
            .bind(&envelope.guild_id)
            .bind(&payload.member_id)
            .bind(payload.joined_at)
            .bind(envelope.occurred_at)
            .bind(if payload.role_ids.is_empty() {
                None
            } else {
                Some(envelope.occurred_at)
            })
            .bind(payload.is_pending)
            .execute(&ctx.pool)
            .await
            .context("failed to upsert member lifecycle join")?;

            if let Some(joined_at) = payload.joined_at {
                if record_member_join(&ctx.pool, &envelope.guild_id, &payload.member_id, joined_at)
                    .await?
                {
                    increment_activation_funnel_counter(
                        &ctx.pool,
                        &envelope.guild_id,
                        date_for(joined_at),
                        "joined_members",
                    )
                    .await?;
                }
            }

            if !payload.role_ids.is_empty() {
                maybe_record_activation_step(
                    &ctx.pool,
                    &envelope.guild_id,
                    &payload.member_id,
                    "activation_funnel_role_members",
                    "first_role_at",
                    "got_role_members",
                )
                .await?;
            }
        }
        EventPayload::MemberLeft(payload) => {
            sqlx::query(
                r#"
                INSERT INTO member_lifecycle (
                    guild_id,
                    member_id,
                    first_seen_at,
                    left_at
                )
                VALUES ($1, $2, $3, $3)
                ON CONFLICT (guild_id, member_id)
                DO UPDATE SET
                    left_at = EXCLUDED.left_at
                "#,
            )
            .bind(&envelope.guild_id)
            .bind(&payload.member_id)
            .bind(envelope.occurred_at)
            .execute(&ctx.pool)
            .await
            .context("failed to upsert member lifecycle leave")?;

            record_member_leave(
                &ctx.pool,
                &envelope.guild_id,
                &payload.member_id,
                envelope.occurred_at,
            )
            .await?;
        }
        EventPayload::MemberRolesUpdated(payload) => {
            let first_role_at = if payload.current_role_ids.is_empty() {
                None
            } else {
                Some(envelope.occurred_at)
            };

            sqlx::query(
                r#"
                INSERT INTO member_lifecycle (
                    guild_id,
                    member_id,
                    first_seen_at,
                    first_role_at,
                    is_pending
                )
                VALUES ($1, $2, $3, $4, $5)
                ON CONFLICT (guild_id, member_id)
                DO UPDATE SET
                    first_role_at = COALESCE(member_lifecycle.first_role_at, EXCLUDED.first_role_at),
                    is_pending = EXCLUDED.is_pending
                "#,
            )
            .bind(&envelope.guild_id)
            .bind(&payload.member_id)
            .bind(envelope.occurred_at)
            .bind(first_role_at)
            .bind(payload.is_pending)
            .execute(&ctx.pool)
            .await
            .context("failed to upsert member role milestone")?;

            maybe_record_onboarding_completion(&ctx.pool, &envelope.guild_id, &payload.member_id)
                .await?;
            maybe_record_activation_step(
                &ctx.pool,
                &envelope.guild_id,
                &payload.member_id,
                "activation_funnel_role_members",
                "first_role_at",
                "got_role_members",
            )
            .await?;
        }
        EventPayload::MessageCreated(payload) => {
            let Some(channel_id) = envelope.channel_id.clone() else {
                return Ok(());
            };
            if let Err(error) =
                ensure_channel_inventory_entry(ctx, &envelope.guild_id, &channel_id).await
            {
                warn!(
                    guild_id = %envelope.guild_id,
                    channel_id = %channel_id,
                    ?error,
                    "failed to resolve channel name"
                );
            }

            let indexed = IndexedMessage {
                attachment_count: payload.attachment_count,
                author_id: payload.author_id.clone(),
                channel_id,
                content_length: payload.content_length,
                guild_id: envelope.guild_id.clone(),
                is_bot: payload.is_bot,
                is_reply: payload.is_reply,
                message_id: payload.message_id.clone(),
                occurred_at: envelope.occurred_at,
                source: "gateway",
            };

            let inserted = insert_message_index(&ctx.pool, &indexed).await?;
            if inserted {
                increment_public_message_count(&ctx.pool).await?;
                apply_message_aggregates(&ctx.pool, &indexed).await?;
            }
        }
        EventPayload::ReactionAdded(payload) => {
            let activity_date = date_for(envelope.occurred_at);

            sqlx::query(
                r#"
                INSERT INTO member_daily_activity (
                    guild_id,
                    member_id,
                    activity_date,
                    messages_sent,
                    reactions_added,
                    voice_seconds,
                    active_channels,
                    was_active,
                    last_active_at,
                    last_channel_id
                )
                VALUES ($1, $2, $3, 0, 1, 0, 1, TRUE, $4, $5)
                ON CONFLICT (guild_id, member_id, activity_date)
                DO UPDATE SET
                    reactions_added = member_daily_activity.reactions_added + 1,
                    active_channels = member_daily_activity.active_channels + CASE
                        WHEN member_daily_activity.last_channel_id IS DISTINCT FROM EXCLUDED.last_channel_id THEN 1
                        ELSE 0
                    END,
                    was_active = TRUE,
                    last_active_at = GREATEST(member_daily_activity.last_active_at, EXCLUDED.last_active_at),
                    last_channel_id = EXCLUDED.last_channel_id
                "#,
            )
            .bind(&envelope.guild_id)
            .bind(&payload.user_id)
            .bind(activity_date)
            .bind(envelope.occurred_at)
            .bind(&envelope.channel_id)
            .execute(&ctx.pool)
            .await
            .context("failed to update reaction activity")?;

            mark_member_active_for_day(
                &ctx.pool,
                &envelope.guild_id,
                &payload.user_id,
                activity_date,
            )
            .await?;

            sqlx::query(
                r#"
                INSERT INTO member_lifecycle (
                    guild_id,
                    member_id,
                    first_seen_at,
                    first_reaction_at
                )
                VALUES ($1, $2, $3, $3)
                ON CONFLICT (guild_id, member_id)
                DO UPDATE SET
                    first_reaction_at = COALESCE(member_lifecycle.first_reaction_at, EXCLUDED.first_reaction_at)
                "#,
            )
            .bind(&envelope.guild_id)
            .bind(&payload.user_id)
            .bind(envelope.occurred_at)
            .execute(&ctx.pool)
            .await
            .context("failed to update first reaction timestamp")?;
            maybe_record_activation_step(
                &ctx.pool,
                &envelope.guild_id,
                &payload.user_id,
                "activation_funnel_reaction_members",
                "first_reaction_at",
                "first_reaction_members",
            )
            .await?;
        }
        EventPayload::VoiceStateUpdated(payload) => {
            sqlx::query(
                r#"
                INSERT INTO voice_state_events (
                    event_id,
                    guild_id,
                    member_id,
                    old_channel_id,
                    new_channel_id,
                    occurred_at
                )
                VALUES ($1, $2, $3, $4, $5, $6)
                ON CONFLICT (event_id) DO NOTHING
                "#,
            )
            .bind(envelope.event_id)
            .bind(&envelope.guild_id)
            .bind(&payload.member_id)
            .bind(&payload.old_channel_id)
            .bind(&payload.new_channel_id)
            .bind(envelope.occurred_at)
            .execute(&ctx.pool)
            .await
            .context("failed to persist voice state event")?;

            process_voice_state_update(
                &ctx.pool,
                &envelope.guild_id,
                &payload.member_id,
                payload.old_channel_id.as_deref(),
                payload.new_channel_id.as_deref(),
                envelope.occurred_at,
            )
            .await?;
        }
    }

    Ok(())
}

async fn enqueue_install_backfill(
    ctx: &WorkerContext,
    guild_id: &str,
    requested_by_user_id: Option<String>,
) -> Result<()> {
    let end_at = Utc::now();
    let start_at = end_at
        .checked_sub_days(Days::new(7))
        .unwrap_or(end_at - chrono::Duration::days(7));

    let job = BackfillJob::new(
        guild_id.to_string(),
        requested_by_user_id,
        7,
        start_at,
        end_at,
        "install",
    );
    enqueue_backfill_job(ctx, &job).await
}

async fn enqueue_backfill_job(ctx: &WorkerContext, job: &BackfillJob) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO historical_backfill_jobs (
            job_id,
            guild_id,
            requested_by_user_id,
            days_requested,
            start_at,
            end_at,
            trigger_source,
            status,
            requested_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, 'queued', $8)
        ON CONFLICT (job_id) DO NOTHING
        "#,
    )
    .bind(job.job_id)
    .bind(&job.guild_id)
    .bind(&job.requested_by_user_id)
    .bind(job.days_requested)
    .bind(job.start_at)
    .bind(job.end_at)
    .bind(&job.trigger_source)
    .bind(job.requested_at)
    .execute(&ctx.pool)
    .await
    .context("failed to persist backfill job")?;

    let payload = serde_json::to_string(job).context("failed to serialize backfill job")?;
    ctx.queue
        .publish(BACKFILL_STREAM, &payload)
        .await
        .context("failed to enqueue backfill job")?;

    Ok(())
}

async fn process_backfill_job(ctx: &WorkerContext, job: &BackfillJob) -> Result<()> {
    let started = Instant::now();
    sqlx::query(
        r#"
        UPDATE historical_backfill_jobs
        SET status = 'running',
            started_at = NOW(),
            last_error = NULL
        WHERE job_id = $1
        "#,
    )
    .bind(job.job_id)
    .execute(&ctx.pool)
    .await
    .context("failed to mark backfill job running")?;

    let result = run_backfill_job(ctx, job).await;
    match result {
        Ok(messages_indexed) => {
            observe_backfill_job(
                "completed",
                started.elapsed().as_secs_f64(),
                messages_indexed,
            );
            sqlx::query(
                r#"
                UPDATE historical_backfill_jobs
                SET status = 'completed',
                    completed_at = NOW(),
                    messages_indexed = $2,
                    last_error = NULL
                WHERE job_id = $1
                "#,
            )
            .bind(job.job_id)
            .bind(messages_indexed)
            .execute(&ctx.pool)
            .await
            .context("failed to mark backfill job completed")?;
            Ok(())
        }
        Err(error) => {
            observe_backfill_job("failed", started.elapsed().as_secs_f64(), 0);
            sqlx::query(
                r#"
                UPDATE historical_backfill_jobs
                SET status = 'failed',
                    completed_at = NOW(),
                    last_error = $2
                WHERE job_id = $1
                "#,
            )
            .bind(job.job_id)
            .bind(error.to_string())
            .execute(&ctx.pool)
            .await
            .context("failed to mark backfill job failed")?;
            Err(error)
        }
    }
}

async fn run_backfill_job(ctx: &WorkerContext, job: &BackfillJob) -> Result<i64> {
    let channels = fetch_guild_channels(ctx, &job.guild_id).await?;
    upsert_channel_inventory(&ctx.pool, &job.guild_id, &channels).await?;
    let mut inserted_count = 0_i64;

    let backfillable_channels = channels
        .into_iter()
        .filter(is_message_backfillable_channel)
        .collect::<Vec<_>>();
    let max_in_flight = ctx
        .backfill_channel_concurrency
        .min(backfillable_channels.len())
        .max(1);
    let shared_ctx = Arc::new(ctx.clone());
    let mut next_channel_index = 0usize;
    let mut join_set = task::JoinSet::new();

    while next_channel_index < max_in_flight {
        let task_ctx = Arc::clone(&shared_ctx);
        let channel = backfillable_channels[next_channel_index].clone();
        let guild_id = job.guild_id.clone();
        let start_at = job.start_at;
        let end_at = job.end_at;
        join_set.spawn(async move {
            run_backfill_channel(task_ctx.as_ref(), guild_id, start_at, end_at, channel).await
        });
        next_channel_index += 1;
    }

    while let Some(result) = join_set.join_next().await {
        inserted_count += result.context("backfill channel task panicked")??;

        if next_channel_index < backfillable_channels.len() {
            let task_ctx = Arc::clone(&shared_ctx);
            let channel = backfillable_channels[next_channel_index].clone();
            let guild_id = job.guild_id.clone();
            let start_at = job.start_at;
            let end_at = job.end_at;
            join_set.spawn(async move {
                run_backfill_channel(task_ctx.as_ref(), guild_id, start_at, end_at, channel).await
            });
            next_channel_index += 1;
        }
    }

    let rebuild_scope = BackfillRebuildScope::new(job.guild_id.clone(), job.start_at, job.end_at);
    rebuild_message_analytics_for_scope(&ctx.pool, &ctx.queue, &rebuild_scope).await?;

    Ok(inserted_count)
}

async fn run_backfill_channel(
    ctx: &WorkerContext,
    guild_id: String,
    start_at: DateTime<Utc>,
    end_at: DateTime<Utc>,
    channel: DiscordChannel,
) -> Result<i64> {
    let mut inserted_count = 0_i64;
    let mut before: Option<String> = None;

    loop {
        let messages = fetch_channel_messages(ctx, &channel.id, before.as_deref()).await?;
        if messages.is_empty() {
            break;
        }

        let mut reached_start = false;
        let mut page_messages = Vec::new();
        for message in &messages {
            if message.timestamp < start_at {
                reached_start = true;
                continue;
            }

            if message.timestamp > end_at {
                continue;
            }

            page_messages.push(IndexedMessage {
                attachment_count: i32::try_from(message.attachments.len()).unwrap_or(i32::MAX),
                author_id: message.author.id.clone(),
                channel_id: channel.id.clone(),
                content_length: i32::try_from(message.content.chars().count()).unwrap_or(i32::MAX),
                guild_id: guild_id.clone(),
                is_bot: message.author.bot.unwrap_or(false),
                is_reply: message.referenced_message.is_some()
                    || message.message_reference.is_some(),
                message_id: message.id.clone(),
                occurred_at: message.timestamp,
                source: "backfill",
            });
        }

        let inserted_messages = insert_message_index_batch(&ctx.pool, &page_messages).await?;
        if !inserted_messages.is_empty() {
            increment_public_message_count_by(&ctx.pool, inserted_messages.len() as i64).await?;
            inserted_count += i64::try_from(inserted_messages.len()).unwrap_or(i64::MAX);
        }

        if reached_start || messages.len() < 100 {
            break;
        }

        before = messages.last().map(|message| message.id.clone());
        if !ctx.backfill_page_delay.is_zero() {
            tokio::time::sleep(ctx.backfill_page_delay).await;
        }
    }

    Ok(inserted_count)
}

async fn fetch_guild_channels(ctx: &WorkerContext, guild_id: &str) -> Result<Vec<DiscordChannel>> {
    discord_get_json(
        ctx,
        &discord_api_url(
            &ctx.discord_api_base_url,
            &format!("/guilds/{guild_id}/channels"),
        )?,
    )
    .await
}

async fn ensure_channel_inventory_entry(
    ctx: &WorkerContext,
    guild_id: &str,
    channel_id: &str,
) -> Result<()> {
    let existing_name = sqlx::query_scalar::<_, String>(
        r#"
        SELECT channel_name
        FROM channel_inventory
        WHERE guild_id = $1
          AND channel_id = $2
        LIMIT 1
        "#,
    )
    .bind(guild_id)
    .bind(channel_id)
    .fetch_optional(&ctx.pool)
    .await
    .context("failed to check channel inventory")?;
    if existing_name.is_some() {
        return Ok(());
    }

    let channel = fetch_channel(ctx, channel_id).await?;
    upsert_channel_inventory(&ctx.pool, guild_id, &[channel]).await
}

async fn fetch_channel(ctx: &WorkerContext, channel_id: &str) -> Result<DiscordChannel> {
    discord_get_json(
        ctx,
        &discord_api_url(
            &ctx.discord_api_base_url,
            &format!("/channels/{channel_id}"),
        )?,
    )
    .await
}

async fn upsert_channel_inventory(
    pool: &PgPool,
    guild_id: &str,
    channels: &[DiscordChannel],
) -> Result<()> {
    for channel in channels {
        let Some(channel_name) = channel.name.as_deref().map(str::trim) else {
            continue;
        };
        if channel_name.is_empty() {
            continue;
        }

        sqlx::query(
            r#"
            INSERT INTO channel_inventory (
                guild_id,
                channel_id,
                channel_name,
                channel_kind,
                last_synced_at
            )
            VALUES ($1, $2, $3, $4, NOW())
            ON CONFLICT (guild_id, channel_id) DO UPDATE
            SET channel_name = EXCLUDED.channel_name,
                channel_kind = EXCLUDED.channel_kind,
                last_synced_at = NOW()
            "#,
        )
        .bind(guild_id)
        .bind(&channel.id)
        .bind(channel_name)
        .bind(channel.kind)
        .execute(pool)
        .await
        .with_context(|| format!("failed to upsert channel inventory for {}", channel.id))?;
    }

    Ok(())
}

async fn fetch_channel_messages(
    ctx: &WorkerContext,
    channel_id: &str,
    before: Option<&str>,
) -> Result<Vec<DiscordMessage>> {
    let mut url = reqwest::Url::parse(&discord_api_url(
        &ctx.discord_api_base_url,
        &format!("/channels/{channel_id}/messages"),
    )?)
    .context("invalid discord channel messages url")?;
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("limit", "100");
        if let Some(before) = before {
            query.append_pair("before", before);
        }
    }

    discord_get_json(ctx, url.as_str()).await
}

async fn discord_get_json<T>(ctx: &WorkerContext, url: &str) -> Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    let mut attempts = 0usize;
    let endpoint = discord_endpoint_label(url);
    loop {
        let _permit = ctx.discord_limiter.acquire().await;
        let started = Instant::now();
        let response = ctx
            .discord_http
            .get(url)
            .header("Authorization", format!("Bot {}", ctx.discord_token))
            .send()
            .await
            .with_context(|| format!("failed to call discord endpoint {url}"))?;
        let status = response.status();
        let body_bytes = response
            .bytes()
            .await
            .with_context(|| format!("failed to read discord response body from {url}"))?;
        observe_discord_request(
            endpoint,
            status.as_u16(),
            started.elapsed().as_secs_f64(),
            body_bytes.len(),
        );

        if status == StatusCode::TOO_MANY_REQUESTS && attempts < 3 {
            let body = serde_json::from_slice::<DiscordRateLimitResponse>(&body_bytes)
                .context("failed to parse discord rate limit response")?;
            let delay_ms = (body.retry_after * 1000.0).ceil() as u64;
            ctx.discord_limiter
                .on_rate_limit(Duration::from_millis(delay_ms.max(250)))
                .await;
            tokio::time::sleep(Duration::from_millis(delay_ms.max(250))).await;
            attempts += 1;
            continue;
        }

        if !status.is_success() {
            let body = String::from_utf8_lossy(&body_bytes);
            anyhow::bail!("discord request failed with {status}: {body}");
        }

        ctx.discord_limiter.on_success().await;
        return serde_json::from_slice::<T>(&body_bytes)
            .with_context(|| format!("failed to decode discord response from {url}"));
    }
}

async fn insert_message_index(pool: &PgPool, message: &IndexedMessage) -> Result<bool> {
    let inserted = sqlx::query_scalar(
        r#"
        WITH inserted AS (
            INSERT INTO message_index (
                message_id,
                guild_id,
                channel_id,
                author_id,
                is_bot,
                is_reply,
                attachment_count,
                content_length,
                occurred_at,
                source
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (message_id) DO NOTHING
            RETURNING 1
        )
        SELECT EXISTS(SELECT 1 FROM inserted)
        "#,
    )
    .bind(&message.message_id)
    .bind(&message.guild_id)
    .bind(&message.channel_id)
    .bind(&message.author_id)
    .bind(message.is_bot)
    .bind(message.is_reply)
    .bind(message.attachment_count)
    .bind(message.content_length)
    .bind(message.occurred_at)
    .bind(message.source)
    .fetch_one(pool)
    .await
    .context("failed to upsert message index")?;

    Ok(inserted)
}

async fn insert_message_index_batch(
    pool: &PgPool,
    messages: &[IndexedMessage],
) -> Result<Vec<IndexedMessage>> {
    if messages.is_empty() {
        return Ok(Vec::new());
    }

    let mut message_ids = Vec::with_capacity(messages.len());
    let mut guild_ids = Vec::with_capacity(messages.len());
    let mut channel_ids = Vec::with_capacity(messages.len());
    let mut author_ids = Vec::with_capacity(messages.len());
    let mut is_bots = Vec::with_capacity(messages.len());
    let mut is_replies = Vec::with_capacity(messages.len());
    let mut attachment_counts = Vec::with_capacity(messages.len());
    let mut content_lengths = Vec::with_capacity(messages.len());
    let mut occurred_ats = Vec::with_capacity(messages.len());
    let mut sources = Vec::with_capacity(messages.len());

    for message in messages {
        message_ids.push(message.message_id.clone());
        guild_ids.push(message.guild_id.clone());
        channel_ids.push(message.channel_id.clone());
        author_ids.push(message.author_id.clone());
        is_bots.push(message.is_bot);
        is_replies.push(message.is_reply);
        attachment_counts.push(message.attachment_count);
        content_lengths.push(message.content_length);
        occurred_ats.push(message.occurred_at);
        sources.push(message.source.to_string());
    }

    let inserted_rows = sqlx::query_as::<_, InsertedIndexedMessageRow>(
        r#"
        WITH input AS (
            SELECT *
            FROM UNNEST(
                $1::TEXT[],
                $2::TEXT[],
                $3::TEXT[],
                $4::TEXT[],
                $5::BOOLEAN[],
                $6::BOOLEAN[],
                $7::INTEGER[],
                $8::INTEGER[],
                $9::TIMESTAMPTZ[],
                $10::TEXT[]
            ) AS batch (
                message_id,
                guild_id,
                channel_id,
                author_id,
                is_bot,
                is_reply,
                attachment_count,
                content_length,
                occurred_at,
                source
            )
        ),
        inserted AS (
            INSERT INTO message_index (
                message_id,
                guild_id,
                channel_id,
                author_id,
                is_bot,
                is_reply,
                attachment_count,
                content_length,
                occurred_at,
                source
            )
            SELECT
                message_id,
                guild_id,
                channel_id,
                author_id,
                is_bot,
                is_reply,
                attachment_count,
                content_length,
                occurred_at,
                source
            FROM input
            ON CONFLICT (message_id) DO NOTHING
            RETURNING
                message_id,
                guild_id,
                channel_id,
                author_id,
                is_bot,
                is_reply,
                attachment_count,
                content_length,
                occurred_at
        )
        SELECT
            attachment_count,
            author_id,
            channel_id,
            content_length,
            guild_id,
            is_bot,
            is_reply,
            message_id,
            occurred_at
        FROM inserted
        ORDER BY occurred_at ASC, message_id ASC
        "#,
    )
    .bind(message_ids)
    .bind(guild_ids)
    .bind(channel_ids)
    .bind(author_ids)
    .bind(is_bots)
    .bind(is_replies)
    .bind(attachment_counts)
    .bind(content_lengths)
    .bind(occurred_ats)
    .bind(sources)
    .fetch_all(pool)
    .await
    .context("failed to batch upsert message index")?;

    Ok(inserted_rows
        .into_iter()
        .map(|row| IndexedMessage {
            attachment_count: row.attachment_count,
            author_id: row.author_id,
            channel_id: row.channel_id,
            content_length: row.content_length,
            guild_id: row.guild_id,
            is_bot: row.is_bot,
            is_reply: row.is_reply,
            message_id: row.message_id,
            occurred_at: row.occurred_at,
            source: "backfill",
        })
        .collect())
}

async fn apply_message_aggregates(pool: &PgPool, message: &IndexedMessage) -> Result<()> {
    update_guild_daily_activity(pool, message).await?;

    if message.is_bot {
        return Ok(());
    }

    let activity_date = date_for(message.occurred_at);

    sqlx::query(
        r#"
        INSERT INTO member_lifecycle (
            guild_id,
            member_id,
            first_seen_at,
            first_message_at
        )
        VALUES ($1, $2, $3, $3)
        ON CONFLICT (guild_id, member_id)
        DO UPDATE SET
            first_message_at = COALESCE(member_lifecycle.first_message_at, EXCLUDED.first_message_at)
        "#,
    )
    .bind(&message.guild_id)
    .bind(&message.author_id)
    .bind(message.occurred_at)
    .execute(pool)
    .await
    .context("failed to record first message")?;

    maybe_record_onboarding_completion(pool, &message.guild_id, &message.author_id).await?;
    maybe_record_activation_step(
        pool,
        &message.guild_id,
        &message.author_id,
        "activation_funnel_message_members",
        "first_message_at",
        "first_message_members",
    )
    .await?;

    sqlx::query(
        r#"
        INSERT INTO member_daily_activity (
            guild_id,
            member_id,
            activity_date,
            messages_sent,
            reactions_added,
            active_channels,
            last_active_at,
            last_channel_id
        )
        VALUES ($1, $2, $3, 1, 0, 1, $4, $5)
        ON CONFLICT (guild_id, member_id, activity_date)
        DO UPDATE SET
            messages_sent = member_daily_activity.messages_sent + 1,
            active_channels = member_daily_activity.active_channels + CASE
                WHEN member_daily_activity.last_channel_id IS DISTINCT FROM EXCLUDED.last_channel_id THEN 1
                ELSE 0
            END,
            last_channel_id = EXCLUDED.last_channel_id,
            last_active_at = GREATEST(member_daily_activity.last_active_at, EXCLUDED.last_active_at)
        "#,
    )
    .bind(&message.guild_id)
    .bind(&message.author_id)
    .bind(activity_date)
    .bind(message.occurred_at)
    .bind(&message.channel_id)
    .execute(pool)
    .await
    .context("failed to update member daily activity")?;

    mark_member_active_for_day(pool, &message.guild_id, &message.author_id, activity_date).await?;

    let sender_was_new = sqlx::query(
        r#"
        INSERT INTO channel_daily_unique_senders (
            guild_id,
            channel_id,
            activity_date,
            member_id
        )
        VALUES ($1, $2, $3, $4)
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(&message.guild_id)
    .bind(&message.channel_id)
    .bind(activity_date)
    .bind(&message.author_id)
    .execute(pool)
    .await
    .context("failed to insert unique sender marker")?
    .rows_affected()
        > 0;

    let response_seconds = fetch_response_time_sample(pool, message).await?;
    let response_sample_count = if response_seconds.is_some() { 1 } else { 0 };

    sqlx::query(
        r#"
        INSERT INTO channel_daily_activity (
            guild_id,
            channel_id,
            activity_date,
            messages,
            unique_senders,
            replies,
            response_seconds_total,
            response_samples,
            last_message_at
        )
        VALUES ($1, $2, $3, 1, $4, $5, $6, $7, $8)
        ON CONFLICT (guild_id, channel_id, activity_date)
        DO UPDATE SET
            messages = channel_daily_activity.messages + 1,
            unique_senders = channel_daily_activity.unique_senders + $4,
            replies = channel_daily_activity.replies + $5,
            response_seconds_total = channel_daily_activity.response_seconds_total + $6,
            response_samples = channel_daily_activity.response_samples + $7,
            last_message_at = GREATEST(channel_daily_activity.last_message_at, EXCLUDED.last_message_at)
        "#,
    )
    .bind(&message.guild_id)
    .bind(&message.channel_id)
    .bind(activity_date)
    .bind(if sender_was_new { 1 } else { 0 })
    .bind(if message.is_reply { 1 } else { 0 })
    .bind(response_seconds.unwrap_or(0))
    .bind(response_sample_count)
    .bind(message.occurred_at)
    .execute(pool)
    .await
    .context("failed to update channel daily activity")?;

    refresh_channel_health_daily(pool, &message.guild_id, &message.channel_id, activity_date)
        .await?;

    Ok(())
}

async fn refresh_channel_health_daily(
    pool: &PgPool,
    guild_id: &str,
    channel_id: &str,
    activity_date: NaiveDate,
) -> Result<()> {
    let current = sqlx::query_as::<_, ChannelActivityAggregateRow>(
        r#"
        SELECT
            messages::BIGINT AS message_count,
            unique_senders::BIGINT AS unique_senders,
            replies::BIGINT AS replies,
            response_seconds_total::BIGINT AS response_seconds_total,
            response_samples::BIGINT AS response_samples
        FROM channel_daily_activity
        WHERE guild_id = $1
          AND channel_id = $2
          AND activity_date = $3
        "#,
    )
    .bind(guild_id)
    .bind(channel_id)
    .bind(activity_date)
    .fetch_one(pool)
    .await
    .context("failed to load current channel activity aggregate")?;

    let previous_messages = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COALESCE(messages, 0)::BIGINT
        FROM channel_daily_activity
        WHERE guild_id = $1
          AND channel_id = $2
          AND activity_date = $3 - 1
        "#,
    )
    .bind(guild_id)
    .bind(channel_id)
    .bind(activity_date)
    .fetch_optional(pool)
    .await
    .context("failed to load previous channel activity aggregate")?
    .unwrap_or(0);

    let avg_response_seconds = if current.response_samples > 0 {
        Some(current.response_seconds_total as f64 / current.response_samples as f64)
    } else {
        None
    };
    let health_score = channel_health_score(
        current.message_count,
        current.unique_senders,
        current.replies,
        previous_messages,
        avg_response_seconds,
    );

    sqlx::query(
        r#"
        INSERT INTO channel_health_daily (
            guild_id,
            channel_id,
            activity_date,
            messages,
            unique_senders,
            replies,
            response_seconds_total,
            response_samples,
            previous_day_messages,
            health_score
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        ON CONFLICT (guild_id, channel_id, activity_date)
        DO UPDATE SET
            messages = EXCLUDED.messages,
            unique_senders = EXCLUDED.unique_senders,
            replies = EXCLUDED.replies,
            response_seconds_total = EXCLUDED.response_seconds_total,
            response_samples = EXCLUDED.response_samples,
            previous_day_messages = EXCLUDED.previous_day_messages,
            health_score = EXCLUDED.health_score
        "#,
    )
    .bind(guild_id)
    .bind(channel_id)
    .bind(activity_date)
    .bind(current.message_count)
    .bind(current.unique_senders)
    .bind(current.replies)
    .bind(current.response_seconds_total)
    .bind(current.response_samples)
    .bind(previous_messages)
    .bind(health_score)
    .execute(pool)
    .await
    .context("failed to refresh channel_health_daily")?;

    Ok(())
}

async fn fetch_response_time_sample(
    pool: &PgPool,
    message: &IndexedMessage,
) -> Result<Option<i64>> {
    let previous = sqlx::query_as::<_, PreviousChannelMessageRow>(
        r#"
        SELECT author_id, occurred_at
        FROM message_index
        WHERE guild_id = $1
          AND channel_id = $2
          AND occurred_at < $3
        ORDER BY occurred_at DESC
        LIMIT 1
        "#,
    )
    .bind(&message.guild_id)
    .bind(&message.channel_id)
    .bind(message.occurred_at)
    .fetch_optional(pool)
    .await
    .context("failed to fetch previous channel message")?;

    let Some(previous) = previous else {
        return Ok(None);
    };

    if previous.author_id == message.author_id {
        return Ok(None);
    }

    let delta = message
        .occurred_at
        .signed_duration_since(previous.occurred_at)
        .num_seconds();
    if (0..=86_400).contains(&delta) {
        Ok(Some(delta))
    } else {
        Ok(None)
    }
}

async fn update_guild_daily_activity(pool: &PgPool, message: &IndexedMessage) -> Result<()> {
    let activity_date = date_for(message.occurred_at);

    sqlx::query(
        r#"
        INSERT INTO guild_daily_activity (
            guild_id,
            activity_date,
            messages,
            last_message_at
        )
        VALUES ($1, $2, 1, $3)
        ON CONFLICT (guild_id, activity_date)
        DO UPDATE SET
            messages = guild_daily_activity.messages + 1,
            last_message_at = GREATEST(guild_daily_activity.last_message_at, EXCLUDED.last_message_at)
        "#,
    )
    .bind(&message.guild_id)
    .bind(activity_date)
    .bind(message.occurred_at)
    .execute(pool)
    .await
    .context("failed to update guild daily activity")?;

    sqlx::query(
        r#"
        INSERT INTO guild_summary_daily (
            guild_id,
            summary_date,
            messages,
            active_members,
            joined_members,
            left_members,
            onboarded_members,
            last_message_at
        )
        VALUES ($1, $2, 1, 0, 0, 0, 0, $3)
        ON CONFLICT (guild_id, summary_date)
        DO UPDATE SET
            messages = guild_summary_daily.messages + 1,
            last_message_at = GREATEST(
                guild_summary_daily.last_message_at,
                EXCLUDED.last_message_at
            )
        "#,
    )
    .bind(&message.guild_id)
    .bind(activity_date)
    .bind(message.occurred_at)
    .execute(pool)
    .await
    .context("failed to update guild_summary_daily messages")?;

    Ok(())
}

async fn process_voice_state_update(
    pool: &PgPool,
    guild_id: &str,
    member_id: &str,
    old_channel_id: Option<&str>,
    new_channel_id: Option<&str>,
    occurred_at: DateTime<Utc>,
) -> Result<()> {
    if old_channel_id == new_channel_id {
        return Ok(());
    }

    if old_channel_id.is_some() {
        close_active_voice_session(pool, guild_id, member_id, occurred_at).await?;
    }

    if let Some(new_channel_id) = new_channel_id {
        sqlx::query(
            r#"
            INSERT INTO active_voice_sessions (
                guild_id,
                member_id,
                channel_id,
                started_at
            )
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (guild_id, member_id)
            DO UPDATE SET
                channel_id = EXCLUDED.channel_id,
                started_at = EXCLUDED.started_at
            "#,
        )
        .bind(guild_id)
        .bind(member_id)
        .bind(new_channel_id)
        .bind(occurred_at)
        .execute(pool)
        .await
        .context("failed to open active voice session")?;

        sqlx::query(
            r#"
            INSERT INTO member_lifecycle (
                guild_id,
                member_id,
                first_seen_at,
                first_voice_at
            )
            VALUES ($1, $2, $3, $3)
            ON CONFLICT (guild_id, member_id)
            DO UPDATE SET
                first_voice_at = COALESCE(member_lifecycle.first_voice_at, EXCLUDED.first_voice_at)
            "#,
        )
        .bind(guild_id)
        .bind(member_id)
        .bind(occurred_at)
        .execute(pool)
        .await
        .context("failed to update first voice timestamp")?;
        maybe_record_activation_step(
            pool,
            guild_id,
            member_id,
            "activation_funnel_voice_members",
            "first_voice_at",
            "first_voice_members",
        )
        .await?;
    }

    Ok(())
}

async fn close_active_voice_session(
    pool: &PgPool,
    guild_id: &str,
    member_id: &str,
    occurred_at: DateTime<Utc>,
) -> Result<()> {
    let active = sqlx::query_as::<_, ActiveVoiceSessionRow>(
        r#"
        DELETE FROM active_voice_sessions
        WHERE guild_id = $1
          AND member_id = $2
        RETURNING channel_id, started_at
        "#,
    )
    .bind(guild_id)
    .bind(member_id)
    .fetch_optional(pool)
    .await
    .context("failed to close active voice session")?;

    let Some(active) = active else {
        return Ok(());
    };

    let ended_at = if occurred_at >= active.started_at {
        occurred_at
    } else {
        active.started_at
    };
    let duration_seconds = ended_at
        .signed_duration_since(active.started_at)
        .num_seconds()
        .max(0);

    sqlx::query(
        r#"
        INSERT INTO voice_sessions (
            guild_id,
            member_id,
            channel_id,
            started_at,
            ended_at,
            duration_seconds
        )
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (guild_id, member_id, started_at)
        DO NOTHING
        "#,
    )
    .bind(guild_id)
    .bind(member_id)
    .bind(&active.channel_id)
    .bind(active.started_at)
    .bind(ended_at)
    .bind(duration_seconds)
    .execute(pool)
    .await
    .context("failed to persist voice session")?;

    if duration_seconds > 0 {
        allocate_voice_seconds(
            pool,
            guild_id,
            member_id,
            &active.channel_id,
            active.started_at,
            ended_at,
        )
        .await?;
    }

    Ok(())
}

async fn allocate_voice_seconds(
    pool: &PgPool,
    guild_id: &str,
    member_id: &str,
    channel_id: &str,
    started_at: DateTime<Utc>,
    ended_at: DateTime<Utc>,
) -> Result<()> {
    let mut cursor = started_at;

    while cursor < ended_at {
        let next_day = cursor
            .date_naive()
            .succ_opt()
            .and_then(|date| date.and_hms_opt(0, 0, 0))
            .map(|naive| DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc))
            .unwrap_or(ended_at);
        let segment_end = ended_at.min(next_day);
        let duration_seconds = segment_end
            .signed_duration_since(cursor)
            .num_seconds()
            .max(0);

        if duration_seconds > 0 {
            upsert_member_voice_activity(
                pool,
                guild_id,
                member_id,
                channel_id,
                cursor,
                segment_end,
                duration_seconds,
            )
            .await?;
        }

        cursor = segment_end;
    }

    Ok(())
}

fn channel_health_score(
    message_count: i64,
    unique_senders: i64,
    replies: i64,
    previous_period_messages: i64,
    avg_response_seconds: Option<f64>,
) -> f64 {
    let participation_score = if message_count > 0 {
        ((unique_senders as f64 / message_count as f64) * 400.0).clamp(0.0, 100.0)
    } else {
        0.0
    };
    let reply_score = if message_count > 0 {
        ((replies as f64 / message_count as f64) * 200.0).clamp(0.0, 100.0)
    } else {
        0.0
    };
    let trend_score = if previous_period_messages > 0 {
        (50.0
            + (((message_count - previous_period_messages) as f64
                / previous_period_messages as f64)
                * 50.0))
            .clamp(0.0, 100.0)
    } else if message_count > 0 {
        70.0
    } else {
        0.0
    };
    let response_score = match avg_response_seconds {
        Some(seconds) if seconds <= 300.0 => 100.0,
        Some(seconds) if seconds <= 1_800.0 => 80.0,
        Some(seconds) if seconds <= 7_200.0 => 55.0,
        Some(_) => 30.0,
        None => 50.0,
    };

    ((participation_score + reply_score + trend_score + response_score) / 4.0 * 10.0).round() / 10.0
}

async fn upsert_member_voice_activity(
    pool: &PgPool,
    guild_id: &str,
    member_id: &str,
    channel_id: &str,
    started_at: DateTime<Utc>,
    last_active_at: DateTime<Utc>,
    duration_seconds: i64,
) -> Result<()> {
    let activity_date = date_for(started_at);

    sqlx::query(
        r#"
        INSERT INTO member_daily_activity (
            guild_id,
            member_id,
            activity_date,
            messages_sent,
            reactions_added,
            voice_seconds,
            active_channels,
            was_active,
            last_active_at,
            last_channel_id
        )
        VALUES ($1, $2, $3, 0, 0, $4, 1, TRUE, $5, $6)
        ON CONFLICT (guild_id, member_id, activity_date)
        DO UPDATE SET
            voice_seconds = member_daily_activity.voice_seconds + $4,
            active_channels = member_daily_activity.active_channels + CASE
                WHEN member_daily_activity.last_channel_id IS DISTINCT FROM EXCLUDED.last_channel_id THEN 1
                ELSE 0
            END,
            was_active = TRUE,
            last_channel_id = EXCLUDED.last_channel_id,
            last_active_at = GREATEST(member_daily_activity.last_active_at, EXCLUDED.last_active_at)
        "#,
    )
    .bind(guild_id)
    .bind(member_id)
    .bind(activity_date)
    .bind(duration_seconds)
    .bind(last_active_at)
    .bind(channel_id)
    .execute(pool)
    .await
    .context("failed to update member voice activity")?;

    mark_member_active_for_day(pool, guild_id, member_id, activity_date).await?;

    Ok(())
}

async fn mark_member_active_for_day(
    pool: &PgPool,
    guild_id: &str,
    member_id: &str,
    activity_date: NaiveDate,
) -> Result<()> {
    let inserted = sqlx::query(
        r#"
        INSERT INTO guild_daily_active_members (
            guild_id,
            activity_date,
            member_id
        )
        VALUES ($1, $2, $3)
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(guild_id)
    .bind(activity_date)
    .bind(member_id)
    .execute(pool)
    .await
    .context("failed to insert guild daily active member marker")?
    .rows_affected()
        > 0;

    if inserted {
        increment_guild_summary_member_counter(pool, guild_id, activity_date, "active_members")
            .await?;
        maybe_record_retention_for_activity_day(pool, guild_id, member_id, activity_date).await?;
    }

    Ok(())
}

async fn record_member_join(
    pool: &PgPool,
    guild_id: &str,
    member_id: &str,
    joined_at: DateTime<Utc>,
) -> Result<bool> {
    let inserted = sqlx::query(
        r#"
        INSERT INTO guild_joined_members (
            guild_id,
            member_id,
            joined_at
        )
        VALUES ($1, $2, $3)
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(guild_id)
    .bind(member_id)
    .bind(joined_at)
    .execute(pool)
    .await
    .context("failed to insert guild joined member marker")?
    .rows_affected()
        > 0;

    if inserted {
        increment_guild_summary_member_counter(
            pool,
            guild_id,
            date_for(joined_at),
            "joined_members",
        )
        .await?;
        increment_retention_cohort_counter(pool, guild_id, date_for(joined_at), "joined_members")
            .await?;
    }

    Ok(inserted)
}

async fn record_member_leave(
    pool: &PgPool,
    guild_id: &str,
    member_id: &str,
    left_at: DateTime<Utc>,
) -> Result<()> {
    let inserted = sqlx::query(
        r#"
        INSERT INTO guild_left_members (
            guild_id,
            member_id,
            left_at
        )
        VALUES ($1, $2, $3)
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(guild_id)
    .bind(member_id)
    .bind(left_at)
    .execute(pool)
    .await
    .context("failed to insert guild left member marker")?
    .rows_affected()
        > 0;

    if inserted {
        increment_guild_summary_member_counter(pool, guild_id, date_for(left_at), "left_members")
            .await?;
    }

    Ok(())
}

async fn maybe_record_onboarding_completion(
    pool: &PgPool,
    guild_id: &str,
    member_id: &str,
) -> Result<()> {
    let joined_at = sqlx::query_scalar::<_, DateTime<Utc>>(
        r#"
        WITH completion AS (
            SELECT
                joined_at,
                GREATEST(first_role_at, first_message_at) AS completed_at
            FROM member_lifecycle
            WHERE guild_id = $1
              AND member_id = $2
              AND joined_at IS NOT NULL
              AND first_role_at IS NOT NULL
              AND first_message_at IS NOT NULL
        ),
        inserted AS (
            INSERT INTO guild_onboarded_members (
                guild_id,
                member_id,
                completed_at
            )
            SELECT $1, $2, completed_at
            FROM completion
            ON CONFLICT DO NOTHING
            RETURNING completed_at
        )
        SELECT joined_at
        FROM completion
        WHERE EXISTS (SELECT 1 FROM inserted)
        "#,
    )
    .bind(guild_id)
    .bind(member_id)
    .fetch_optional(pool)
    .await
    .context("failed to insert guild onboarding completion marker")?;

    if let Some(joined_at) = joined_at {
        increment_guild_summary_member_counter(
            pool,
            guild_id,
            date_for(joined_at),
            "onboarded_members",
        )
        .await?;
    }

    Ok(())
}

async fn maybe_record_activation_step(
    pool: &PgPool,
    guild_id: &str,
    member_id: &str,
    marker_table: &str,
    completion_column: &str,
    counter_column: &str,
) -> Result<()> {
    let query = format!(
        r#"
        WITH eligible AS (
            SELECT DATE(joined_at) AS cohort_date
            FROM member_lifecycle
            WHERE guild_id = $1
              AND member_id = $2
              AND joined_at IS NOT NULL
              AND {completion_column} IS NOT NULL
              AND {completion_column} >= joined_at
        ),
        inserted AS (
            INSERT INTO {marker_table} (
                guild_id,
                member_id,
                cohort_date
            )
            SELECT $1, $2, cohort_date
            FROM eligible
            ON CONFLICT DO NOTHING
            RETURNING cohort_date
        )
        SELECT cohort_date
        FROM inserted
        "#
    );

    let cohort_date = sqlx::query_scalar::<_, NaiveDate>(&query)
        .bind(guild_id)
        .bind(member_id)
        .fetch_optional(pool)
        .await
        .with_context(|| format!("failed to record activation funnel step for {counter_column}"))?;

    if let Some(cohort_date) = cohort_date {
        increment_activation_funnel_counter(pool, guild_id, cohort_date, counter_column).await?;
    }

    Ok(())
}

async fn increment_activation_funnel_counter(
    pool: &PgPool,
    guild_id: &str,
    cohort_date: NaiveDate,
    counter_column: &str,
) -> Result<()> {
    let query = format!(
        r#"
        INSERT INTO activation_funnel_daily (
            guild_id,
            cohort_date,
            joined_members,
            got_role_members,
            first_message_members,
            first_reaction_members,
            first_voice_members,
            returned_next_week_members
        )
        VALUES ($1, $2, 0, 0, 0, 0, 0, 0)
        ON CONFLICT (guild_id, cohort_date)
        DO UPDATE SET
            {counter_column} = activation_funnel_daily.{counter_column} + 1
        "#
    );

    sqlx::query(&query)
        .bind(guild_id)
        .bind(cohort_date)
        .execute(pool)
        .await
        .with_context(|| format!("failed to increment activation_funnel_daily {counter_column}"))?;

    Ok(())
}

async fn increment_guild_summary_member_counter(
    pool: &PgPool,
    guild_id: &str,
    summary_date: NaiveDate,
    counter_column: &str,
) -> Result<()> {
    let query = format!(
        r#"
        INSERT INTO guild_summary_daily (
            guild_id,
            summary_date,
            messages,
            active_members,
            joined_members,
            left_members,
            onboarded_members,
            last_message_at
        )
        VALUES ($1, $2, 0, 0, 0, 0, 0, $3)
        ON CONFLICT (guild_id, summary_date)
        DO UPDATE SET
            {counter_column} = guild_summary_daily.{counter_column} + 1
        "#
    );

    sqlx::query(&query)
        .bind(guild_id)
        .bind(summary_date)
        .bind(start_of_day(summary_date))
        .execute(pool)
        .await
        .with_context(|| format!("failed to increment guild_summary_daily {counter_column}"))?;

    Ok(())
}

async fn maybe_record_retention_for_activity_day(
    pool: &PgPool,
    guild_id: &str,
    member_id: &str,
    activity_date: NaiveDate,
) -> Result<()> {
    let joined_at = sqlx::query_scalar::<_, DateTime<Utc>>(
        r#"
        SELECT joined_at
        FROM member_lifecycle
        WHERE guild_id = $1
          AND member_id = $2
          AND joined_at IS NOT NULL
        "#,
    )
    .bind(guild_id)
    .bind(member_id)
    .fetch_optional(pool)
    .await
    .context("failed to load member joined_at for retention")?;

    let Some(joined_at) = joined_at else {
        return Ok(());
    };

    let cohort_date = date_for(joined_at);
    let days_since_join = activity_date.signed_duration_since(cohort_date).num_days();

    if (7..14).contains(&days_since_join) {
        let inserted = sqlx::query(
            r#"
            INSERT INTO retention_cohort_d7_members (
                guild_id,
                member_id,
                cohort_date
            )
            VALUES ($1, $2, $3)
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(guild_id)
        .bind(member_id)
        .bind(cohort_date)
        .execute(pool)
        .await
        .context("failed to insert D7 retention marker")?
        .rows_affected()
            > 0;

        if inserted {
            increment_retention_cohort_counter(pool, guild_id, cohort_date, "d7_retained_members")
                .await?;
            increment_activation_funnel_counter(
                pool,
                guild_id,
                cohort_date,
                "returned_next_week_members",
            )
            .await?;
            record_channel_retention_contribution(
                pool,
                guild_id,
                member_id,
                joined_at,
                cohort_date,
                "channel_retention_d7_members",
                "d7_retained_members",
            )
            .await?;
        }
    }

    if (30..37).contains(&days_since_join) {
        let inserted = sqlx::query(
            r#"
            INSERT INTO retention_cohort_d30_members (
                guild_id,
                member_id,
                cohort_date
            )
            VALUES ($1, $2, $3)
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(guild_id)
        .bind(member_id)
        .bind(cohort_date)
        .execute(pool)
        .await
        .context("failed to insert D30 retention marker")?
        .rows_affected()
            > 0;

        if inserted {
            increment_retention_cohort_counter(pool, guild_id, cohort_date, "d30_retained_members")
                .await?;
            record_channel_retention_contribution(
                pool,
                guild_id,
                member_id,
                joined_at,
                cohort_date,
                "channel_retention_d30_members",
                "d30_retained_members",
            )
            .await?;
        }
    }

    Ok(())
}

async fn increment_retention_cohort_counter(
    pool: &PgPool,
    guild_id: &str,
    cohort_date: NaiveDate,
    counter_column: &str,
) -> Result<()> {
    let query = format!(
        r#"
        INSERT INTO retention_cohorts (
            guild_id,
            cohort_date,
            joined_members,
            d7_retained_members,
            d30_retained_members
        )
        VALUES ($1, $2, 0, 0, 0)
        ON CONFLICT (guild_id, cohort_date)
        DO UPDATE SET
            {counter_column} = retention_cohorts.{counter_column} + 1
        "#
    );

    sqlx::query(&query)
        .bind(guild_id)
        .bind(cohort_date)
        .execute(pool)
        .await
        .with_context(|| format!("failed to increment retention_cohorts {counter_column}"))?;

    Ok(())
}

async fn record_channel_retention_contribution(
    pool: &PgPool,
    guild_id: &str,
    member_id: &str,
    joined_at: DateTime<Utc>,
    cohort_date: NaiveDate,
    marker_table: &str,
    counter_column: &str,
) -> Result<()> {
    let channels = sqlx::query_as::<_, MemberChannelRow>(
        r#"
        SELECT DISTINCT channel_id
        FROM message_index
        WHERE guild_id = $1
          AND author_id = $2
          AND channel_id <> ''
          AND occurred_at >= $3
          AND occurred_at < $3 + INTERVAL '7 days'
        ORDER BY channel_id ASC
        "#,
    )
    .bind(guild_id)
    .bind(member_id)
    .bind(joined_at)
    .fetch_all(pool)
    .await
    .context("failed to load member channels for retention contribution")?;

    for channel in channels {
        let query = format!(
            r#"
            INSERT INTO {marker_table} (
                guild_id,
                channel_id,
                member_id,
                cohort_date
            )
            VALUES ($1, $2, $3, $4)
            ON CONFLICT DO NOTHING
            "#
        );

        let inserted = sqlx::query(&query)
            .bind(guild_id)
            .bind(&channel.channel_id)
            .bind(member_id)
            .bind(cohort_date)
            .execute(pool)
            .await
            .with_context(|| {
                format!("failed to insert channel retention marker for {counter_column}")
            })?
            .rows_affected()
            > 0;

        if inserted {
            increment_channel_retention_counter(
                pool,
                guild_id,
                &channel.channel_id,
                cohort_date,
                counter_column,
            )
            .await?;
        }
    }

    Ok(())
}

async fn increment_channel_retention_counter(
    pool: &PgPool,
    guild_id: &str,
    channel_id: &str,
    cohort_date: NaiveDate,
    counter_column: &str,
) -> Result<()> {
    let query = format!(
        r#"
        INSERT INTO channel_retention_daily (
            guild_id,
            channel_id,
            cohort_date,
            d7_retained_members,
            d30_retained_members
        )
        VALUES ($1, $2, $3, 0, 0)
        ON CONFLICT (guild_id, channel_id, cohort_date)
        DO UPDATE SET
            {counter_column} = channel_retention_daily.{counter_column} + 1
        "#
    );

    sqlx::query(&query)
        .bind(guild_id)
        .bind(channel_id)
        .bind(cohort_date)
        .execute(pool)
        .await
        .with_context(|| format!("failed to increment channel_retention_daily {counter_column}"))?;

    Ok(())
}

async fn increment_public_message_count(pool: &PgPool) -> Result<()> {
    increment_public_message_count_by(pool, 1).await
}

async fn increment_public_message_count_by(pool: &PgPool, count: i64) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE public_stats_cache
        SET messages_tracked = messages_tracked + $1,
            refreshed_at = NOW()
        WHERE cache_key = 'global'
        "#,
    )
    .bind(count)
    .execute(pool)
    .await
    .context("failed to increment public stats cache")?;

    Ok(())
}

async fn refresh_public_stats_cache(pool: &PgPool) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE public_stats_cache AS cache
        SET servers = stats.servers,
            members = stats.members,
            refreshed_at = NOW()
        FROM (
            SELECT
                COALESCE(COUNT(*) FILTER (WHERE is_active = TRUE), 0)::BIGINT AS servers,
                COALESCE(SUM(member_count) FILTER (WHERE is_active = TRUE), 0)::BIGINT AS members
            FROM guild_inventory
        ) AS stats
        WHERE cache.cache_key = 'global'
        "#,
    )
    .execute(pool)
    .await
    .context("failed to refresh public stats cache")?;

    Ok(())
}

async fn rebuild_message_analytics_for_scope(
    pool: &PgPool,
    queue: &RedisEventQueue,
    scope: &BackfillRebuildScope,
) -> Result<()> {
    rebuild_member_lifecycle_from_messages_in_scope(pool, scope).await?;
    rebuild_member_daily_activity_from_messages_in_scope(pool, scope).await?;
    rebuild_guild_daily_active_members_in_scope(pool, scope).await?;
    rebuild_channel_daily_unique_senders_in_scope(pool, scope).await?;
    rebuild_channel_daily_activity_from_messages_in_scope(pool, scope).await?;
    rebuild_channel_health_daily_from_activity_in_scope(pool, scope).await?;
    rebuild_guild_daily_activity_from_messages_in_scope(pool, scope).await?;
    rebuild_guild_onboarded_members_in_scope(pool, scope).await?;
    rebuild_activation_funnel_member_markers_in_scope(pool, scope).await?;
    rebuild_retention_cohort_members_in_scope(pool, scope).await?;
    rebuild_channel_retention_members_in_scope(pool, scope).await?;
    rebuild_retention_cohorts_in_scope(pool, scope).await?;
    rebuild_channel_retention_daily_in_scope(pool, scope).await?;
    rebuild_activation_funnel_daily_in_scope(pool, scope).await?;
    rebuild_guild_summary_daily_in_scope(pool, scope).await?;
    recount_public_message_count(pool).await?;
    refresh_public_stats_cache(pool).await?;
    invalidate_dashboard_message_caches(queue, &scope.guild_id).await;

    Ok(())
}

async fn rebuild_member_lifecycle_from_messages_in_scope(
    pool: &PgPool,
    scope: &BackfillRebuildScope,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO member_lifecycle (
            guild_id,
            member_id,
            first_seen_at,
            first_message_at
        )
        SELECT
            guild_id,
            author_id AS member_id,
            MIN(occurred_at) AS first_seen_at,
            MIN(occurred_at) AS first_message_at
        FROM message_index
        WHERE guild_id = $1
          AND is_bot = FALSE
          AND occurred_at >= $2
          AND occurred_at <= $3
        GROUP BY guild_id, author_id
        ON CONFLICT (guild_id, member_id)
        DO UPDATE SET
            first_seen_at = LEAST(member_lifecycle.first_seen_at, EXCLUDED.first_seen_at),
            first_message_at = CASE
                WHEN member_lifecycle.first_message_at IS NULL THEN EXCLUDED.first_message_at
                ELSE LEAST(member_lifecycle.first_message_at, EXCLUDED.first_message_at)
            END
        "#,
    )
    .bind(&scope.guild_id)
    .bind(scope.activity_start_at)
    .bind(scope.activity_end_at)
    .execute(pool)
    .await
    .context("failed to rebuild member_lifecycle from messages")?;

    Ok(())
}

async fn rebuild_member_daily_activity_from_messages_in_scope(
    pool: &PgPool,
    scope: &BackfillRebuildScope,
) -> Result<()> {
    sqlx::query(
        r#"
        DELETE FROM member_daily_activity
        WHERE guild_id = $1
          AND activity_date >= $2
          AND activity_date <= $3
        "#,
    )
    .bind(&scope.guild_id)
    .bind(scope.activity_start_date)
    .bind(scope.activity_end_date)
    .execute(pool)
    .await
    .context("failed to clear member_daily_activity for rebuild")?;

    sqlx::query(
        r#"
        INSERT INTO member_daily_activity (
            guild_id,
            member_id,
            activity_date,
            messages_sent,
            reactions_added,
            voice_seconds,
            active_channels,
            was_active,
            last_active_at,
            last_channel_id
        )
        SELECT
            guild_id,
            author_id AS member_id,
            DATE(occurred_at) AS activity_date,
            COUNT(*)::INTEGER AS messages_sent,
            0 AS reactions_added,
            0::BIGINT AS voice_seconds,
            COUNT(DISTINCT channel_id)::INTEGER AS active_channels,
            TRUE AS was_active,
            MAX(occurred_at) AS last_active_at,
            (ARRAY_AGG(channel_id ORDER BY occurred_at DESC, message_id DESC))[1] AS last_channel_id
        FROM message_index
        WHERE guild_id = $1
          AND is_bot = FALSE
          AND DATE(occurred_at) >= $2
          AND DATE(occurred_at) <= $3
        GROUP BY guild_id, author_id, DATE(occurred_at)
        ON CONFLICT (guild_id, member_id, activity_date)
        DO UPDATE SET
            messages_sent = EXCLUDED.messages_sent,
            active_channels = GREATEST(
                member_daily_activity.active_channels,
                EXCLUDED.active_channels
            ),
            was_active = TRUE,
            last_channel_id = CASE
                WHEN member_daily_activity.last_active_at > EXCLUDED.last_active_at
                    THEN member_daily_activity.last_channel_id
                ELSE EXCLUDED.last_channel_id
            END,
            last_active_at = GREATEST(
                member_daily_activity.last_active_at,
                EXCLUDED.last_active_at
            )
        "#,
    )
    .bind(&scope.guild_id)
    .bind(scope.activity_start_date)
    .bind(scope.activity_end_date)
    .execute(pool)
    .await
    .context("failed to rebuild member_daily_activity from messages")?;

    Ok(())
}

async fn rebuild_guild_daily_active_members_in_scope(
    pool: &PgPool,
    scope: &BackfillRebuildScope,
) -> Result<()> {
    sqlx::query(
        r#"
        DELETE FROM guild_daily_active_members
        WHERE guild_id = $1
          AND activity_date >= $2
          AND activity_date <= $3
        "#,
    )
    .bind(&scope.guild_id)
    .bind(scope.activity_start_date)
    .bind(scope.activity_end_date)
    .execute(pool)
    .await
    .context("failed to clear guild_daily_active_members for rebuild")?;

    sqlx::query(
        r#"
        INSERT INTO guild_daily_active_members (
            guild_id,
            activity_date,
            member_id
        )
        SELECT guild_id, activity_date, member_id
        FROM member_daily_activity
        WHERE guild_id = $1
          AND activity_date >= $2
          AND activity_date <= $3
          AND (
                was_active = TRUE
                OR messages_sent > 0
                OR reactions_added > 0
                OR voice_seconds > 0
          )
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(&scope.guild_id)
    .bind(scope.activity_start_date)
    .bind(scope.activity_end_date)
    .execute(pool)
    .await
    .context("failed to rebuild guild_daily_active_members")?;

    Ok(())
}

async fn rebuild_channel_daily_unique_senders_in_scope(
    pool: &PgPool,
    scope: &BackfillRebuildScope,
) -> Result<()> {
    sqlx::query(
        r#"
        DELETE FROM channel_daily_unique_senders
        WHERE guild_id = $1
          AND activity_date >= $2
          AND activity_date <= $3
        "#,
    )
    .bind(&scope.guild_id)
    .bind(scope.activity_start_date)
    .bind(scope.activity_end_date)
    .execute(pool)
    .await
    .context("failed to clear channel_daily_unique_senders for rebuild")?;

    sqlx::query(
        r#"
        INSERT INTO channel_daily_unique_senders (
            guild_id,
            channel_id,
            activity_date,
            member_id
        )
        SELECT DISTINCT
            guild_id,
            channel_id,
            DATE(occurred_at) AS activity_date,
            author_id AS member_id
        FROM message_index
        WHERE guild_id = $1
          AND is_bot = FALSE
          AND channel_id <> ''
          AND DATE(occurred_at) >= $2
          AND DATE(occurred_at) <= $3
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(&scope.guild_id)
    .bind(scope.activity_start_date)
    .bind(scope.activity_end_date)
    .execute(pool)
    .await
    .context("failed to rebuild channel_daily_unique_senders")?;

    Ok(())
}

async fn rebuild_channel_daily_activity_from_messages_in_scope(
    pool: &PgPool,
    scope: &BackfillRebuildScope,
) -> Result<()> {
    sqlx::query(
        r#"
        DELETE FROM channel_daily_activity
        WHERE guild_id = $1
          AND activity_date >= $2
          AND activity_date <= $3
        "#,
    )
    .bind(&scope.guild_id)
    .bind(scope.activity_start_date)
    .bind(scope.activity_end_date)
    .execute(pool)
    .await
    .context("failed to clear channel_daily_activity for rebuild")?;

    sqlx::query(
        r#"
        WITH affected_channels AS (
            SELECT DISTINCT channel_id
            FROM message_index
            WHERE guild_id = $1
              AND channel_id <> ''
              AND occurred_at >= $2
              AND occurred_at <= $3
        ),
        ordered AS (
            SELECT
                message_index.guild_id,
                message_index.channel_id,
                DATE(message_index.occurred_at) AS activity_date,
                message_index.occurred_at,
                message_index.message_id,
                message_index.author_id,
                message_index.is_reply,
                LAG(message_index.author_id) OVER (
                    PARTITION BY message_index.guild_id, message_index.channel_id
                    ORDER BY message_index.occurred_at ASC, message_index.message_id ASC
                ) AS previous_author_id,
                LAG(message_index.occurred_at) OVER (
                    PARTITION BY message_index.guild_id, message_index.channel_id
                    ORDER BY message_index.occurred_at ASC, message_index.message_id ASC
                ) AS previous_occurred_at
            FROM message_index
            INNER JOIN affected_channels
                ON affected_channels.channel_id = message_index.channel_id
            WHERE message_index.guild_id = $1
              AND message_index.is_bot = FALSE
              AND message_index.channel_id <> ''
              AND message_index.occurred_at <= $3
        )
        INSERT INTO channel_daily_activity (
            guild_id,
            channel_id,
            activity_date,
            messages,
            unique_senders,
            replies,
            response_seconds_total,
            response_samples,
            last_message_at
        )
        SELECT
            guild_id,
            channel_id,
            activity_date,
            COUNT(*)::INTEGER AS messages,
            COUNT(DISTINCT author_id)::INTEGER AS unique_senders,
            SUM(CASE WHEN is_reply THEN 1 ELSE 0 END)::INTEGER AS replies,
            SUM(
                CASE
                    WHEN previous_author_id IS NOT NULL
                     AND previous_author_id IS DISTINCT FROM author_id
                     AND EXTRACT(EPOCH FROM (occurred_at - previous_occurred_at)) BETWEEN 0 AND 86400
                        THEN EXTRACT(EPOCH FROM (occurred_at - previous_occurred_at))::BIGINT
                    ELSE 0::BIGINT
                END
            )::BIGINT AS response_seconds_total,
            SUM(
                CASE
                    WHEN previous_author_id IS NOT NULL
                     AND previous_author_id IS DISTINCT FROM author_id
                     AND EXTRACT(EPOCH FROM (occurred_at - previous_occurred_at)) BETWEEN 0 AND 86400
                        THEN 1
                    ELSE 0
                END
            )::INTEGER AS response_samples,
            MAX(occurred_at) AS last_message_at
        FROM ordered
        WHERE activity_date >= $4
          AND activity_date <= $5
        GROUP BY guild_id, channel_id, activity_date
        "#,
    )
    .bind(&scope.guild_id)
    .bind(scope.activity_start_at)
    .bind(scope.activity_end_at)
    .bind(scope.activity_start_date)
    .bind(scope.activity_end_date)
    .execute(pool)
    .await
    .context("failed to rebuild channel_daily_activity from messages")?;

    Ok(())
}

async fn rebuild_channel_health_daily_from_activity_in_scope(
    pool: &PgPool,
    scope: &BackfillRebuildScope,
) -> Result<()> {
    sqlx::query(
        r#"
        DELETE FROM channel_health_daily
        WHERE guild_id = $1
          AND activity_date >= $2
          AND activity_date <= $3
        "#,
    )
    .bind(&scope.guild_id)
    .bind(scope.activity_start_date)
    .bind(scope.activity_end_date)
    .execute(pool)
    .await
    .context("failed to clear channel_health_daily for rebuild")?;

    sqlx::query(
        r#"
        INSERT INTO channel_health_daily (
            guild_id,
            channel_id,
            activity_date,
            messages,
            unique_senders,
            replies,
            response_seconds_total,
            response_samples,
            previous_day_messages,
            health_score
        )
        SELECT
            current.guild_id,
            current.channel_id,
            current.activity_date,
            current.messages::BIGINT AS messages,
            current.unique_senders::BIGINT AS unique_senders,
            current.replies::BIGINT AS replies,
            current.response_seconds_total::BIGINT AS response_seconds_total,
            current.response_samples::BIGINT AS response_samples,
            COALESCE(previous.messages, 0)::BIGINT AS previous_day_messages,
            ROUND((
                (
                    CASE
                        WHEN current.messages > 0
                            THEN LEAST(
                                GREATEST(
                                    (current.unique_senders::DOUBLE PRECISION
                                        / current.messages::DOUBLE PRECISION) * 400.0,
                                    0.0
                                ),
                                100.0
                            )
                        ELSE 0.0
                    END
                    +
                    CASE
                        WHEN current.messages > 0
                            THEN LEAST(
                                GREATEST(
                                    (current.replies::DOUBLE PRECISION
                                        / current.messages::DOUBLE PRECISION) * 200.0,
                                    0.0
                                ),
                                100.0
                            )
                        ELSE 0.0
                    END
                    +
                    CASE
                        WHEN COALESCE(previous.messages, 0) > 0
                            THEN LEAST(
                                GREATEST(
                                    50.0
                                    + (
                                        ((current.messages - previous.messages)::DOUBLE PRECISION
                                            / previous.messages::DOUBLE PRECISION) * 50.0
                                    ),
                                    0.0
                                ),
                                100.0
                            )
                        WHEN current.messages > 0 THEN 70.0
                        ELSE 0.0
                    END
                    +
                    CASE
                        WHEN current.response_samples > 0
                             AND (current.response_seconds_total::DOUBLE PRECISION
                                 / current.response_samples::DOUBLE PRECISION) <= 300.0
                            THEN 100.0
                        WHEN current.response_samples > 0
                             AND (current.response_seconds_total::DOUBLE PRECISION
                                 / current.response_samples::DOUBLE PRECISION) <= 1800.0
                            THEN 80.0
                        WHEN current.response_samples > 0
                             AND (current.response_seconds_total::DOUBLE PRECISION
                                 / current.response_samples::DOUBLE PRECISION) <= 7200.0
                            THEN 55.0
                        WHEN current.response_samples > 0 THEN 30.0
                        ELSE 50.0
                    END
                ) / 4.0
            ) * 10.0) / 10.0 AS health_score
        FROM channel_daily_activity AS current
        LEFT JOIN channel_daily_activity AS previous
            ON previous.guild_id = current.guild_id
           AND previous.channel_id = current.channel_id
           AND previous.activity_date = current.activity_date - 1
        WHERE current.guild_id = $1
          AND current.activity_date >= $2
          AND current.activity_date <= $3
        "#,
    )
    .bind(&scope.guild_id)
    .bind(scope.activity_start_date)
    .bind(scope.activity_end_date)
    .execute(pool)
    .await
    .context("failed to rebuild channel_health_daily from activity")?;

    Ok(())
}

async fn rebuild_guild_daily_activity_from_messages_in_scope(
    pool: &PgPool,
    scope: &BackfillRebuildScope,
) -> Result<()> {
    sqlx::query(
        r#"
        DELETE FROM guild_daily_activity
        WHERE guild_id = $1
          AND activity_date >= $2
          AND activity_date <= $3
        "#,
    )
    .bind(&scope.guild_id)
    .bind(scope.activity_start_date)
    .bind(scope.activity_end_date)
    .execute(pool)
    .await
    .context("failed to clear guild_daily_activity for rebuild")?;

    sqlx::query(
        r#"
        INSERT INTO guild_daily_activity (
            guild_id,
            activity_date,
            messages,
            last_message_at
        )
        SELECT
            guild_id,
            DATE(occurred_at) AS activity_date,
            COUNT(*)::BIGINT AS messages,
            MAX(occurred_at) AS last_message_at
        FROM message_index
        WHERE guild_id = $1
          AND DATE(occurred_at) >= $2
          AND DATE(occurred_at) <= $3
        GROUP BY guild_id, DATE(occurred_at)
        "#,
    )
    .bind(&scope.guild_id)
    .bind(scope.activity_start_date)
    .bind(scope.activity_end_date)
    .execute(pool)
    .await
    .context("failed to rebuild guild_daily_activity from messages")?;

    Ok(())
}

async fn rebuild_guild_onboarded_members_in_scope(
    pool: &PgPool,
    scope: &BackfillRebuildScope,
) -> Result<()> {
    sqlx::query(
        r#"
        DELETE FROM guild_onboarded_members
        WHERE guild_id = $1
          AND member_id IN (
              SELECT DISTINCT author_id
              FROM message_index
              WHERE guild_id = $1
                AND is_bot = FALSE
                AND occurred_at >= $2
                AND occurred_at <= $3
          )
        "#,
    )
    .bind(&scope.guild_id)
    .bind(scope.activity_start_at)
    .bind(scope.activity_end_at)
    .execute(pool)
    .await
    .context("failed to clear guild_onboarded_members for rebuild")?;

    sqlx::query(
        r#"
        INSERT INTO guild_onboarded_members (
            guild_id,
            member_id,
            completed_at
        )
        SELECT
            guild_id,
            member_id,
            GREATEST(first_role_at, first_message_at) AS completed_at
        FROM member_lifecycle
        WHERE guild_id = $1
          AND member_id IN (
              SELECT DISTINCT author_id
              FROM message_index
              WHERE guild_id = $1
                AND is_bot = FALSE
                AND occurred_at >= $2
                AND occurred_at <= $3
          )
          AND first_role_at IS NOT NULL
          AND first_message_at IS NOT NULL
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(&scope.guild_id)
    .bind(scope.activity_start_at)
    .bind(scope.activity_end_at)
    .execute(pool)
    .await
    .context("failed to rebuild guild_onboarded_members")?;

    Ok(())
}

async fn rebuild_activation_funnel_member_markers_in_scope(
    pool: &PgPool,
    scope: &BackfillRebuildScope,
) -> Result<()> {
    for (table, column) in [
        ("activation_funnel_role_members", "first_role_at"),
        ("activation_funnel_message_members", "first_message_at"),
        ("activation_funnel_reaction_members", "first_reaction_at"),
        ("activation_funnel_voice_members", "first_voice_at"),
    ] {
        let delete_query = format!(
            "DELETE FROM {table} WHERE guild_id = $1 AND cohort_date >= $2 AND cohort_date <= $3"
        );
        sqlx::query(&delete_query)
            .bind(&scope.guild_id)
            .bind(scope.cohort_start_date)
            .bind(scope.cohort_end_date)
            .execute(pool)
            .await
            .with_context(|| format!("failed to clear {table} for rebuild"))?;

        let insert_query = format!(
            r#"
            INSERT INTO {table} (
                guild_id,
                member_id,
                cohort_date
            )
            SELECT
                guild_id,
                member_id,
                DATE(joined_at) AS cohort_date
            FROM member_lifecycle
            WHERE guild_id = $1
              AND DATE(joined_at) >= $2
              AND DATE(joined_at) <= $3
              AND joined_at IS NOT NULL
              AND {column} IS NOT NULL
              AND {column} >= joined_at
            ON CONFLICT DO NOTHING
            "#
        );
        sqlx::query(&insert_query)
            .bind(&scope.guild_id)
            .bind(scope.cohort_start_date)
            .bind(scope.cohort_end_date)
            .execute(pool)
            .await
            .with_context(|| format!("failed to rebuild {table}"))?;
    }

    Ok(())
}

async fn rebuild_retention_cohort_members_in_scope(
    pool: &PgPool,
    scope: &BackfillRebuildScope,
) -> Result<()> {
    for (table, start_day, end_day) in [
        ("retention_cohort_d7_members", 7_i64, 14_i64),
        ("retention_cohort_d30_members", 30_i64, 37_i64),
    ] {
        let delete_query = format!(
            "DELETE FROM {table} WHERE guild_id = $1 AND cohort_date >= $2 AND cohort_date <= $3"
        );
        sqlx::query(&delete_query)
            .bind(&scope.guild_id)
            .bind(scope.cohort_start_date)
            .bind(scope.cohort_end_date)
            .execute(pool)
            .await
            .with_context(|| format!("failed to clear {table} for rebuild"))?;

        let insert_query = format!(
            r#"
            INSERT INTO {table} (
                guild_id,
                member_id,
                cohort_date
            )
            SELECT DISTINCT
                lifecycle.guild_id,
                lifecycle.member_id,
                DATE(lifecycle.joined_at) AS cohort_date
            FROM member_lifecycle AS lifecycle
            INNER JOIN member_daily_activity AS activity
                ON activity.guild_id = lifecycle.guild_id
               AND activity.member_id = lifecycle.member_id
            WHERE lifecycle.guild_id = $1
              AND DATE(lifecycle.joined_at) >= $2
              AND DATE(lifecycle.joined_at) <= $3
              AND lifecycle.joined_at IS NOT NULL
              AND activity.activity_date >= DATE(lifecycle.joined_at) + {start_day}
              AND activity.activity_date < DATE(lifecycle.joined_at) + {end_day}
            ON CONFLICT DO NOTHING
            "#
        );
        sqlx::query(&insert_query)
            .bind(&scope.guild_id)
            .bind(scope.cohort_start_date)
            .bind(scope.cohort_end_date)
            .execute(pool)
            .await
            .with_context(|| format!("failed to rebuild {table}"))?;
    }

    Ok(())
}

async fn rebuild_channel_retention_members_in_scope(
    pool: &PgPool,
    scope: &BackfillRebuildScope,
) -> Result<()> {
    for (table, retained_table) in [
        (
            "channel_retention_d7_members",
            "retention_cohort_d7_members",
        ),
        (
            "channel_retention_d30_members",
            "retention_cohort_d30_members",
        ),
    ] {
        let delete_query = format!(
            "DELETE FROM {table} WHERE guild_id = $1 AND cohort_date >= $2 AND cohort_date <= $3"
        );
        sqlx::query(&delete_query)
            .bind(&scope.guild_id)
            .bind(scope.cohort_start_date)
            .bind(scope.cohort_end_date)
            .execute(pool)
            .await
            .with_context(|| format!("failed to clear {table} for rebuild"))?;

        let insert_query = format!(
            r#"
            INSERT INTO {table} (
                guild_id,
                channel_id,
                member_id,
                cohort_date
            )
            SELECT DISTINCT
                retained.guild_id,
                message.channel_id,
                retained.member_id,
                retained.cohort_date
            FROM {retained_table} AS retained
            INNER JOIN member_lifecycle AS lifecycle
                ON lifecycle.guild_id = retained.guild_id
               AND lifecycle.member_id = retained.member_id
            INNER JOIN message_index AS message
                ON message.guild_id = retained.guild_id
               AND message.author_id = retained.member_id
            WHERE retained.guild_id = $1
              AND retained.cohort_date >= $2
              AND retained.cohort_date <= $3
              AND lifecycle.joined_at IS NOT NULL
              AND message.channel_id <> ''
              AND message.occurred_at >= lifecycle.joined_at
              AND message.occurred_at < lifecycle.joined_at + INTERVAL '7 days'
            ON CONFLICT DO NOTHING
            "#
        );
        sqlx::query(&insert_query)
            .bind(&scope.guild_id)
            .bind(scope.cohort_start_date)
            .bind(scope.cohort_end_date)
            .execute(pool)
            .await
            .with_context(|| format!("failed to rebuild {table}"))?;
    }

    Ok(())
}

async fn rebuild_retention_cohorts_in_scope(
    pool: &PgPool,
    scope: &BackfillRebuildScope,
) -> Result<()> {
    sqlx::query(
        r#"
        DELETE FROM retention_cohorts
        WHERE guild_id = $1
          AND cohort_date >= $2
          AND cohort_date <= $3
        "#,
    )
    .bind(&scope.guild_id)
    .bind(scope.cohort_start_date)
    .bind(scope.cohort_end_date)
    .execute(pool)
    .await
    .context("failed to clear retention_cohorts for rebuild")?;

    sqlx::query(
        r#"
        INSERT INTO retention_cohorts (
            guild_id,
            cohort_date,
            joined_members,
            d7_retained_members,
            d30_retained_members
        )
        SELECT
            guild_id,
            cohort_date,
            SUM(joined_members)::BIGINT AS joined_members,
            SUM(d7_retained_members)::BIGINT AS d7_retained_members,
            SUM(d30_retained_members)::BIGINT AS d30_retained_members
        FROM (
            SELECT
                guild_id,
                DATE(joined_at) AS cohort_date,
                COUNT(*)::BIGINT AS joined_members,
                0::BIGINT AS d7_retained_members,
                0::BIGINT AS d30_retained_members
            FROM member_lifecycle
            WHERE guild_id = $1
              AND joined_at IS NOT NULL
              AND DATE(joined_at) >= $2
              AND DATE(joined_at) <= $3
            GROUP BY guild_id, DATE(joined_at)
            UNION ALL
            SELECT
                guild_id,
                cohort_date,
                0::BIGINT AS joined_members,
                COUNT(*)::BIGINT AS d7_retained_members,
                0::BIGINT AS d30_retained_members
            FROM retention_cohort_d7_members
            WHERE guild_id = $1
              AND cohort_date >= $2
              AND cohort_date <= $3
            GROUP BY guild_id, cohort_date
            UNION ALL
            SELECT
                guild_id,
                cohort_date,
                0::BIGINT AS joined_members,
                0::BIGINT AS d7_retained_members,
                COUNT(*)::BIGINT AS d30_retained_members
            FROM retention_cohort_d30_members
            WHERE guild_id = $1
              AND cohort_date >= $2
              AND cohort_date <= $3
            GROUP BY guild_id, cohort_date
        ) AS combined
        GROUP BY guild_id, cohort_date
        "#,
    )
    .bind(&scope.guild_id)
    .bind(scope.cohort_start_date)
    .bind(scope.cohort_end_date)
    .execute(pool)
    .await
    .context("failed to rebuild retention_cohorts")?;

    Ok(())
}

async fn rebuild_channel_retention_daily_in_scope(
    pool: &PgPool,
    scope: &BackfillRebuildScope,
) -> Result<()> {
    sqlx::query(
        r#"
        DELETE FROM channel_retention_daily
        WHERE guild_id = $1
          AND cohort_date >= $2
          AND cohort_date <= $3
        "#,
    )
    .bind(&scope.guild_id)
    .bind(scope.cohort_start_date)
    .bind(scope.cohort_end_date)
    .execute(pool)
    .await
    .context("failed to clear channel_retention_daily for rebuild")?;

    sqlx::query(
        r#"
        INSERT INTO channel_retention_daily (
            guild_id,
            channel_id,
            cohort_date,
            d7_retained_members,
            d30_retained_members
        )
        SELECT
            guild_id,
            channel_id,
            cohort_date,
            SUM(d7_retained_members)::BIGINT AS d7_retained_members,
            SUM(d30_retained_members)::BIGINT AS d30_retained_members
        FROM (
            SELECT
                guild_id,
                channel_id,
                cohort_date,
                COUNT(*)::BIGINT AS d7_retained_members,
                0::BIGINT AS d30_retained_members
            FROM channel_retention_d7_members
            WHERE guild_id = $1
              AND cohort_date >= $2
              AND cohort_date <= $3
            GROUP BY guild_id, channel_id, cohort_date
            UNION ALL
            SELECT
                guild_id,
                channel_id,
                cohort_date,
                0::BIGINT AS d7_retained_members,
                COUNT(*)::BIGINT AS d30_retained_members
            FROM channel_retention_d30_members
            WHERE guild_id = $1
              AND cohort_date >= $2
              AND cohort_date <= $3
            GROUP BY guild_id, channel_id, cohort_date
        ) AS combined
        GROUP BY guild_id, channel_id, cohort_date
        "#,
    )
    .bind(&scope.guild_id)
    .bind(scope.cohort_start_date)
    .bind(scope.cohort_end_date)
    .execute(pool)
    .await
    .context("failed to rebuild channel_retention_daily")?;

    Ok(())
}

async fn rebuild_activation_funnel_daily_in_scope(
    pool: &PgPool,
    scope: &BackfillRebuildScope,
) -> Result<()> {
    sqlx::query(
        r#"
        DELETE FROM activation_funnel_daily
        WHERE guild_id = $1
          AND cohort_date >= $2
          AND cohort_date <= $3
        "#,
    )
    .bind(&scope.guild_id)
    .bind(scope.cohort_start_date)
    .bind(scope.cohort_end_date)
    .execute(pool)
    .await
    .context("failed to clear activation_funnel_daily for rebuild")?;

    sqlx::query(
        r#"
        INSERT INTO activation_funnel_daily (
            guild_id,
            cohort_date,
            joined_members,
            got_role_members,
            first_message_members,
            first_reaction_members,
            first_voice_members,
            returned_next_week_members
        )
        SELECT
            guild_id,
            cohort_date,
            SUM(joined_members)::BIGINT AS joined_members,
            SUM(got_role_members)::BIGINT AS got_role_members,
            SUM(first_message_members)::BIGINT AS first_message_members,
            SUM(first_reaction_members)::BIGINT AS first_reaction_members,
            SUM(first_voice_members)::BIGINT AS first_voice_members,
            SUM(returned_next_week_members)::BIGINT AS returned_next_week_members
        FROM (
            SELECT
                guild_id,
                DATE(joined_at) AS cohort_date,
                COUNT(*)::BIGINT AS joined_members,
                0::BIGINT AS got_role_members,
                0::BIGINT AS first_message_members,
                0::BIGINT AS first_reaction_members,
                0::BIGINT AS first_voice_members,
                0::BIGINT AS returned_next_week_members
            FROM member_lifecycle
            WHERE guild_id = $1
              AND joined_at IS NOT NULL
              AND DATE(joined_at) >= $2
              AND DATE(joined_at) <= $3
            GROUP BY guild_id, DATE(joined_at)
            UNION ALL
            SELECT
                guild_id,
                cohort_date,
                0::BIGINT AS joined_members,
                COUNT(*)::BIGINT AS got_role_members,
                0::BIGINT AS first_message_members,
                0::BIGINT AS first_reaction_members,
                0::BIGINT AS first_voice_members,
                0::BIGINT AS returned_next_week_members
            FROM activation_funnel_role_members
            WHERE guild_id = $1
              AND cohort_date >= $2
              AND cohort_date <= $3
            GROUP BY guild_id, cohort_date
            UNION ALL
            SELECT
                guild_id,
                cohort_date,
                0::BIGINT AS joined_members,
                0::BIGINT AS got_role_members,
                COUNT(*)::BIGINT AS first_message_members,
                0::BIGINT AS first_reaction_members,
                0::BIGINT AS first_voice_members,
                0::BIGINT AS returned_next_week_members
            FROM activation_funnel_message_members
            WHERE guild_id = $1
              AND cohort_date >= $2
              AND cohort_date <= $3
            GROUP BY guild_id, cohort_date
            UNION ALL
            SELECT
                guild_id,
                cohort_date,
                0::BIGINT AS joined_members,
                0::BIGINT AS got_role_members,
                0::BIGINT AS first_message_members,
                COUNT(*)::BIGINT AS first_reaction_members,
                0::BIGINT AS first_voice_members,
                0::BIGINT AS returned_next_week_members
            FROM activation_funnel_reaction_members
            WHERE guild_id = $1
              AND cohort_date >= $2
              AND cohort_date <= $3
            GROUP BY guild_id, cohort_date
            UNION ALL
            SELECT
                guild_id,
                cohort_date,
                0::BIGINT AS joined_members,
                0::BIGINT AS got_role_members,
                0::BIGINT AS first_message_members,
                0::BIGINT AS first_reaction_members,
                COUNT(*)::BIGINT AS first_voice_members,
                0::BIGINT AS returned_next_week_members
            FROM activation_funnel_voice_members
            WHERE guild_id = $1
              AND cohort_date >= $2
              AND cohort_date <= $3
            GROUP BY guild_id, cohort_date
            UNION ALL
            SELECT
                guild_id,
                cohort_date,
                0::BIGINT AS joined_members,
                0::BIGINT AS got_role_members,
                0::BIGINT AS first_message_members,
                0::BIGINT AS first_reaction_members,
                0::BIGINT AS first_voice_members,
                COUNT(*)::BIGINT AS returned_next_week_members
            FROM retention_cohort_d7_members
            WHERE guild_id = $1
              AND cohort_date >= $2
              AND cohort_date <= $3
            GROUP BY guild_id, cohort_date
        ) AS combined
        GROUP BY guild_id, cohort_date
        "#,
    )
    .bind(&scope.guild_id)
    .bind(scope.cohort_start_date)
    .bind(scope.cohort_end_date)
    .execute(pool)
    .await
    .context("failed to rebuild activation_funnel_daily")?;

    Ok(())
}

async fn rebuild_guild_summary_daily_in_scope(
    pool: &PgPool,
    scope: &BackfillRebuildScope,
) -> Result<()> {
    sqlx::query(
        r#"
        DELETE FROM guild_summary_daily
        WHERE guild_id = $1
          AND summary_date >= $2
          AND summary_date <= $3
        "#,
    )
    .bind(&scope.guild_id)
    .bind(scope.cohort_start_date)
    .bind(scope.activity_end_date)
    .execute(pool)
    .await
    .context("failed to clear guild_summary_daily for rebuild")?;

    sqlx::query(
        r#"
        INSERT INTO guild_summary_daily (
            guild_id,
            summary_date,
            messages,
            active_members,
            joined_members,
            left_members,
            onboarded_members,
            last_message_at
        )
        SELECT
            guild_id,
            summary_date,
            SUM(messages)::BIGINT AS messages,
            SUM(active_members)::BIGINT AS active_members,
            SUM(joined_members)::BIGINT AS joined_members,
            SUM(left_members)::BIGINT AS left_members,
            SUM(onboarded_members)::BIGINT AS onboarded_members,
            MAX(last_message_at) AS last_message_at
        FROM (
            SELECT
                guild_id,
                activity_date AS summary_date,
                messages,
                0::BIGINT AS active_members,
                0::BIGINT AS joined_members,
                0::BIGINT AS left_members,
                0::BIGINT AS onboarded_members,
                last_message_at
            FROM guild_daily_activity
            WHERE guild_id = $1
              AND activity_date >= $2
              AND activity_date <= $3
            UNION ALL
            SELECT
                guild_id,
                activity_date AS summary_date,
                0::BIGINT AS messages,
                COUNT(*)::BIGINT AS active_members,
                0::BIGINT AS joined_members,
                0::BIGINT AS left_members,
                0::BIGINT AS onboarded_members,
                activity_date::timestamp AT TIME ZONE 'UTC' AS last_message_at
            FROM guild_daily_active_members
            WHERE guild_id = $1
              AND activity_date >= $2
              AND activity_date <= $3
            GROUP BY guild_id, activity_date
            UNION ALL
            SELECT
                guild_id,
                DATE(joined_at) AS summary_date,
                0::BIGINT AS messages,
                0::BIGINT AS active_members,
                COUNT(*)::BIGINT AS joined_members,
                0::BIGINT AS left_members,
                0::BIGINT AS onboarded_members,
                DATE(joined_at)::timestamp AT TIME ZONE 'UTC' AS last_message_at
            FROM guild_joined_members
            WHERE guild_id = $1
              AND DATE(joined_at) >= $2
              AND DATE(joined_at) <= $3
            GROUP BY guild_id, DATE(joined_at)
            UNION ALL
            SELECT
                guild_id,
                DATE(left_at) AS summary_date,
                0::BIGINT AS messages,
                0::BIGINT AS active_members,
                0::BIGINT AS joined_members,
                COUNT(*)::BIGINT AS left_members,
                0::BIGINT AS onboarded_members,
                DATE(left_at)::timestamp AT TIME ZONE 'UTC' AS last_message_at
            FROM guild_left_members
            WHERE guild_id = $1
              AND DATE(left_at) >= $2
              AND DATE(left_at) <= $3
            GROUP BY guild_id, DATE(left_at)
            UNION ALL
            SELECT
                lifecycle.guild_id,
                DATE(lifecycle.joined_at) AS summary_date,
                0::BIGINT AS messages,
                0::BIGINT AS active_members,
                0::BIGINT AS joined_members,
                0::BIGINT AS left_members,
                COUNT(*)::BIGINT AS onboarded_members,
                DATE(lifecycle.joined_at)::timestamp AT TIME ZONE 'UTC' AS last_message_at
            FROM guild_onboarded_members AS onboarded
            INNER JOIN member_lifecycle AS lifecycle
                ON lifecycle.guild_id = onboarded.guild_id
               AND lifecycle.member_id = onboarded.member_id
            WHERE onboarded.guild_id = $1
              AND lifecycle.joined_at IS NOT NULL
              AND DATE(lifecycle.joined_at) >= $2
              AND DATE(lifecycle.joined_at) <= $3
            GROUP BY lifecycle.guild_id, DATE(lifecycle.joined_at)
        ) AS combined
        GROUP BY guild_id, summary_date
        "#,
    )
    .bind(&scope.guild_id)
    .bind(scope.cohort_start_date)
    .bind(scope.activity_end_date)
    .execute(pool)
    .await
    .context("failed to rebuild guild_summary_daily")?;

    Ok(())
}

async fn recount_public_message_count(pool: &PgPool) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO public_stats_cache (
            cache_key,
            messages_tracked,
            servers,
            members,
            refreshed_at
        )
        SELECT
            'global',
            COALESCE((SELECT COUNT(*) FROM message_index), 0)::BIGINT,
            COALESCE((
                SELECT COUNT(*)
                FROM guild_inventory
                WHERE is_active = TRUE
            ), 0)::BIGINT,
            COALESCE((
                SELECT SUM(member_count)
                FROM guild_inventory
                WHERE is_active = TRUE
            ), 0)::BIGINT,
            NOW()
        ON CONFLICT (cache_key)
        DO UPDATE SET
            messages_tracked = EXCLUDED.messages_tracked,
            servers = EXCLUDED.servers,
            members = EXCLUDED.members,
            refreshed_at = NOW()
        "#,
    )
    .execute(pool)
    .await
    .context("failed to recount public message count")?;

    Ok(())
}

async fn invalidate_dashboard_message_caches(queue: &RedisEventQueue, guild_id: &str) {
    for resource in ["summary_health", "retention_snapshot", "hotspots"] {
        for days in 1..=90 {
            let cache_key = format!("guild:{guild_id}:{resource}:{days}");
            if let Err(error) = queue.del_key(&cache_key).await {
                warn!(cache_key, ?error, "failed to invalidate dashboard cache");
            }
        }
    }
}

async fn ensure_analytics_schema(pool: &PgPool) -> Result<()> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS guild_inventory (
            guild_id TEXT PRIMARY KEY,
            guild_name TEXT NOT NULL,
            owner_id TEXT NOT NULL,
            member_count BIGINT NOT NULL DEFAULT 0,
            is_active BOOLEAN NOT NULL DEFAULT TRUE,
            last_seen_at TIMESTAMPTZ NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create guild_inventory")?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS channel_inventory (
            guild_id TEXT NOT NULL,
            channel_id TEXT NOT NULL,
            channel_name TEXT NOT NULL,
            channel_kind INTEGER NOT NULL DEFAULT 0,
            last_synced_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            PRIMARY KEY (guild_id, channel_id)
        );
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create channel_inventory")?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_channel_inventory_guild_name
            ON channel_inventory (guild_id, channel_name ASC);
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create channel_inventory index")?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS member_lifecycle (
            guild_id TEXT NOT NULL,
            member_id TEXT NOT NULL,
            joined_at TIMESTAMPTZ NULL,
            left_at TIMESTAMPTZ NULL,
            first_seen_at TIMESTAMPTZ NOT NULL,
            first_message_at TIMESTAMPTZ NULL,
            first_reaction_at TIMESTAMPTZ NULL,
            first_voice_at TIMESTAMPTZ NULL,
            first_role_at TIMESTAMPTZ NULL,
            is_pending BOOLEAN NOT NULL DEFAULT FALSE,
            PRIMARY KEY (guild_id, member_id)
        );
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create member_lifecycle")?;

    sqlx::query(
        r#"
        ALTER TABLE member_lifecycle
            ADD COLUMN IF NOT EXISTS first_reaction_at TIMESTAMPTZ NULL,
            ADD COLUMN IF NOT EXISTS first_voice_at TIMESTAMPTZ NULL,
            ADD COLUMN IF NOT EXISTS first_role_at TIMESTAMPTZ NULL;
        "#,
    )
    .execute(pool)
    .await
    .context("failed to alter member_lifecycle")?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS member_daily_activity (
            guild_id TEXT NOT NULL,
            member_id TEXT NOT NULL,
            activity_date DATE NOT NULL,
            messages_sent INTEGER NOT NULL DEFAULT 0,
            reactions_added INTEGER NOT NULL DEFAULT 0,
            voice_seconds BIGINT NOT NULL DEFAULT 0,
            active_channels INTEGER NOT NULL DEFAULT 0,
            was_active BOOLEAN NOT NULL DEFAULT TRUE,
            last_channel_id TEXT NULL,
            last_active_at TIMESTAMPTZ NOT NULL,
            PRIMARY KEY (guild_id, member_id, activity_date)
        );
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create member_daily_activity")?;

    sqlx::query(
        r#"
        ALTER TABLE member_daily_activity
            ADD COLUMN IF NOT EXISTS voice_seconds BIGINT NOT NULL DEFAULT 0,
            ADD COLUMN IF NOT EXISTS was_active BOOLEAN NOT NULL DEFAULT TRUE;
        "#,
    )
    .execute(pool)
    .await
    .context("failed to alter member_daily_activity")?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS channel_daily_activity (
            guild_id TEXT NOT NULL,
            channel_id TEXT NOT NULL,
            activity_date DATE NOT NULL,
            messages INTEGER NOT NULL DEFAULT 0,
            unique_senders INTEGER NOT NULL DEFAULT 0,
            replies INTEGER NOT NULL DEFAULT 0,
            response_seconds_total BIGINT NOT NULL DEFAULT 0,
            response_samples INTEGER NOT NULL DEFAULT 0,
            last_message_at TIMESTAMPTZ NOT NULL,
            PRIMARY KEY (guild_id, channel_id, activity_date)
        );
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create channel_daily_activity")?;

    sqlx::query(
        r#"
        ALTER TABLE channel_daily_activity
            ADD COLUMN IF NOT EXISTS replies INTEGER NOT NULL DEFAULT 0,
            ADD COLUMN IF NOT EXISTS response_seconds_total BIGINT NOT NULL DEFAULT 0,
            ADD COLUMN IF NOT EXISTS response_samples INTEGER NOT NULL DEFAULT 0;
        "#,
    )
    .execute(pool)
    .await
    .context("failed to alter channel_daily_activity")?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS channel_health_daily (
            guild_id TEXT NOT NULL,
            channel_id TEXT NOT NULL,
            activity_date DATE NOT NULL,
            messages BIGINT NOT NULL DEFAULT 0,
            unique_senders BIGINT NOT NULL DEFAULT 0,
            replies BIGINT NOT NULL DEFAULT 0,
            response_seconds_total BIGINT NOT NULL DEFAULT 0,
            response_samples BIGINT NOT NULL DEFAULT 0,
            previous_day_messages BIGINT NOT NULL DEFAULT 0,
            health_score DOUBLE PRECISION NOT NULL DEFAULT 0,
            PRIMARY KEY (guild_id, channel_id, activity_date)
        );
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create channel_health_daily")?;

    sqlx::query(
        r#"
        ALTER TABLE channel_health_daily
            ADD COLUMN IF NOT EXISTS previous_day_messages BIGINT NOT NULL DEFAULT 0,
            ADD COLUMN IF NOT EXISTS health_score DOUBLE PRECISION NOT NULL DEFAULT 0;
        "#,
    )
    .execute(pool)
    .await
    .context("failed to alter channel_health_daily")?;

    sqlx::query(
        r#"
        INSERT INTO channel_health_daily (
            guild_id,
            channel_id,
            activity_date,
            messages,
            unique_senders,
            replies,
            response_seconds_total,
            response_samples,
            previous_day_messages,
            health_score
        )
        SELECT
            current.guild_id,
            current.channel_id,
            current.activity_date,
            current.messages::BIGINT,
            current.unique_senders::BIGINT,
            current.replies::BIGINT,
            current.response_seconds_total::BIGINT,
            current.response_samples::BIGINT,
            COALESCE(previous.messages, 0)::BIGINT AS previous_day_messages,
            0
        FROM channel_daily_activity AS current
        LEFT JOIN channel_daily_activity AS previous
            ON previous.guild_id = current.guild_id
           AND previous.channel_id = current.channel_id
           AND previous.activity_date = current.activity_date - 1
        WHERE NOT EXISTS (
            SELECT 1
            FROM channel_health_daily
            LIMIT 1
        )
        ON CONFLICT (guild_id, channel_id, activity_date) DO NOTHING;
        "#,
    )
    .execute(pool)
    .await
    .context("failed to bootstrap channel_health_daily")?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS channel_daily_unique_senders (
            guild_id TEXT NOT NULL,
            channel_id TEXT NOT NULL,
            activity_date DATE NOT NULL,
            member_id TEXT NOT NULL,
            PRIMARY KEY (guild_id, channel_id, activity_date, member_id)
        );
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create channel_daily_unique_senders")?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS guild_daily_active_members (
            guild_id TEXT NOT NULL,
            activity_date DATE NOT NULL,
            member_id TEXT NOT NULL,
            PRIMARY KEY (guild_id, activity_date, member_id)
        );
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create guild_daily_active_members")?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS guild_joined_members (
            guild_id TEXT NOT NULL,
            member_id TEXT NOT NULL,
            joined_at TIMESTAMPTZ NOT NULL,
            PRIMARY KEY (guild_id, member_id)
        );
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create guild_joined_members")?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS guild_left_members (
            guild_id TEXT NOT NULL,
            member_id TEXT NOT NULL,
            left_at TIMESTAMPTZ NOT NULL,
            PRIMARY KEY (guild_id, member_id)
        );
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create guild_left_members")?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS guild_onboarded_members (
            guild_id TEXT NOT NULL,
            member_id TEXT NOT NULL,
            completed_at TIMESTAMPTZ NOT NULL,
            PRIMARY KEY (guild_id, member_id)
        );
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create guild_onboarded_members")?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS retention_cohorts (
            guild_id TEXT NOT NULL,
            cohort_date DATE NOT NULL,
            joined_members BIGINT NOT NULL DEFAULT 0,
            d7_retained_members BIGINT NOT NULL DEFAULT 0,
            d30_retained_members BIGINT NOT NULL DEFAULT 0,
            PRIMARY KEY (guild_id, cohort_date)
        );
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create retention_cohorts")?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_retention_cohorts_guild_date
            ON retention_cohorts (guild_id, cohort_date DESC);
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create retention_cohorts index")?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS retention_cohort_d7_members (
            guild_id TEXT NOT NULL,
            member_id TEXT NOT NULL,
            cohort_date DATE NOT NULL,
            PRIMARY KEY (guild_id, member_id, cohort_date)
        );
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create retention_cohort_d7_members")?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS retention_cohort_d30_members (
            guild_id TEXT NOT NULL,
            member_id TEXT NOT NULL,
            cohort_date DATE NOT NULL,
            PRIMARY KEY (guild_id, member_id, cohort_date)
        );
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create retention_cohort_d30_members")?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS channel_retention_daily (
            guild_id TEXT NOT NULL,
            channel_id TEXT NOT NULL,
            cohort_date DATE NOT NULL,
            d7_retained_members BIGINT NOT NULL DEFAULT 0,
            d30_retained_members BIGINT NOT NULL DEFAULT 0,
            PRIMARY KEY (guild_id, channel_id, cohort_date)
        );
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create channel_retention_daily")?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_channel_retention_daily_guild_date
            ON channel_retention_daily (guild_id, cohort_date DESC, channel_id);
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create channel_retention_daily index")?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS channel_retention_d7_members (
            guild_id TEXT NOT NULL,
            channel_id TEXT NOT NULL,
            member_id TEXT NOT NULL,
            cohort_date DATE NOT NULL,
            PRIMARY KEY (guild_id, channel_id, member_id, cohort_date)
        );
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create channel_retention_d7_members")?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS channel_retention_d30_members (
            guild_id TEXT NOT NULL,
            channel_id TEXT NOT NULL,
            member_id TEXT NOT NULL,
            cohort_date DATE NOT NULL,
            PRIMARY KEY (guild_id, channel_id, member_id, cohort_date)
        );
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create channel_retention_d30_members")?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS activation_funnel_daily (
            guild_id TEXT NOT NULL,
            cohort_date DATE NOT NULL,
            joined_members BIGINT NOT NULL DEFAULT 0,
            got_role_members BIGINT NOT NULL DEFAULT 0,
            first_message_members BIGINT NOT NULL DEFAULT 0,
            first_reaction_members BIGINT NOT NULL DEFAULT 0,
            first_voice_members BIGINT NOT NULL DEFAULT 0,
            returned_next_week_members BIGINT NOT NULL DEFAULT 0,
            PRIMARY KEY (guild_id, cohort_date)
        );
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create activation_funnel_daily")?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_activation_funnel_daily_guild_date
            ON activation_funnel_daily (guild_id, cohort_date DESC);
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create activation_funnel_daily index")?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS activation_funnel_role_members (
            guild_id TEXT NOT NULL,
            member_id TEXT NOT NULL,
            cohort_date DATE NOT NULL,
            PRIMARY KEY (guild_id, member_id, cohort_date)
        );
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create activation_funnel_role_members")?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS activation_funnel_message_members (
            guild_id TEXT NOT NULL,
            member_id TEXT NOT NULL,
            cohort_date DATE NOT NULL,
            PRIMARY KEY (guild_id, member_id, cohort_date)
        );
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create activation_funnel_message_members")?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS activation_funnel_reaction_members (
            guild_id TEXT NOT NULL,
            member_id TEXT NOT NULL,
            cohort_date DATE NOT NULL,
            PRIMARY KEY (guild_id, member_id, cohort_date)
        );
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create activation_funnel_reaction_members")?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS activation_funnel_voice_members (
            guild_id TEXT NOT NULL,
            member_id TEXT NOT NULL,
            cohort_date DATE NOT NULL,
            PRIMARY KEY (guild_id, member_id, cohort_date)
        );
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create activation_funnel_voice_members")?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS active_voice_sessions (
            guild_id TEXT NOT NULL,
            member_id TEXT NOT NULL,
            channel_id TEXT NOT NULL,
            started_at TIMESTAMPTZ NOT NULL,
            PRIMARY KEY (guild_id, member_id)
        );
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create active_voice_sessions")?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS voice_state_events (
            event_id UUID PRIMARY KEY,
            guild_id TEXT NOT NULL,
            member_id TEXT NOT NULL,
            old_channel_id TEXT NULL,
            new_channel_id TEXT NULL,
            occurred_at TIMESTAMPTZ NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create voice_state_events")?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS voice_sessions (
            guild_id TEXT NOT NULL,
            member_id TEXT NOT NULL,
            channel_id TEXT NOT NULL,
            started_at TIMESTAMPTZ NOT NULL,
            ended_at TIMESTAMPTZ NOT NULL,
            duration_seconds BIGINT NOT NULL,
            PRIMARY KEY (guild_id, member_id, started_at)
        );
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create voice_sessions")?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS message_index (
            message_id TEXT PRIMARY KEY,
            guild_id TEXT NOT NULL,
            channel_id TEXT NOT NULL,
            author_id TEXT NOT NULL,
            is_bot BOOLEAN NOT NULL DEFAULT FALSE,
            is_reply BOOLEAN NOT NULL DEFAULT FALSE,
            attachment_count INTEGER NOT NULL DEFAULT 0,
            content_length INTEGER NOT NULL DEFAULT 0,
            occurred_at TIMESTAMPTZ NOT NULL,
            source TEXT NOT NULL,
            indexed_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        );
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create message_index")?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_message_index_guild_occurred_at
            ON message_index (guild_id, occurred_at DESC);
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create message_index guild index")?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_message_index_guild_channel_occurred_at
            ON message_index (guild_id, channel_id, occurred_at DESC);
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create message_index guild channel index")?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS guild_daily_activity (
            guild_id TEXT NOT NULL,
            activity_date DATE NOT NULL,
            messages BIGINT NOT NULL DEFAULT 0,
            last_message_at TIMESTAMPTZ NOT NULL,
            PRIMARY KEY (guild_id, activity_date)
        );
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create guild_daily_activity")?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_guild_daily_activity_guild_date
            ON guild_daily_activity (guild_id, activity_date DESC);
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create guild_daily_activity index")?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS guild_summary_daily (
            guild_id TEXT NOT NULL,
            summary_date DATE NOT NULL,
            messages BIGINT NOT NULL DEFAULT 0,
            active_members BIGINT NOT NULL DEFAULT 0,
            joined_members BIGINT NOT NULL DEFAULT 0,
            left_members BIGINT NOT NULL DEFAULT 0,
            onboarded_members BIGINT NOT NULL DEFAULT 0,
            last_message_at TIMESTAMPTZ NOT NULL,
            PRIMARY KEY (guild_id, summary_date)
        );
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create guild_summary_daily")?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_guild_summary_daily_guild_date
            ON guild_summary_daily (guild_id, summary_date DESC);
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create guild_summary_daily index")?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS public_stats_cache (
            cache_key TEXT PRIMARY KEY,
            messages_tracked BIGINT NOT NULL DEFAULT 0,
            servers BIGINT NOT NULL DEFAULT 0,
            members BIGINT NOT NULL DEFAULT 0,
            refreshed_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        );
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create public_stats_cache")?;

    sqlx::query(
        r#"
        INSERT INTO message_index (
            message_id,
            guild_id,
            channel_id,
            author_id,
            is_bot,
            is_reply,
            attachment_count,
            content_length,
            occurred_at,
            source
        )
        SELECT
            payload_json->'payload'->'data'->>'message_id',
            guild_id,
            COALESCE(channel_id, ''),
            COALESCE(payload_json->'payload'->'data'->>'author_id', user_id, ''),
            COALESCE((payload_json->'payload'->'data'->>'is_bot')::BOOLEAN, FALSE),
            COALESCE((payload_json->'payload'->'data'->>'is_reply')::BOOLEAN, FALSE),
            COALESCE((payload_json->'payload'->'data'->>'attachment_count')::INTEGER, 0),
            COALESCE((payload_json->'payload'->'data'->>'content_length')::INTEGER, 0),
            occurred_at,
            'raw_event_bootstrap'
        FROM raw_events
        WHERE event_name = 'message_created'
          AND payload_json->'payload'->'data'->>'message_id' IS NOT NULL
        ON CONFLICT (message_id) DO NOTHING;
        "#,
    )
    .execute(pool)
    .await
    .context("failed to bootstrap message_index from raw_events")?;

    sqlx::query(
        r#"
        INSERT INTO guild_daily_activity (
            guild_id,
            activity_date,
            messages,
            last_message_at
        )
        SELECT
            guild_id,
            DATE(occurred_at) AS activity_date,
            COUNT(*)::BIGINT AS messages,
            MAX(occurred_at) AS last_message_at
        FROM message_index
        WHERE NOT EXISTS (
            SELECT 1
            FROM guild_daily_activity
            LIMIT 1
        )
        GROUP BY guild_id, DATE(occurred_at)
        ON CONFLICT (guild_id, activity_date) DO NOTHING;
        "#,
    )
    .execute(pool)
    .await
    .context("failed to bootstrap guild_daily_activity")?;

    sqlx::query(
        r#"
        INSERT INTO guild_daily_active_members (
            guild_id,
            activity_date,
            member_id
        )
        SELECT guild_id, activity_date, member_id
        FROM member_daily_activity
        WHERE NOT EXISTS (
            SELECT 1
            FROM guild_daily_active_members
            LIMIT 1
        )
        ON CONFLICT DO NOTHING;
        "#,
    )
    .execute(pool)
    .await
    .context("failed to bootstrap guild_daily_active_members")?;

    sqlx::query(
        r#"
        INSERT INTO guild_joined_members (
            guild_id,
            member_id,
            joined_at
        )
        SELECT guild_id, member_id, joined_at
        FROM member_lifecycle
        WHERE joined_at IS NOT NULL
          AND NOT EXISTS (
              SELECT 1
              FROM guild_joined_members
              LIMIT 1
          )
        ON CONFLICT DO NOTHING;
        "#,
    )
    .execute(pool)
    .await
    .context("failed to bootstrap guild_joined_members")?;

    sqlx::query(
        r#"
        INSERT INTO guild_left_members (
            guild_id,
            member_id,
            left_at
        )
        SELECT guild_id, member_id, left_at
        FROM member_lifecycle
        WHERE left_at IS NOT NULL
          AND NOT EXISTS (
              SELECT 1
              FROM guild_left_members
              LIMIT 1
          )
        ON CONFLICT DO NOTHING;
        "#,
    )
    .execute(pool)
    .await
    .context("failed to bootstrap guild_left_members")?;

    sqlx::query(
        r#"
        INSERT INTO guild_onboarded_members (
            guild_id,
            member_id,
            completed_at
        )
        SELECT guild_id, member_id, GREATEST(first_role_at, first_message_at) AS completed_at
        FROM member_lifecycle
        WHERE first_role_at IS NOT NULL
          AND first_message_at IS NOT NULL
          AND NOT EXISTS (
              SELECT 1
              FROM guild_onboarded_members
              LIMIT 1
          )
        ON CONFLICT DO NOTHING;
        "#,
    )
    .execute(pool)
    .await
    .context("failed to bootstrap guild_onboarded_members")?;

    sqlx::query(
        r#"
        INSERT INTO retention_cohort_d7_members (
            guild_id,
            member_id,
            cohort_date
        )
        SELECT DISTINCT
            lifecycle.guild_id,
            lifecycle.member_id,
            DATE(lifecycle.joined_at) AS cohort_date
        FROM member_lifecycle AS lifecycle
        INNER JOIN member_daily_activity AS activity
            ON activity.guild_id = lifecycle.guild_id
           AND activity.member_id = lifecycle.member_id
        WHERE lifecycle.joined_at IS NOT NULL
          AND activity.activity_date >= DATE(lifecycle.joined_at) + 7
          AND activity.activity_date < DATE(lifecycle.joined_at) + 14
          AND NOT EXISTS (
              SELECT 1
              FROM retention_cohort_d7_members
              LIMIT 1
          )
        ON CONFLICT DO NOTHING;
        "#,
    )
    .execute(pool)
    .await
    .context("failed to bootstrap retention_cohort_d7_members")?;

    sqlx::query(
        r#"
        INSERT INTO retention_cohort_d30_members (
            guild_id,
            member_id,
            cohort_date
        )
        SELECT DISTINCT
            lifecycle.guild_id,
            lifecycle.member_id,
            DATE(lifecycle.joined_at) AS cohort_date
        FROM member_lifecycle AS lifecycle
        INNER JOIN member_daily_activity AS activity
            ON activity.guild_id = lifecycle.guild_id
           AND activity.member_id = lifecycle.member_id
        WHERE lifecycle.joined_at IS NOT NULL
          AND activity.activity_date >= DATE(lifecycle.joined_at) + 30
          AND activity.activity_date < DATE(lifecycle.joined_at) + 37
          AND NOT EXISTS (
              SELECT 1
              FROM retention_cohort_d30_members
              LIMIT 1
          )
        ON CONFLICT DO NOTHING;
        "#,
    )
    .execute(pool)
    .await
    .context("failed to bootstrap retention_cohort_d30_members")?;

    sqlx::query(
        r#"
        INSERT INTO retention_cohorts (
            guild_id,
            cohort_date,
            joined_members,
            d7_retained_members,
            d30_retained_members
        )
        SELECT
            guild_id,
            cohort_date,
            SUM(joined_members)::BIGINT AS joined_members,
            SUM(d7_retained_members)::BIGINT AS d7_retained_members,
            SUM(d30_retained_members)::BIGINT AS d30_retained_members
        FROM (
            SELECT
                guild_id,
                DATE(joined_at) AS cohort_date,
                COUNT(*)::BIGINT AS joined_members,
                0::BIGINT AS d7_retained_members,
                0::BIGINT AS d30_retained_members
            FROM member_lifecycle
            WHERE joined_at IS NOT NULL
            GROUP BY guild_id, DATE(joined_at)
            UNION ALL
            SELECT
                guild_id,
                cohort_date,
                0::BIGINT AS joined_members,
                COUNT(*)::BIGINT AS d7_retained_members,
                0::BIGINT AS d30_retained_members
            FROM retention_cohort_d7_members
            GROUP BY guild_id, cohort_date
            UNION ALL
            SELECT
                guild_id,
                cohort_date,
                0::BIGINT AS joined_members,
                0::BIGINT AS d7_retained_members,
                COUNT(*)::BIGINT AS d30_retained_members
            FROM retention_cohort_d30_members
            GROUP BY guild_id, cohort_date
        ) AS combined
        WHERE NOT EXISTS (
            SELECT 1
            FROM retention_cohorts
            LIMIT 1
        )
        GROUP BY guild_id, cohort_date
        ON CONFLICT (guild_id, cohort_date) DO NOTHING;
        "#,
    )
    .execute(pool)
    .await
    .context("failed to bootstrap retention_cohorts")?;

    sqlx::query(
        r#"
        INSERT INTO channel_retention_d7_members (
            guild_id,
            channel_id,
            member_id,
            cohort_date
        )
        SELECT DISTINCT
            retained.guild_id,
            message.channel_id,
            retained.member_id,
            retained.cohort_date
        FROM retention_cohort_d7_members AS retained
        INNER JOIN member_lifecycle AS lifecycle
            ON lifecycle.guild_id = retained.guild_id
           AND lifecycle.member_id = retained.member_id
        INNER JOIN message_index AS message
            ON message.guild_id = retained.guild_id
           AND message.author_id = retained.member_id
        WHERE lifecycle.joined_at IS NOT NULL
          AND message.channel_id <> ''
          AND message.occurred_at >= lifecycle.joined_at
          AND message.occurred_at < lifecycle.joined_at + INTERVAL '7 days'
          AND NOT EXISTS (
              SELECT 1
              FROM channel_retention_d7_members
              LIMIT 1
          )
        ON CONFLICT DO NOTHING;
        "#,
    )
    .execute(pool)
    .await
    .context("failed to bootstrap channel_retention_d7_members")?;

    sqlx::query(
        r#"
        INSERT INTO channel_retention_d30_members (
            guild_id,
            channel_id,
            member_id,
            cohort_date
        )
        SELECT DISTINCT
            retained.guild_id,
            message.channel_id,
            retained.member_id,
            retained.cohort_date
        FROM retention_cohort_d30_members AS retained
        INNER JOIN member_lifecycle AS lifecycle
            ON lifecycle.guild_id = retained.guild_id
           AND lifecycle.member_id = retained.member_id
        INNER JOIN message_index AS message
            ON message.guild_id = retained.guild_id
           AND message.author_id = retained.member_id
        WHERE lifecycle.joined_at IS NOT NULL
          AND message.channel_id <> ''
          AND message.occurred_at >= lifecycle.joined_at
          AND message.occurred_at < lifecycle.joined_at + INTERVAL '7 days'
          AND NOT EXISTS (
              SELECT 1
              FROM channel_retention_d30_members
              LIMIT 1
          )
        ON CONFLICT DO NOTHING;
        "#,
    )
    .execute(pool)
    .await
    .context("failed to bootstrap channel_retention_d30_members")?;

    sqlx::query(
        r#"
        INSERT INTO channel_retention_daily (
            guild_id,
            channel_id,
            cohort_date,
            d7_retained_members,
            d30_retained_members
        )
        SELECT
            guild_id,
            channel_id,
            cohort_date,
            SUM(d7_retained_members)::BIGINT AS d7_retained_members,
            SUM(d30_retained_members)::BIGINT AS d30_retained_members
        FROM (
            SELECT
                guild_id,
                channel_id,
                cohort_date,
                COUNT(*)::BIGINT AS d7_retained_members,
                0::BIGINT AS d30_retained_members
            FROM channel_retention_d7_members
            GROUP BY guild_id, channel_id, cohort_date
            UNION ALL
            SELECT
                guild_id,
                channel_id,
                cohort_date,
                0::BIGINT AS d7_retained_members,
                COUNT(*)::BIGINT AS d30_retained_members
            FROM channel_retention_d30_members
            GROUP BY guild_id, channel_id, cohort_date
        ) AS combined
        WHERE NOT EXISTS (
            SELECT 1
            FROM channel_retention_daily
            LIMIT 1
        )
        GROUP BY guild_id, channel_id, cohort_date
        ON CONFLICT (guild_id, channel_id, cohort_date) DO NOTHING;
        "#,
    )
    .execute(pool)
    .await
    .context("failed to bootstrap channel_retention_daily")?;

    sqlx::query(
        r#"
        INSERT INTO activation_funnel_role_members (
            guild_id,
            member_id,
            cohort_date
        )
        SELECT
            guild_id,
            member_id,
            DATE(joined_at) AS cohort_date
        FROM member_lifecycle
        WHERE joined_at IS NOT NULL
          AND first_role_at IS NOT NULL
          AND first_role_at >= joined_at
          AND NOT EXISTS (
              SELECT 1
              FROM activation_funnel_role_members
              LIMIT 1
          )
        ON CONFLICT DO NOTHING;
        "#,
    )
    .execute(pool)
    .await
    .context("failed to bootstrap activation_funnel_role_members")?;

    sqlx::query(
        r#"
        INSERT INTO activation_funnel_message_members (
            guild_id,
            member_id,
            cohort_date
        )
        SELECT
            guild_id,
            member_id,
            DATE(joined_at) AS cohort_date
        FROM member_lifecycle
        WHERE joined_at IS NOT NULL
          AND first_message_at IS NOT NULL
          AND first_message_at >= joined_at
          AND NOT EXISTS (
              SELECT 1
              FROM activation_funnel_message_members
              LIMIT 1
          )
        ON CONFLICT DO NOTHING;
        "#,
    )
    .execute(pool)
    .await
    .context("failed to bootstrap activation_funnel_message_members")?;

    sqlx::query(
        r#"
        INSERT INTO activation_funnel_reaction_members (
            guild_id,
            member_id,
            cohort_date
        )
        SELECT
            guild_id,
            member_id,
            DATE(joined_at) AS cohort_date
        FROM member_lifecycle
        WHERE joined_at IS NOT NULL
          AND first_reaction_at IS NOT NULL
          AND first_reaction_at >= joined_at
          AND NOT EXISTS (
              SELECT 1
              FROM activation_funnel_reaction_members
              LIMIT 1
          )
        ON CONFLICT DO NOTHING;
        "#,
    )
    .execute(pool)
    .await
    .context("failed to bootstrap activation_funnel_reaction_members")?;

    sqlx::query(
        r#"
        INSERT INTO activation_funnel_voice_members (
            guild_id,
            member_id,
            cohort_date
        )
        SELECT
            guild_id,
            member_id,
            DATE(joined_at) AS cohort_date
        FROM member_lifecycle
        WHERE joined_at IS NOT NULL
          AND first_voice_at IS NOT NULL
          AND first_voice_at >= joined_at
          AND NOT EXISTS (
              SELECT 1
              FROM activation_funnel_voice_members
              LIMIT 1
          )
        ON CONFLICT DO NOTHING;
        "#,
    )
    .execute(pool)
    .await
    .context("failed to bootstrap activation_funnel_voice_members")?;

    sqlx::query(
        r#"
        INSERT INTO activation_funnel_daily (
            guild_id,
            cohort_date,
            joined_members,
            got_role_members,
            first_message_members,
            first_reaction_members,
            first_voice_members,
            returned_next_week_members
        )
        SELECT
            guild_id,
            cohort_date,
            SUM(joined_members)::BIGINT AS joined_members,
            SUM(got_role_members)::BIGINT AS got_role_members,
            SUM(first_message_members)::BIGINT AS first_message_members,
            SUM(first_reaction_members)::BIGINT AS first_reaction_members,
            SUM(first_voice_members)::BIGINT AS first_voice_members,
            SUM(returned_next_week_members)::BIGINT AS returned_next_week_members
        FROM (
            SELECT
                guild_id,
                DATE(joined_at) AS cohort_date,
                COUNT(*)::BIGINT AS joined_members,
                0::BIGINT AS got_role_members,
                0::BIGINT AS first_message_members,
                0::BIGINT AS first_reaction_members,
                0::BIGINT AS first_voice_members,
                0::BIGINT AS returned_next_week_members
            FROM member_lifecycle
            WHERE joined_at IS NOT NULL
            GROUP BY guild_id, DATE(joined_at)
            UNION ALL
            SELECT
                guild_id,
                cohort_date,
                0::BIGINT AS joined_members,
                COUNT(*)::BIGINT AS got_role_members,
                0::BIGINT AS first_message_members,
                0::BIGINT AS first_reaction_members,
                0::BIGINT AS first_voice_members,
                0::BIGINT AS returned_next_week_members
            FROM activation_funnel_role_members
            GROUP BY guild_id, cohort_date
            UNION ALL
            SELECT
                guild_id,
                cohort_date,
                0::BIGINT AS joined_members,
                0::BIGINT AS got_role_members,
                COUNT(*)::BIGINT AS first_message_members,
                0::BIGINT AS first_reaction_members,
                0::BIGINT AS first_voice_members,
                0::BIGINT AS returned_next_week_members
            FROM activation_funnel_message_members
            GROUP BY guild_id, cohort_date
            UNION ALL
            SELECT
                guild_id,
                cohort_date,
                0::BIGINT AS joined_members,
                0::BIGINT AS got_role_members,
                0::BIGINT AS first_message_members,
                COUNT(*)::BIGINT AS first_reaction_members,
                0::BIGINT AS first_voice_members,
                0::BIGINT AS returned_next_week_members
            FROM activation_funnel_reaction_members
            GROUP BY guild_id, cohort_date
            UNION ALL
            SELECT
                guild_id,
                cohort_date,
                0::BIGINT AS joined_members,
                0::BIGINT AS got_role_members,
                0::BIGINT AS first_message_members,
                0::BIGINT AS first_reaction_members,
                COUNT(*)::BIGINT AS first_voice_members,
                0::BIGINT AS returned_next_week_members
            FROM activation_funnel_voice_members
            GROUP BY guild_id, cohort_date
            UNION ALL
            SELECT
                guild_id,
                cohort_date,
                0::BIGINT AS joined_members,
                0::BIGINT AS got_role_members,
                0::BIGINT AS first_message_members,
                0::BIGINT AS first_reaction_members,
                0::BIGINT AS first_voice_members,
                COUNT(*)::BIGINT AS returned_next_week_members
            FROM retention_cohort_d7_members
            GROUP BY guild_id, cohort_date
        ) AS combined
        WHERE NOT EXISTS (
            SELECT 1
            FROM activation_funnel_daily
            LIMIT 1
        )
        GROUP BY guild_id, cohort_date
        ON CONFLICT (guild_id, cohort_date) DO NOTHING;
        "#,
    )
    .execute(pool)
    .await
    .context("failed to bootstrap activation_funnel_daily")?;

    sqlx::query(
        r#"
        INSERT INTO guild_summary_daily (
            guild_id,
            summary_date,
            messages,
            active_members,
            joined_members,
            left_members,
            onboarded_members,
            last_message_at
        )
        SELECT
            guild_id,
            summary_date,
            SUM(messages)::BIGINT AS messages,
            SUM(active_members)::BIGINT AS active_members,
            SUM(joined_members)::BIGINT AS joined_members,
            SUM(left_members)::BIGINT AS left_members,
            SUM(onboarded_members)::BIGINT AS onboarded_members,
            MAX(last_message_at) AS last_message_at
        FROM (
            SELECT
                guild_id,
                activity_date AS summary_date,
                messages,
                0::BIGINT AS active_members,
                0::BIGINT AS joined_members,
                0::BIGINT AS left_members,
                0::BIGINT AS onboarded_members,
                last_message_at
            FROM guild_daily_activity
            UNION ALL
            SELECT
                guild_id,
                activity_date AS summary_date,
                0::BIGINT AS messages,
                COUNT(*)::BIGINT AS active_members,
                0::BIGINT AS joined_members,
                0::BIGINT AS left_members,
                0::BIGINT AS onboarded_members,
                activity_date::timestamp AT TIME ZONE 'UTC' AS last_message_at
            FROM guild_daily_active_members
            GROUP BY guild_id, activity_date
            UNION ALL
            SELECT
                guild_id,
                DATE(joined_at) AS summary_date,
                0::BIGINT AS messages,
                0::BIGINT AS active_members,
                COUNT(*)::BIGINT AS joined_members,
                0::BIGINT AS left_members,
                0::BIGINT AS onboarded_members,
                DATE(joined_at)::timestamp AT TIME ZONE 'UTC' AS last_message_at
            FROM guild_joined_members
            GROUP BY guild_id, DATE(joined_at)
            UNION ALL
            SELECT
                guild_id,
                DATE(left_at) AS summary_date,
                0::BIGINT AS messages,
                0::BIGINT AS active_members,
                0::BIGINT AS joined_members,
                COUNT(*)::BIGINT AS left_members,
                0::BIGINT AS onboarded_members,
                DATE(left_at)::timestamp AT TIME ZONE 'UTC' AS last_message_at
            FROM guild_left_members
            GROUP BY guild_id, DATE(left_at)
            UNION ALL
            SELECT
                lifecycle.guild_id,
                DATE(lifecycle.joined_at) AS summary_date,
                0::BIGINT AS messages,
                0::BIGINT AS active_members,
                0::BIGINT AS joined_members,
                0::BIGINT AS left_members,
                COUNT(*)::BIGINT AS onboarded_members,
                DATE(lifecycle.joined_at)::timestamp AT TIME ZONE 'UTC' AS last_message_at
            FROM guild_onboarded_members AS onboarded
            INNER JOIN member_lifecycle AS lifecycle
                ON lifecycle.guild_id = onboarded.guild_id
               AND lifecycle.member_id = onboarded.member_id
            WHERE lifecycle.joined_at IS NOT NULL
            GROUP BY lifecycle.guild_id, DATE(lifecycle.joined_at)
        ) AS combined
        WHERE NOT EXISTS (
            SELECT 1
            FROM guild_summary_daily
            LIMIT 1
        )
        GROUP BY guild_id, summary_date
        ON CONFLICT (guild_id, summary_date) DO NOTHING;
        "#,
    )
    .execute(pool)
    .await
    .context("failed to bootstrap guild_summary_daily")?;

    sqlx::query(
        r#"
        INSERT INTO public_stats_cache (
            cache_key,
            messages_tracked,
            servers,
            members,
            refreshed_at
        )
        SELECT
            'global',
            COALESCE((SELECT COUNT(*) FROM message_index), 0)::BIGINT,
            COALESCE((
                SELECT COUNT(*)
                FROM guild_inventory
                WHERE is_active = TRUE
            ), 0)::BIGINT,
            COALESCE((
                SELECT SUM(member_count)
                FROM guild_inventory
                WHERE is_active = TRUE
            ), 0)::BIGINT,
            NOW()
        WHERE NOT EXISTS (
            SELECT 1
            FROM public_stats_cache
            WHERE cache_key = 'global'
        );
        "#,
    )
    .execute(pool)
    .await
    .context("failed to initialize public_stats_cache")?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS historical_backfill_jobs (
            job_id UUID PRIMARY KEY,
            guild_id TEXT NOT NULL,
            requested_by_user_id TEXT NULL,
            days_requested INTEGER NOT NULL,
            start_at TIMESTAMPTZ NOT NULL,
            end_at TIMESTAMPTZ NOT NULL,
            trigger_source TEXT NOT NULL,
            status TEXT NOT NULL,
            requested_at TIMESTAMPTZ NOT NULL,
            started_at TIMESTAMPTZ NULL,
            completed_at TIMESTAMPTZ NULL,
            last_error TEXT NULL,
            messages_indexed BIGINT NOT NULL DEFAULT 0
        );
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create historical_backfill_jobs")?;

    Ok(())
}

fn is_message_backfillable_channel(channel: &DiscordChannel) -> bool {
    matches!(channel.kind, 0 | 5 | 10 | 11 | 12)
}

fn stream_names() -> [&'static str; 5] {
    [
        "events.guild",
        "events.member",
        "events.message",
        "events.voice",
        BACKFILL_STREAM,
    ]
}

fn date_for(timestamp: DateTime<Utc>) -> NaiveDate {
    timestamp.date_naive()
}

fn start_of_day(activity_date: NaiveDate) -> DateTime<Utc> {
    DateTime::<Utc>::from_naive_utc_and_offset(
        activity_date
            .and_hms_opt(0, 0, 0)
            .expect("midnight should be valid"),
        Utc,
    )
}

fn init_tracing(rust_log: &str) {
    let filter = EnvFilter::try_new(rust_log).unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

async fn run_metrics_server(addr: SocketAddr) -> Result<()> {
    let app = Router::new()
        .route("/metrics", get(metrics))
        .with_state(worker_metrics());
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .context("failed to bind worker metrics listener")?;
    info!(address = %addr, "worker metrics listening");
    axum::serve(listener, app)
        .await
        .context("worker metrics server crashed")
}

async fn metrics(State(metrics): State<&'static WorkerMetrics>) -> impl IntoResponse {
    let encoder = TextEncoder::new();
    let metric_families = metrics.registry.gather();
    let mut body = Vec::new();

    match encoder.encode(&metric_families, &mut body) {
        Ok(()) => (
            HttpStatusCode::OK,
            [("content-type", encoder.format_type().to_string())],
            body,
        )
            .into_response(),
        Err(error) => (
            HttpStatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to encode metrics: {error}"),
        )
            .into_response(),
    }
}

fn worker_metrics() -> &'static WorkerMetrics {
    static WORKER_METRICS: OnceLock<WorkerMetrics> = OnceLock::new();
    WORKER_METRICS.get_or_init(|| {
        let registry = Registry::new_custom(Some("guildest".to_string()), None)
            .expect("failed to create worker metrics registry");
        let backfill_job_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "worker_backfill_job_duration_seconds",
                "Backfill job duration in seconds",
            ),
            &["status"],
        )
        .expect("failed to create backfill duration metric");
        let discord_request_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "worker_discord_request_duration_seconds",
                "Discord API request duration in seconds",
            ),
            &["endpoint", "status"],
        )
        .expect("failed to create discord request duration metric");
        let discord_response_size_bytes = HistogramVec::new(
            HistogramOpts::new(
                "worker_discord_response_size_bytes",
                "Discord API response size in bytes",
            ),
            &["endpoint"],
        )
        .expect("failed to create discord response size metric");
        let discord_requests_total = IntCounterVec::new(
            prometheus::Opts::new(
                "worker_discord_requests_total",
                "Total Discord API requests observed",
            ),
            &["endpoint", "status"],
        )
        .expect("failed to create discord request counter");
        let dead_lettered_deliveries_total = IntCounterVec::new(
            prometheus::Opts::new(
                "worker_dead_lettered_deliveries_total",
                "Deliveries sent to the dead-letter streams",
            ),
            &["stream"],
        )
        .expect("failed to create dead-letter counter");
        let retried_deliveries_total = IntCounterVec::new(
            prometheus::Opts::new(
                "worker_retried_deliveries_total",
                "Deliveries republished for retry",
            ),
            &["stream"],
        )
        .expect("failed to create retried delivery counter");
        let messages_indexed_per_backfill_job = Histogram::with_opts(HistogramOpts::new(
            "worker_messages_indexed_per_backfill_job",
            "Messages indexed by each backfill job",
        ))
        .expect("failed to create backfill messages metric");
        let queue_ready_messages = IntGaugeVec::new(
            prometheus::Opts::new(
                "worker_queue_ready_messages",
                "Messages currently in each worker stream",
            ),
            &["stream"],
        )
        .expect("failed to create queue ready gauge");
        let queue_oldest_ready_age_seconds = IntGaugeVec::new(
            prometheus::Opts::new(
                "worker_queue_oldest_ready_age_seconds",
                "Age in seconds of the oldest message in each worker stream",
            ),
            &["stream"],
        )
        .expect("failed to create queue oldest ready age gauge");
        let queue_pending_messages = IntGaugeVec::new(
            prometheus::Opts::new(
                "worker_queue_pending_messages",
                "Pending unacked messages in each worker consumer group",
            ),
            &["stream"],
        )
        .expect("failed to create queue pending gauge");
        let queue_dead_letter_depth = IntGaugeVec::new(
            prometheus::Opts::new(
                "worker_queue_dead_letter_depth",
                "Messages currently in each dead-letter stream",
            ),
            &["stream"],
        )
        .expect("failed to create dead-letter depth gauge");
        let queue_oldest_dead_letter_age_seconds = IntGaugeVec::new(
            prometheus::Opts::new(
                "worker_queue_oldest_dead_letter_age_seconds",
                "Age in seconds of the oldest message in each dead-letter stream",
            ),
            &["stream"],
        )
        .expect("failed to create oldest dead-letter age gauge");
        let queue_scheduled_retry_depth = IntGaugeVec::new(
            prometheus::Opts::new(
                "worker_queue_scheduled_retry_depth",
                "Messages currently waiting in each scheduled retry set",
            ),
            &["stream"],
        )
        .expect("failed to create scheduled retry depth gauge");
        let queue_scheduled_retry_overdue_seconds = IntGaugeVec::new(
            prometheus::Opts::new(
                "worker_queue_scheduled_retry_overdue_seconds",
                "How many seconds the oldest scheduled retry is overdue by, if any",
            ),
            &["stream"],
        )
        .expect("failed to create scheduled retry overdue gauge");

        registry
            .register(Box::new(backfill_job_duration_seconds.clone()))
            .expect("failed to register backfill duration metric");
        registry
            .register(Box::new(discord_request_duration_seconds.clone()))
            .expect("failed to register discord request duration metric");
        registry
            .register(Box::new(discord_response_size_bytes.clone()))
            .expect("failed to register discord response size metric");
        registry
            .register(Box::new(discord_requests_total.clone()))
            .expect("failed to register discord request counter");
        registry
            .register(Box::new(dead_lettered_deliveries_total.clone()))
            .expect("failed to register dead-letter counter");
        registry
            .register(Box::new(retried_deliveries_total.clone()))
            .expect("failed to register retried delivery counter");
        registry
            .register(Box::new(messages_indexed_per_backfill_job.clone()))
            .expect("failed to register backfill messages metric");
        registry
            .register(Box::new(queue_ready_messages.clone()))
            .expect("failed to register queue ready gauge");
        registry
            .register(Box::new(queue_oldest_ready_age_seconds.clone()))
            .expect("failed to register queue oldest ready age gauge");
        registry
            .register(Box::new(queue_pending_messages.clone()))
            .expect("failed to register queue pending gauge");
        registry
            .register(Box::new(queue_dead_letter_depth.clone()))
            .expect("failed to register dead-letter depth gauge");
        registry
            .register(Box::new(queue_oldest_dead_letter_age_seconds.clone()))
            .expect("failed to register oldest dead-letter age gauge");
        registry
            .register(Box::new(queue_scheduled_retry_depth.clone()))
            .expect("failed to register scheduled retry depth gauge");
        registry
            .register(Box::new(queue_scheduled_retry_overdue_seconds.clone()))
            .expect("failed to register scheduled retry overdue gauge");

        WorkerMetrics {
            backfill_job_duration_seconds,
            dead_lettered_deliveries_total,
            discord_request_duration_seconds,
            discord_response_size_bytes,
            discord_requests_total,
            messages_indexed_per_backfill_job,
            queue_dead_letter_depth,
            queue_oldest_dead_letter_age_seconds,
            queue_oldest_ready_age_seconds,
            queue_pending_messages,
            queue_ready_messages,
            queue_scheduled_retry_depth,
            queue_scheduled_retry_overdue_seconds,
            retried_deliveries_total,
            registry,
        }
    })
}

fn observe_discord_request(
    endpoint: &'static str,
    status: u16,
    duration_seconds: f64,
    response_size_bytes: usize,
) {
    let status = status.to_string();
    let metrics = worker_metrics();
    metrics
        .discord_request_duration_seconds
        .with_label_values(&[endpoint, &status])
        .observe(duration_seconds);
    metrics
        .discord_requests_total
        .with_label_values(&[endpoint, &status])
        .inc();
    metrics
        .discord_response_size_bytes
        .with_label_values(&[endpoint])
        .observe(response_size_bytes as f64);
}

fn observe_backfill_job(status: &'static str, duration_seconds: f64, messages_indexed: i64) {
    let metrics = worker_metrics();
    metrics
        .backfill_job_duration_seconds
        .with_label_values(&[status])
        .observe(duration_seconds);
    metrics
        .messages_indexed_per_backfill_job
        .observe(messages_indexed as f64);
}

fn observe_dead_lettered_delivery(stream: &str) {
    worker_metrics()
        .dead_lettered_deliveries_total
        .with_label_values(&[stream])
        .inc();
}

fn observe_retried_delivery(stream: &str) {
    worker_metrics()
        .retried_deliveries_total
        .with_label_values(&[stream])
        .inc();
}

fn observe_queue_depth(
    stream: &str,
    ready_messages: i64,
    pending_messages: i64,
    dead_letter_depth: i64,
    scheduled_retry_depth: i64,
    oldest_ready_age_seconds: i64,
    oldest_dead_letter_age_seconds: i64,
    scheduled_retry_overdue_seconds: i64,
) {
    let metrics = worker_metrics();
    metrics
        .queue_ready_messages
        .with_label_values(&[stream])
        .set(ready_messages);
    metrics
        .queue_pending_messages
        .with_label_values(&[stream])
        .set(pending_messages);
    metrics
        .queue_dead_letter_depth
        .with_label_values(&[stream])
        .set(dead_letter_depth);
    metrics
        .queue_oldest_ready_age_seconds
        .with_label_values(&[stream])
        .set(oldest_ready_age_seconds);
    metrics
        .queue_oldest_dead_letter_age_seconds
        .with_label_values(&[stream])
        .set(oldest_dead_letter_age_seconds);
    metrics
        .queue_scheduled_retry_depth
        .with_label_values(&[stream])
        .set(scheduled_retry_depth);
    metrics
        .queue_scheduled_retry_overdue_seconds
        .with_label_values(&[stream])
        .set(scheduled_retry_overdue_seconds);
}

async fn refresh_queue_metrics(queue: &RedisEventQueue, stream: &str) -> Result<()> {
    let now_ms = Utc::now().timestamp_millis();
    let ready_messages = queue.stream_len(stream).await?;
    let pending_messages = queue.pending_count(stream, stream).await?;
    let dead_letter_depth = queue.stream_len(&dead_letter_stream_name(stream)).await?;
    let scheduled_retry_depth = queue
        .sorted_set_len(&scheduled_retry_set_name(stream))
        .await?;
    let oldest_ready_age_seconds = queue
        .oldest_stream_entry_ms(stream)
        .await?
        .map(|timestamp_ms| ((now_ms - timestamp_ms).max(0)) / 1000)
        .unwrap_or(0);
    let oldest_dead_letter_age_seconds = queue
        .oldest_stream_entry_ms(&dead_letter_stream_name(stream))
        .await?
        .map(|timestamp_ms| ((now_ms - timestamp_ms).max(0)) / 1000)
        .unwrap_or(0);
    let scheduled_retry_overdue_seconds = queue
        .earliest_sorted_set_score_ms(&scheduled_retry_set_name(stream))
        .await?
        .map(|timestamp_ms| ((now_ms - timestamp_ms).max(0)) / 1000)
        .unwrap_or(0);
    observe_queue_depth(
        stream,
        ready_messages,
        pending_messages,
        dead_letter_depth,
        scheduled_retry_depth,
        oldest_ready_age_seconds,
        oldest_dead_letter_age_seconds,
        scheduled_retry_overdue_seconds,
    );
    Ok(())
}

async fn publish_due_retries(queue: &RedisEventQueue, stream: &str) -> Result<()> {
    let scheduled_key = scheduled_retry_set_name(stream);
    let payloads = queue
        .pop_due_scheduled_messages(
            &scheduled_key,
            Utc::now().timestamp_millis(),
            RETRY_SCHEDULER_BATCH_SIZE,
        )
        .await?;
    let had_payloads = !payloads.is_empty();

    for payload in payloads {
        let retry: ScheduledRetryDelivery =
            serde_json::from_str(&payload).context("failed to decode scheduled retry delivery")?;
        if let Err(error) = queue
            .publish(&retry.delivery.stream, &retry.delivery.payload)
            .await
        {
            let retry_at = Utc::now() + chrono::Duration::seconds(5);
            queue
                .schedule_message(&scheduled_key, &payload, retry_at.timestamp_millis())
                .await
                .context("failed to requeue scheduled retry delivery")?;
            return Err(error).context("failed to publish scheduled retry delivery");
        }

        observe_retried_delivery(&retry.delivery.stream);
    }

    if had_payloads {
        refresh_queue_metrics(queue, stream).await?;
    }

    Ok(())
}

fn dead_letter_stream_name(stream: &str) -> String {
    format!("dead_letter.{stream}")
}

fn scheduled_retry_set_name(stream: &str) -> String {
    format!("scheduled_retry.{stream}")
}

fn retry_backoff_delay(attempts: i64) -> Duration {
    let exponent = (attempts.saturating_sub(1)).clamp(0, 10) as u32;
    let multiplier = 2_u64.saturating_pow(exponent);
    Duration::from_millis(DELIVERY_RETRY_BASE_DELAY_MS.saturating_mul(multiplier))
}

fn retry_state_key(delivery: &QueueDelivery) -> Result<String> {
    if delivery.stream == BACKFILL_STREAM {
        if let Ok(job) = serde_json::from_str::<BackfillJob>(&delivery.payload) {
            return Ok(format!("worker:retry:{}:{}", delivery.stream, job.job_id));
        }
        return Ok(format!(
            "worker:retry:{}:{}",
            delivery.stream,
            payload_fingerprint(&delivery.payload)
        ));
    }

    if let Ok(envelope) = serde_json::from_str::<EventEnvelope>(&delivery.payload) {
        return Ok(format!(
            "worker:retry:{}:{}",
            delivery.stream, envelope.event_id
        ));
    }

    if let Ok(event_ref) = serde_json::from_str::<QueuedEventRef>(&delivery.payload) {
        return Ok(format!(
            "worker:retry:{}:{}",
            delivery.stream, event_ref.event_id
        ));
    }

    Ok(format!(
        "worker:retry:{}:{}",
        delivery.stream,
        payload_fingerprint(&delivery.payload)
    ))
}

fn payload_fingerprint(payload: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    payload.hash(&mut hasher);
    hasher.finish()
}

fn discord_api_url(base: &str, path: &str) -> Result<String> {
    let path = path.trim_start_matches('/');
    let base = base.trim_end_matches('/');
    Ok(format!("{base}/{path}"))
}

fn discord_endpoint_label(url: &str) -> &'static str {
    if url.contains("/guilds/") && url.ends_with("/channels") {
        "guild_channels"
    } else if url.contains("/channels/") && url.contains("/messages") {
        "channel_messages"
    } else {
        "other"
    }
}
