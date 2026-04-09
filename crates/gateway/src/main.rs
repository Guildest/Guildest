use std::{
    net::SocketAddr,
    sync::{Arc, OnceLock},
    time::Instant,
};

use anyhow::{Context, Result};
use axum::{Router, extract::State, response::IntoResponse, routing::get};
use chrono::{DateTime, Utc};
use common::{
    ai::{AiStore, NewAiMessageObservation, PostgresAiStore},
    config::Settings,
    events::{
        EventEnvelope, EventPayload, GuildAvailablePayload, GuildRemovedPayload,
        MemberJoinedPayload, MemberLeftPayload, MemberRolesUpdatedPayload, MessageCreatedPayload,
        ReactionAddedPayload, VoiceStateUpdatedPayload,
    },
    jobs::{AI_CLASSIFY_STREAM, AiClassifyJob},
    queue::{EventQueue, QueuedEventRef, RedisEventQueue},
    store::{PostgresRawEventStore, RawEventStore},
};
use regex::Regex;
use sha2::{Digest, Sha256};
use std::sync::LazyLock;
use prometheus::{Encoder, HistogramOpts, HistogramVec, IntCounterVec, Registry, TextEncoder};
use serenity::{
    Client,
    all::{
        GatewayIntents, Guild, GuildId, GuildMemberUpdateEvent, Member, Message, Reaction, Ready,
        UnavailableGuild, User, VoiceState,
    },
    async_trait,
    prelude::{Context as DiscordContext, EventHandler},
};
use sqlx::postgres::PgPoolOptions;
use tokio::task;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

static METRICS: OnceLock<GatewayMetrics> = OnceLock::new();

const REDACTION_VERSION: &str = "v1";

/// Patterns that identify PII or secrets. Each match is replaced with a placeholder.
static REDACTION_PATTERNS: LazyLock<Vec<(&'static str, Regex)>> = LazyLock::new(|| {
    vec![
        ("EMAIL", Regex::new(r"[a-zA-Z0-9._%+\-]+@[a-zA-Z0-9.\-]+\.[a-zA-Z]{2,}").unwrap()),
        ("PHONE", Regex::new(r"\b(\+?1[\s\-.]?)?\(?\d{3}\)?[\s\-.]?\d{3}[\s\-.]?\d{4}\b").unwrap()),
        ("API_KEY", Regex::new(r"\b(sk|pk|token|key|secret|bearer|auth)[-_]?[A-Za-z0-9]{16,}\b").unwrap()),
        ("URL_TOKEN", Regex::new(r"https?://[^\s]*[?&](token|key|secret|auth|code|access_token)=[^\s&]+").unwrap()),
        ("WALLET", Regex::new(r"\b0x[a-fA-F0-9]{40}\b|\b[13][a-km-zA-HJ-NP-Z1-9]{25,34}\b").unwrap()),
        ("IP_ADDR", Regex::new(r"\b\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}\b").unwrap()),
        ("NAME_PATTERN", Regex::new(r"(?i)\b(my name is|i'm|i am)\s+[A-Z][a-z]+\b").unwrap()),
        ("FILE_PATH", Regex::new(r"/[Uu]sers/[^/\s]+").unwrap()),
    ]
});

fn redact_content(text: &str) -> String {
    let mut result = text.to_string();
    for (label, pattern) in REDACTION_PATTERNS.iter() {
        result = pattern
            .replace_all(&result, format!("[{label}]").as_str())
            .into_owned();
    }
    result
}

fn sha256_hex(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("{:x}", hasher.finalize())
}

struct GatewayMetrics {
    event_dispatch_duration_seconds: HistogramVec,
    event_payload_bytes: HistogramVec,
    events_published_total: IntCounterVec,
    events_received_total: IntCounterVec,
    pipeline_stage_duration_seconds: HistogramVec,
    publish_failures_total: IntCounterVec,
    registry: Registry,
}

struct Pipeline {
    ai_store: PostgresAiStore,
    queue: RedisEventQueue,
    store: PostgresRawEventStore,
}

impl Pipeline {
    async fn publish(&self, envelope: EventEnvelope) -> Result<()> {
        let event_name = envelope.event_name.clone();
        let serialize_started = Instant::now();
        let payload = match serde_json::to_string(&envelope) {
            Ok(payload) => {
                observe_pipeline_stage(
                    &event_name,
                    "serialize",
                    "success",
                    serialize_started.elapsed().as_secs_f64(),
                );
                payload
            }
            Err(error) => {
                observe_pipeline_stage(
                    &event_name,
                    "serialize",
                    "failure",
                    serialize_started.elapsed().as_secs_f64(),
                );
                observe_publish_failure(&event_name, "serialize");
                return Err(error).context("failed to serialize queue event");
            }
        };
        observe_payload_size(&event_name, payload.len());

        let persist_started = Instant::now();
        let raw_event_id = self
            .store
            .insert_serialized(&envelope, &payload)
            .await
            .map_err(|error| {
                observe_pipeline_stage(
                    &event_name,
                    "persist",
                    "failure",
                    persist_started.elapsed().as_secs_f64(),
                );
                observe_publish_failure(&event_name, "persist");
                error
            })?;
        observe_pipeline_stage(
            &event_name,
            "persist",
            "success",
            persist_started.elapsed().as_secs_f64(),
        );

        let queue_started = Instant::now();
        let queue_payload = serde_json::to_string(&QueuedEventRef::new(raw_event_id, &envelope))
            .map_err(|error| {
                observe_pipeline_stage(
                    &event_name,
                    "queue",
                    "failure",
                    queue_started.elapsed().as_secs_f64(),
                );
                observe_publish_failure(&event_name, "queue");
                error
            })
            .context("failed to serialize queue event reference")?;
        self.queue
            .publish(envelope.stream_name(), &queue_payload)
            .await
            .map_err(|error| {
                observe_pipeline_stage(
                    &event_name,
                    "queue",
                    "failure",
                    queue_started.elapsed().as_secs_f64(),
                );
                observe_publish_failure(&event_name, "queue");
                error
            })?;
        observe_pipeline_stage(
            &event_name,
            "queue",
            "success",
            queue_started.elapsed().as_secs_f64(),
        );
        observe_published_event(&event_name);
        Ok(())
    }
}

struct Handler {
    pipeline: Arc<Pipeline>,
}

impl Handler {
    async fn dispatch(&self, envelope: EventEnvelope) {
        let event_name = envelope.event_name.clone();
        observe_received_event(&event_name);
        let started = Instant::now();
        if let Err(error) = self.pipeline.publish(envelope).await {
            observe_dispatch(&event_name, "failure", started.elapsed().as_secs_f64());
            error!(?error, "failed to persist and enqueue event");
        } else {
            observe_dispatch(&event_name, "success", started.elapsed().as_secs_f64());
        }
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: DiscordContext, ready: Ready) {
        info!(bot_user = %ready.user.name, "gateway connected");
    }

    async fn guild_create(&self, _: DiscordContext, guild: Guild, is_new: Option<bool>) {
        let member_count = i64::try_from(guild.member_count).unwrap_or(i64::MAX);
        let envelope = EventEnvelope::new(
            guild.id.to_string(),
            None,
            Some(guild.owner_id.to_string()),
            Utc::now(),
            EventPayload::GuildAvailable(GuildAvailablePayload {
                guild_id: guild.id.to_string(),
                name: guild.name.clone(),
                member_count,
                owner_id: guild.owner_id.to_string(),
                is_new: is_new.unwrap_or(false),
            }),
        );

        self.dispatch(envelope).await;
    }

    async fn guild_delete(
        &self,
        _: DiscordContext,
        incomplete: UnavailableGuild,
        _: Option<Guild>,
    ) {
        let envelope = EventEnvelope::new(
            incomplete.id.to_string(),
            None,
            None,
            Utc::now(),
            EventPayload::GuildRemoved(GuildRemovedPayload {
                guild_id: incomplete.id.to_string(),
                is_unavailable: incomplete.unavailable,
            }),
        );

        self.dispatch(envelope).await;
    }

    async fn guild_member_addition(&self, _: DiscordContext, member: Member) {
        let joined_at = member.joined_at.as_ref().map(timestamp_to_chrono);
        let envelope = EventEnvelope::new(
            member.guild_id.to_string(),
            None,
            Some(member.user.id.to_string()),
            joined_at.unwrap_or_else(Utc::now),
            EventPayload::MemberJoined(MemberJoinedPayload {
                member_id: member.user.id.to_string(),
                joined_at,
                is_pending: member.pending,
                role_ids: member
                    .roles
                    .iter()
                    .map(|role_id| role_id.to_string())
                    .collect(),
            }),
        );

        self.dispatch(envelope).await;
    }

    async fn guild_member_removal(
        &self,
        _: DiscordContext,
        guild_id: GuildId,
        user: User,
        member_data_if_available: Option<Member>,
    ) {
        let envelope = EventEnvelope::new(
            guild_id.to_string(),
            None,
            Some(user.id.to_string()),
            Utc::now(),
            EventPayload::MemberLeft(MemberLeftPayload {
                member_id: user.id.to_string(),
                had_member_record: member_data_if_available.is_some(),
            }),
        );

        self.dispatch(envelope).await;
    }

    async fn guild_member_update(
        &self,
        _: DiscordContext,
        old_if_available: Option<Member>,
        new: Option<Member>,
        event: GuildMemberUpdateEvent,
    ) {
        let previous_roles = old_if_available
            .as_ref()
            .map(|member| {
                member
                    .roles
                    .iter()
                    .map(|role_id| role_id.to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let current_role_ids = new
            .as_ref()
            .map(|member| {
                member
                    .roles
                    .iter()
                    .map(|role_id| role_id.to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| {
                event
                    .roles
                    .iter()
                    .map(|role_id| role_id.to_string())
                    .collect()
            });

        let added_role_ids = current_role_ids
            .iter()
            .filter(|role_id| !previous_roles.contains(role_id))
            .cloned()
            .collect::<Vec<_>>();
        let removed_role_ids = previous_roles
            .iter()
            .filter(|role_id| !current_role_ids.contains(role_id))
            .cloned()
            .collect::<Vec<_>>();

        if added_role_ids.is_empty() && removed_role_ids.is_empty() {
            return;
        }

        let envelope = EventEnvelope::new(
            event.guild_id.to_string(),
            None,
            Some(event.user.id.to_string()),
            Utc::now(),
            EventPayload::MemberRolesUpdated(MemberRolesUpdatedPayload {
                member_id: event.user.id.to_string(),
                added_role_ids,
                removed_role_ids,
                current_role_ids,
                is_pending: event.pending,
            }),
        );

        self.dispatch(envelope).await;
    }

    async fn message(&self, _: DiscordContext, message: Message) {
        let Some(guild_id) = message.guild_id else {
            return;
        };

        let guild_id_str = guild_id.to_string();
        let channel_id_str = message.channel_id.to_string();
        let message_id_str = message.id.to_string();
        let author_id_str = message.author.id.to_string();
        let is_bot = message.author.bot;
        let content = message.content.clone();
        let occurred_at = timestamp_to_chrono(&message.timestamp);

        let envelope = EventEnvelope::new(
            guild_id_str.clone(),
            Some(channel_id_str.clone()),
            Some(author_id_str.clone()),
            occurred_at,
            EventPayload::MessageCreated(MessageCreatedPayload {
                message_id: message_id_str.clone(),
                author_id: author_id_str.clone(),
                is_bot,
                is_reply: message.referenced_message.is_some()
                    || message.message_reference.is_some(),
                attachment_count: i32::try_from(message.attachments.len()).unwrap_or(i32::MAX),
                content_length: i32::try_from(content.chars().count()).unwrap_or(i32::MAX),
            }),
        );

        self.dispatch(envelope).await;

        if !is_bot {
            self.capture_ai_observation(
                &guild_id_str,
                &channel_id_str,
                &message_id_str,
                &author_id_str,
                occurred_at,
                &content,
            )
            .await;
        }
    }

    async fn reaction_add(&self, _: DiscordContext, reaction: Reaction) {
        let Some(guild_id) = reaction.guild_id else {
            return;
        };
        let Some(user_id) = reaction.user_id else {
            return;
        };

        let envelope = EventEnvelope::new(
            guild_id.to_string(),
            Some(reaction.channel_id.to_string()),
            Some(user_id.to_string()),
            Utc::now(),
            EventPayload::ReactionAdded(ReactionAddedPayload {
                message_id: reaction.message_id.to_string(),
                user_id: user_id.to_string(),
                emoji: reaction.emoji.to_string(),
            }),
        );

        self.dispatch(envelope).await;
    }

    async fn voice_state_update(
        &self,
        _: DiscordContext,
        old: Option<VoiceState>,
        new: VoiceState,
    ) {
        let Some(guild_id) = new.guild_id else {
            return;
        };

        let envelope = EventEnvelope::new(
            guild_id.to_string(),
            new.channel_id.map(|channel_id| channel_id.to_string()),
            Some(new.user_id.to_string()),
            Utc::now(),
            EventPayload::VoiceStateUpdated(VoiceStateUpdatedPayload {
                member_id: new.user_id.to_string(),
                old_channel_id: old.and_then(|state| state.channel_id.map(|id| id.to_string())),
                new_channel_id: new.channel_id.map(|id| id.to_string()),
            }),
        );

        self.dispatch(envelope).await;
    }
}

impl Handler {
    async fn capture_ai_observation(
        &self,
        guild_id: &str,
        channel_id: &str,
        message_id: &str,
        author_id: &str,
        occurred_at: DateTime<Utc>,
        content: &str,
    ) {
        let enabled = match self
            .pipeline
            .ai_store
            .is_content_capture_enabled(guild_id, channel_id)
            .await
        {
            Ok(v) => v,
            Err(err) => {
                error!(?err, "failed to check ai content capture");
                return;
            }
        };

        if !enabled {
            return;
        }

        let (content_redacted, content_fingerprint, redaction_status) = if content.is_empty() {
            (None, None, "not_captured")
        } else {
            (
                Some(redact_content(content)),
                Some(sha256_hex(content)),
                "redacted",
            )
        };

        let obs = NewAiMessageObservation {
            guild_id: guild_id.to_string(),
            channel_id: channel_id.to_string(),
            message_id: message_id.to_string(),
            author_id: author_id.to_string(),
            occurred_at,
            content_redacted,
            content_fingerprint,
            redaction_status,
            redaction_version: Some(REDACTION_VERSION.to_string()),
        };

        let obs_id = match self.pipeline.ai_store.insert_observation(&obs).await {
            Ok(id) => id,
            Err(err) => {
                error!(?err, "failed to insert ai observation");
                return;
            }
        };

        let job = AiClassifyJob::new(obs_id, guild_id, channel_id);
        match serde_json::to_string(&job) {
            Ok(payload) => {
                if let Err(err) = self
                    .pipeline
                    .queue
                    .publish(AI_CLASSIFY_STREAM, &payload)
                    .await
                {
                    error!(?err, "failed to publish ai classify job");
                }
            }
            Err(err) => error!(?err, "failed to serialize ai classify job"),
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let settings = Settings::from_env()?;
    init_tracing(&settings.rust_log);

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&settings.database_url)
        .await
        .context("failed to connect to postgres")?;

    let store = PostgresRawEventStore::new(pool.clone());
    store.ensure_schema().await?;

    let ai_store = PostgresAiStore::new(pool);
    ai_store.ensure_schema().await?;

    let queue = RedisEventQueue::new(&settings.redis_url)?;
    let handler = Handler {
        pipeline: Arc::new(Pipeline { ai_store, queue, store }),
    };
    let metrics_addr: SocketAddr = settings
        .gateway_metrics_bind_addr
        .parse()
        .context("invalid GATEWAY_METRICS_BIND_ADDR")?;
    task::spawn(async move {
        if let Err(error) = run_metrics_server(metrics_addr).await {
            error!(?error, "gateway metrics server exited");
        }
    });

    let mut intents = GatewayIntents::GUILDS
        | GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::GUILD_MESSAGE_REACTIONS
        | GatewayIntents::GUILD_VOICE_STATES;

    if settings.discord_enable_guild_members_intent {
        intents |= GatewayIntents::GUILD_MEMBERS;
    }
    if settings.discord_enable_message_content_intent {
        intents |= GatewayIntents::MESSAGE_CONTENT;
    }

    let mut client = Client::builder(&settings.discord_token, intents)
        .event_handler(handler)
        .application_id(settings.discord_application_id.into())
        .await
        .context("failed to build serenity client")?;

    client.start_autosharded().await.context("gateway crashed")
}

fn init_tracing(rust_log: &str) {
    let filter = EnvFilter::try_new(rust_log).unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

fn gateway_metrics() -> &'static GatewayMetrics {
    METRICS.get_or_init(|| {
        let registry = Registry::new();
        let event_dispatch_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "gateway_event_dispatch_duration_seconds",
                "Gateway event dispatch duration in seconds",
            ),
            &["event", "status"],
        )
        .expect("failed to create gateway dispatch histogram");
        let event_payload_bytes = HistogramVec::new(
            HistogramOpts::new(
                "gateway_event_payload_bytes",
                "Serialized gateway event payload size in bytes",
            ),
            &["event"],
        )
        .expect("failed to create gateway payload size histogram");
        let events_published_total = IntCounterVec::new(
            prometheus::Opts::new(
                "gateway_events_published_total",
                "Gateway events successfully persisted and queued",
            ),
            &["event"],
        )
        .expect("failed to create gateway published counter");
        let events_received_total = IntCounterVec::new(
            prometheus::Opts::new(
                "gateway_events_received_total",
                "Gateway events received by the handler",
            ),
            &["event"],
        )
        .expect("failed to create gateway received counter");
        let pipeline_stage_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "gateway_pipeline_stage_duration_seconds",
                "Gateway pipeline stage duration in seconds",
            ),
            &["event", "stage", "status"],
        )
        .expect("failed to create gateway pipeline stage histogram");
        let publish_failures_total = IntCounterVec::new(
            prometheus::Opts::new(
                "gateway_publish_failures_total",
                "Gateway publish failures grouped by event and stage",
            ),
            &["event", "stage"],
        )
        .expect("failed to create gateway failure counter");

        registry
            .register(Box::new(event_dispatch_duration_seconds.clone()))
            .expect("failed to register gateway dispatch histogram");
        registry
            .register(Box::new(event_payload_bytes.clone()))
            .expect("failed to register gateway payload histogram");
        registry
            .register(Box::new(events_published_total.clone()))
            .expect("failed to register gateway published counter");
        registry
            .register(Box::new(events_received_total.clone()))
            .expect("failed to register gateway received counter");
        registry
            .register(Box::new(pipeline_stage_duration_seconds.clone()))
            .expect("failed to register gateway pipeline stage histogram");
        registry
            .register(Box::new(publish_failures_total.clone()))
            .expect("failed to register gateway failure counter");

        GatewayMetrics {
            event_dispatch_duration_seconds,
            event_payload_bytes,
            events_published_total,
            events_received_total,
            pipeline_stage_duration_seconds,
            publish_failures_total,
            registry,
        }
    })
}

async fn run_metrics_server(addr: SocketAddr) -> Result<()> {
    let app = Router::new()
        .route("/metrics", get(metrics))
        .route("/readyz", get(|| async { "ok" }))
        .with_state(gateway_metrics());

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("failed to bind gateway metrics listener to {addr}"))?;
    info!(address = %addr, "gateway metrics listening");
    axum::serve(listener, app)
        .await
        .context("gateway metrics server crashed")
}

async fn metrics(State(metrics): State<&'static GatewayMetrics>) -> impl IntoResponse {
    let mut encoded = Vec::new();
    let encoder = TextEncoder::new();
    let metric_families = metrics.registry.gather();
    match encoder.encode(&metric_families, &mut encoded) {
        Ok(_) => (
            [(
                axum::http::header::CONTENT_TYPE,
                encoder.format_type().to_string(),
            )],
            encoded,
        )
            .into_response(),
        Err(error) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to encode metrics: {error}"),
        )
            .into_response(),
    }
}

fn observe_dispatch(event_name: &str, status: &str, duration_seconds: f64) {
    gateway_metrics()
        .event_dispatch_duration_seconds
        .with_label_values(&[event_name, status])
        .observe(duration_seconds);
}

fn observe_payload_size(event_name: &str, payload_bytes: usize) {
    gateway_metrics()
        .event_payload_bytes
        .with_label_values(&[event_name])
        .observe(payload_bytes as f64);
}

fn observe_received_event(event_name: &str) {
    gateway_metrics()
        .events_received_total
        .with_label_values(&[event_name])
        .inc();
}

fn observe_published_event(event_name: &str) {
    gateway_metrics()
        .events_published_total
        .with_label_values(&[event_name])
        .inc();
}

fn observe_pipeline_stage(event_name: &str, stage: &str, status: &str, duration_seconds: f64) {
    gateway_metrics()
        .pipeline_stage_duration_seconds
        .with_label_values(&[event_name, stage, status])
        .observe(duration_seconds);
}

fn observe_publish_failure(event_name: &str, stage: &str) {
    gateway_metrics()
        .publish_failures_total
        .with_label_values(&[event_name, stage])
        .inc();
}

fn timestamp_to_chrono(timestamp: &serenity::model::Timestamp) -> DateTime<Utc> {
    DateTime::from_timestamp(timestamp.unix_timestamp(), 0).unwrap_or_else(Utc::now)
}
