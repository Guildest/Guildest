use std::{
    collections::HashMap,
    convert::Infallible,
    net::SocketAddr,
    sync::{Arc, OnceLock},
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    response::{
        IntoResponse, Redirect, Response,
        sse::{Event, KeepAlive, Sse},
    },
    routing::{get, post},
};
use chrono::{Days, Utc};
use common::{
    ai::{AiStore, LivePulseResponse, PostgresAiStore, UpdateAiGuildSettings},
    config::Settings,
    jobs::{BACKFILL_STREAM, BackfillJob},
    queue::{
        EventQueue, PUBLIC_STATS_MEMBERS_KEY, PUBLIC_STATS_MESSAGES_KEY, PUBLIC_STATS_SERVERS_KEY,
        PUBLIC_STATS_UPDATES_CHANNEL, RedisEventQueue,
    },
    store::{PostgresRawEventStore, RawEventStore},
};
use futures_util::{Stream, StreamExt, stream};
use prometheus::{Encoder, HistogramOpts, HistogramVec, IntCounterVec, Registry, TextEncoder};
use reqwest::Client;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sqlx::{FromRow, PgPool, postgres::PgPoolOptions};
use tokio::sync::watch;
use tower_http::cors::{Any, CorsLayer};
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

const ADMINISTRATOR_PERMISSION: u64 = 0x8;
const DEFAULT_DISCORD_BOT_PERMISSIONS: &str = "274878024896";
const DISCORD_AUTHORIZE_URL: &str = "https://discord.com/oauth2/authorize";
const DISCORD_TOKEN_URL: &str = "https://discord.com/api/v10/oauth2/token";
const DISCORD_USERS_ME_URL: &str = "https://discord.com/api/v10/users/@me";
const DISCORD_USERS_ME_GUILDS_URL: &str = "https://discord.com/api/v10/users/@me/guilds";
const SESSION_COOKIE_NAME: &str = "guildest_session";
const SESSION_TTL_SECONDS: i64 = 60 * 60 * 24 * 30;
const DASHBOARD_CACHE_TTL_SECONDS: u64 = 60;
const DEAD_LETTER_ACTION_LOCK_TTL_SECONDS: u64 = 30;
const RESEND_EMAILS_URL: &str = "https://api.resend.com/emails";

#[derive(Clone)]
struct AppState {
    ai_store: PostgresAiStore,
    http_client: Client,
    metrics: &'static ApiMetrics,
    pool: PgPool,
    public_stats_cache: watch::Sender<PublicStatsResponse>,
    queue: RedisEventQueue,
    settings: Settings,
}

struct ApiMetrics {
    registry: Registry,
    request_duration_seconds: HistogramVec,
    requests_total: IntCounterVec,
    response_size_bytes: HistogramVec,
    sql_query_duration_seconds: HistogramVec,
}

#[derive(Debug, Clone)]
struct MemberDirectoryEntry {
    global_name: Option<String>,
    nickname: Option<String>,
    username: String,
}

#[derive(Debug, Serialize)]
struct AccessibleGuild {
    guild_id: String,
    guild_name: String,
    is_owner: bool,
    member_count: i64,
}

#[derive(Debug, Serialize)]
struct BackfillRequestResponse {
    job_id: String,
    status: &'static str,
}

#[derive(Debug, Serialize)]
struct DashboardGuildSummaryResponse {
    backfill_status: Option<String>,
    daily: Vec<MessageSummaryDay>,
    days_requested: i32,
    guild_id: String,
    total_messages: i64,
}

#[derive(Debug, Serialize)]
struct DashboardActivationFunnelResponse {
    days_requested: i32,
    guild_id: String,
    steps: Vec<DashboardActivationFunnelStep>,
}

#[derive(Debug, Serialize, Deserialize)]
struct DashboardGuildHealthSummaryResponse {
    dau: i64,
    days_requested: i32,
    guild_id: String,
    join_leave_ratio: Option<f64>,
    joined_members: i64,
    left_members: i64,
    onboarding_completion_rate: Option<f64>,
    onboarded_members: i64,
    wau: i64,
    mau: i64,
}

#[derive(Debug, Serialize, Deserialize)]
struct DashboardRetentionCohortsResponse {
    cohorts: Vec<DashboardRetentionCohort>,
    d30_retention_rate: Option<f64>,
    d7_retention_rate: Option<f64>,
    days_requested: i32,
    guild_id: String,
}

#[derive(Debug, Serialize)]
struct DashboardGuildUsersResponse {
    avg_messages_per_active_user: f64,
    days_requested: i32,
    guild_id: String,
    total_active_users: i64,
    total_voice_seconds: i64,
    users: Vec<DashboardTopUser>,
}

#[derive(Debug, Serialize, Deserialize)]
struct DashboardGuildHotspotsResponse {
    active_channels: i64,
    days_requested: i32,
    guild_id: String,
    hourly_activity: Vec<HourlyActivityPoint>,
    peak_hour_utc: Option<String>,
    retention_channels: Vec<DashboardRetentionChannel>,
    top_channels: Vec<DashboardTopChannel>,
    total_messages: i64,
}

#[derive(Debug, Serialize, Deserialize)]
struct DashboardPipelineHealthResponse {
    guild_id: String,
    healthy_streams: i64,
    max_oldest_ready_age_seconds: i64,
    max_scheduled_retry_overdue_seconds: i64,
    overall_status: String,
    streams: Vec<DashboardPipelineStreamHealth>,
    total_dead_letter_messages: i64,
    total_pending_messages: i64,
    total_ready_messages: i64,
    total_scheduled_retry_messages: i64,
    total_streams: i64,
}

#[derive(Debug, Serialize, Deserialize)]
struct DashboardPipelineIncidentsResponse {
    guild_id: String,
    incidents: Vec<DashboardPipelineIncident>,
    total_dead_letter_messages: i64,
}

#[derive(Debug, Serialize, Deserialize)]
struct DashboardPipelineReplayHistoryResponse {
    guild_id: String,
    replays: Vec<DashboardPipelineReplay>,
}

#[derive(Debug, Serialize, Deserialize)]
struct DashboardPipelineDiscardHistoryResponse {
    discards: Vec<DashboardPipelineDiscard>,
    guild_id: String,
}

#[derive(Debug, Deserialize)]
struct ReplayPipelineIncidentRequest {
    dead_letter_entry_id: String,
    operator_reason: Option<String>,
    source_stream: String,
}

#[derive(Debug, Deserialize)]
struct DiscardPipelineIncidentRequest {
    dead_letter_entry_id: String,
    operator_reason: Option<String>,
    source_stream: String,
}

#[derive(Debug, Serialize)]
struct ReplayPipelineIncidentResponse {
    dead_letter_entry_id: String,
    delivery_id: String,
    source_stream: String,
    status: &'static str,
}

#[derive(Debug, Serialize)]
struct DiscardPipelineIncidentResponse {
    dead_letter_entry_id: String,
    delivery_id: String,
    source_stream: String,
    status: &'static str,
}

#[derive(Debug, Serialize)]
struct DashboardMeResponse {
    accessible_guilds: Vec<AccessibleGuild>,
    user: DashboardUser,
}

#[derive(Debug, Serialize)]
struct DashboardUser {
    display_name: String,
    discord_user_id: String,
    username: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct DashboardTopChannel {
    avg_response_seconds: Option<f64>,
    channel_id: String,
    health_score: f64,
    label: String,
    message_count: i64,
    messages_per_sender: Option<f64>,
    previous_period_messages: i64,
    replies: i64,
    trend_percent_change: Option<f64>,
    unique_senders: i64,
}

#[derive(Debug, Serialize, Deserialize)]
struct DashboardRetentionChannel {
    channel_id: String,
    d30_retained_members: i64,
    d7_retained_members: i64,
    label: String,
    retention_score: i64,
}

#[derive(Debug, Serialize, Deserialize)]
struct DashboardPipelineStreamHealth {
    dead_letter_messages: i64,
    label: String,
    oldest_dead_letter_age_seconds: i64,
    oldest_ready_age_seconds: i64,
    pending_messages: i64,
    ready_messages: i64,
    scheduled_retry_messages: i64,
    scheduled_retry_overdue_seconds: i64,
    status: String,
    stream: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct DashboardPipelineIncident {
    attempts: i64,
    dead_letter_entry_id: String,
    delivery_id: String,
    error: String,
    failed_at: String,
    payload_preview: String,
    retry_key: String,
    source_stream: String,
    source_stream_label: String,
    age_seconds: i64,
}

#[derive(Debug, Serialize, Deserialize)]
struct DashboardPipelineReplay {
    attempts: i64,
    delivery_id: String,
    operator_reason: Option<String>,
    replayed_at: String,
    replayed_by_label: String,
    replayed_by_user_id: Option<String>,
    source_stream: String,
    source_stream_label: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct DashboardPipelineDiscard {
    attempts: i64,
    delivery_id: String,
    discarded_at: String,
    discarded_by_label: String,
    discarded_by_user_id: Option<String>,
    operator_reason: Option<String>,
    source_stream: String,
    source_stream_label: String,
}

#[derive(Debug, Deserialize)]
struct DeadLetterDeliveryPayload {
    attempts: i64,
    delivery_id: String,
    error: String,
    failed_at: chrono::DateTime<Utc>,
    payload: String,
    retry_key: String,
    source_stream: String,
}

#[derive(Debug, Serialize)]
struct DashboardTopUser {
    active_days: i64,
    discord_user_id: String,
    label: String,
    secondary_label: Option<String>,
    messages_sent: i64,
    reactions_added: i64,
    voice_seconds: i64,
}

#[derive(Debug, Serialize)]
struct DashboardActivationFunnelStep {
    count: i64,
    key: &'static str,
    label: &'static str,
}

#[derive(Debug, Serialize, Deserialize)]
struct DashboardRetentionCohort {
    cohort_age_days: i32,
    cohort_date: String,
    d30_retained: i64,
    d30_retention_rate: Option<f64>,
    d7_retained: i64,
    d7_retention_rate: Option<f64>,
    joined_count: i64,
}

#[derive(Debug, Serialize, Deserialize)]
struct HourlyActivityPoint {
    hour_label: String,
    hour_of_day: i32,
    message_count: i64,
}

#[derive(Debug, Serialize)]
struct MessageSummaryDay {
    date: String,
    message_count: i64,
}

#[derive(Debug, Serialize)]
struct PublicMessageHeatmapResponse {
    days: Vec<MessageSummaryDay>,
    days_requested: i32,
    scope: &'static str,
    time_zone: &'static str,
    total_messages: i64,
    window_end_utc: String,
    window_start_utc: String,
}

#[derive(Debug, Serialize)]
struct DashboardMessageHeatmapResponse {
    days: Vec<MessageSummaryDay>,
    days_requested: i32,
    guild_id: String,
    time_zone: &'static str,
    total_messages: i64,
    window_end_utc: String,
    window_start_utc: String,
}

#[derive(Debug, Serialize)]
struct PublicLinksResponse {
    invite_url: String,
    install_url: String,
    login_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct PublicStatsResponse {
    members: i64,
    messages_tracked: i64,
    servers: i64,
}

#[derive(Debug, Deserialize)]
struct WaitlistSubmitRequest {
    source: Option<String>,
    use_case: Option<String>,
    notes: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TeamsLeadSubmitRequest {
    name: Option<String>,
    email: String,
    company: Option<String>,
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SubscribeSubmitRequest {
    email: String,
}

#[derive(Debug, Serialize)]
struct ResendEmailRequest {
    from: String,
    to: Vec<String>,
    subject: String,
    text: String,
    html: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    reply_to: Option<String>,
}

#[derive(Debug, Serialize)]
struct WaitlistSubmitResponse {
    ok: bool,
    id: Uuid,
}

#[derive(Debug, Deserialize)]
struct BackfillQuery {
    days: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct DaysQuery {
    days: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct DiscordCurrentUser {
    avatar: Option<String>,
    global_name: Option<String>,
    id: String,
    username: String,
}

#[derive(Debug, Deserialize)]
struct DiscordCurrentUserGuild {
    icon: Option<String>,
    id: String,
    name: String,
    owner: bool,
    permissions: String,
}

#[derive(Debug, Deserialize)]
struct DiscordGuildChannel {
    id: String,
    name: Option<String>,
    #[serde(rename = "type")]
    kind: i32,
}

#[derive(Debug, Deserialize)]
struct DiscordGuildMember {
    nick: Option<String>,
    user: DiscordMemberUser,
}

#[derive(Debug, Deserialize)]
struct DiscordMemberUser {
    global_name: Option<String>,
    id: String,
    username: String,
}

#[derive(Debug, Deserialize)]
struct DiscordTokenResponse {
    access_token: String,
    token_type: String,
}

#[derive(Debug, Deserialize)]
struct OAuthCallbackQuery {
    code: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
    guild_id: Option<String>,
    permissions: Option<String>,
    state: Option<String>,
}

#[derive(Debug, Serialize)]
struct SessionPersistResult {
    accessible_guilds: i64,
    session_id: Uuid,
}

#[derive(Debug, FromRow)]
struct PublicStatsRow {
    members: i64,
    messages_tracked: i64,
    servers: i64,
}

struct PublicStatsEventStream {
    initial_event: Option<Event>,
    updates: watch::Receiver<PublicStatsResponse>,
}

#[derive(Debug, FromRow)]
struct DashboardUserRow {
    discord_user_id: String,
    global_name: Option<String>,
    username: String,
}

#[derive(Debug, FromRow)]
struct AccessibleGuildRow {
    guild_id: String,
    guild_name: String,
    is_owner: bool,
    member_count: i64,
}

#[derive(Debug, FromRow)]
struct MessageSummaryDayRow {
    day: chrono::NaiveDate,
    message_count: i64,
}

#[derive(Debug, FromRow)]
struct TopChannelRow {
    avg_response_seconds: Option<f64>,
    channel_id: String,
    message_count: i64,
    previous_period_messages: i64,
    replies: i64,
    unique_senders: i64,
}

#[derive(Debug, FromRow)]
struct RetentionChannelRow {
    channel_id: String,
    d30_retained_members: i64,
    d7_retained_members: i64,
}

#[derive(Debug, FromRow)]
struct ChannelInventoryRow {
    channel_id: String,
    channel_name: String,
}

#[derive(Debug, FromRow)]
struct MemberInventoryRow {
    global_name: Option<String>,
    member_id: String,
    nickname: Option<String>,
    username: String,
}

#[derive(Debug, FromRow)]
struct TopUserRow {
    active_days: i64,
    discord_user_id: String,
    messages_sent: i64,
    reactions_added: i64,
    voice_seconds: i64,
}

#[derive(Debug, FromRow)]
struct HourlyActivityRow {
    hour_of_day: i32,
    message_count: i64,
}

#[derive(Debug, FromRow)]
struct UserSummaryRow {
    total_active_users: i64,
    total_messages: i64,
    total_voice_seconds: i64,
}

#[derive(Debug, FromRow)]
struct PipelineReplayAuditRow {
    attempts: i64,
    delivery_id: String,
    operator_reason: Option<String>,
    replayed_at: chrono::DateTime<Utc>,
    replayed_by_display_name: String,
    replayed_by_user_id: Option<String>,
    source_stream: String,
}

#[derive(Debug, FromRow)]
struct PipelineDiscardAuditRow {
    attempts: i64,
    delivery_id: String,
    discarded_at: chrono::DateTime<Utc>,
    discarded_by_display_name: String,
    discarded_by_user_id: Option<String>,
    operator_reason: Option<String>,
    source_stream: String,
}

#[derive(Debug, FromRow)]
struct ChannelSummaryRow {
    active_channels: i64,
    total_messages: i64,
}

#[derive(Debug, FromRow)]
struct ActivationFunnelRow {
    first_message_count: i64,
    first_reaction_count: i64,
    first_voice_count: i64,
    got_role_count: i64,
    joined_count: i64,
    returned_next_week_count: i64,
}

#[derive(Debug, FromRow)]
struct RetentionCohortRow {
    cohort_age_days: i32,
    cohort_date: chrono::NaiveDate,
    d30_retained: i64,
    d7_retained: i64,
    joined_count: i64,
}

#[derive(Debug, FromRow)]
struct GuildHealthSummaryRow {
    dau: i64,
    joined_members: i64,
    left_members: i64,
    mau: i64,
    onboarded_members: i64,
    wau: i64,
}

#[derive(Debug, FromRow)]
struct BackfillStatusRow {
    status: String,
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
    PostgresRawEventStore::new(pool.clone())
        .ensure_schema()
        .await?;
    let ai_store = PostgresAiStore::new(pool.clone());
    ai_store.ensure_schema().await?;
    ensure_public_schema(&pool).await?;
    let queue = RedisEventQueue::new(&settings.redis_url)?;
    let initial_public_stats = load_public_stats_snapshot(&pool, &queue)
        .await
        .context("failed to initialize public stats cache")?;
    let (public_stats_cache, _) = watch::channel(initial_public_stats);

    let state = Arc::new(AppState {
        ai_store,
        http_client: Client::new(),
        metrics: api_metrics(),
        pool,
        public_stats_cache,
        queue,
        settings: settings.clone(),
    });
    tokio::spawn(run_public_stats_cache_updater(state.clone()));
    let cors = build_cors(&settings.public_api_allowed_origin);
    let app = Router::new()
        .route("/health", get(health))
        .route("/metrics", get(metrics))
        .route("/v1/public/stats", get(public_stats))
        .route("/v1/public/stats/stream", get(public_stats_stream))
        .route("/v1/public/messages/heatmap", get(public_message_heatmap))
        .route("/v1/public/links", get(public_links))
        .route("/v1/public/waitlist", post(public_waitlist_submit))
        .route("/v1/public/subscribe", post(public_subscribe_submit))
        .route("/v1/public/teams-lead", post(public_teams_lead_submit))
        .route("/v1/public/admin/waitlist", get(public_waitlist_export))
        .route("/v1/public/oauth/start/login", get(start_login_oauth))
        .route("/v1/public/oauth/start/invite", get(start_invite_oauth))
        .route("/v1/public/install/start", get(start_install))
        .route("/v1/public/oauth/callback", get(oauth_callback))
        .route("/v1/dashboard/me", get(dashboard_me))
        .route(
            "/v1/dashboard/guilds/{guild_id}/messages/summary",
            get(dashboard_guild_message_summary),
        )
        .route(
            "/v1/dashboard/guilds/{guild_id}/messages/heatmap",
            get(dashboard_guild_message_heatmap),
        )
        .route(
            "/v1/dashboard/guilds/{guild_id}/activation/funnel",
            get(dashboard_activation_funnel),
        )
        .route(
            "/v1/dashboard/guilds/{guild_id}/summary/health",
            get(dashboard_guild_health_summary),
        )
        .route(
            "/v1/dashboard/guilds/{guild_id}/retention/cohorts",
            get(dashboard_retention_cohorts),
        )
        .route(
            "/v1/dashboard/guilds/{guild_id}/users/summary",
            get(dashboard_guild_users_summary),
        )
        .route(
            "/v1/dashboard/guilds/{guild_id}/channels/hotspots",
            get(dashboard_guild_hotspots),
        )
        .route(
            "/v1/dashboard/guilds/{guild_id}/ops/pipeline",
            get(dashboard_pipeline_health),
        )
        .route(
            "/v1/dashboard/guilds/{guild_id}/ops/incidents",
            get(dashboard_pipeline_incidents),
        )
        .route(
            "/v1/dashboard/guilds/{guild_id}/ops/replays",
            get(dashboard_pipeline_replay_history),
        )
        .route(
            "/v1/dashboard/guilds/{guild_id}/ops/discards",
            get(dashboard_pipeline_discard_history),
        )
        .route(
            "/v1/dashboard/guilds/{guild_id}/ops/incidents/replay",
            post(replay_dashboard_pipeline_incident),
        )
        .route(
            "/v1/dashboard/guilds/{guild_id}/ops/incidents/discard",
            post(discard_dashboard_pipeline_incident),
        )
        .route(
            "/v1/dashboard/guilds/{guild_id}/backfill",
            post(request_guild_backfill),
        )
        .route(
            "/v1/dashboard/guilds/{guild_id}/ai/settings",
            get(dashboard_ai_settings).put(dashboard_ai_settings_update),
        )
        .route(
            "/v1/dashboard/guilds/{guild_id}/ai/live-pulse",
            get(dashboard_ai_live_pulse),
        )
        .with_state(state)
        .layer(cors);

    let addr: SocketAddr = settings
        .api_bind_addr
        .parse()
        .context("invalid API_BIND_ADDR")?;
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .context("failed to bind api listener")?;

    info!(address = %addr, "api listening");
    axum::serve(listener, app)
        .await
        .context("api server crashed")
}

async fn health() -> &'static str {
    "ok"
}

async fn metrics(State(state): State<Arc<AppState>>) -> Response {
    let encoder = TextEncoder::new();
    let metric_families = state.metrics.registry.gather();
    let mut body = Vec::new();

    match encoder.encode(&metric_families, &mut body) {
        Ok(()) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, encoder.format_type().to_string())],
            body,
        )
            .into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to encode metrics: {error}"),
        )
            .into_response(),
    }
}

async fn public_links(
    State(state): State<Arc<AppState>>,
) -> Result<Json<PublicLinksResponse>, StatusCode> {
    Ok(Json(PublicLinksResponse {
        invite_url: public_url(&state.settings, "/v1/public/oauth/start/invite"),
        install_url: public_url(&state.settings, "/v1/public/install/start"),
        login_url: public_url(&state.settings, "/v1/public/oauth/start/login"),
    }))
}

async fn public_waitlist_submit(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<WaitlistSubmitRequest>,
) -> Result<Json<WaitlistSubmitResponse>, StatusCode> {
    let session_id = read_session_id(&headers).ok_or(StatusCode::UNAUTHORIZED)?;
    let user = fetch_dashboard_user(&state.pool, session_id)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    let id = Uuid::new_v4();
    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let trim_opt = |s: Option<String>| s.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let source = trim_opt(request.source);
    let use_case = trim_opt(request.use_case);
    let notes = trim_opt(request.notes);

    sqlx::query(
        r#"
        INSERT INTO waitlist_entries (
            id, kind, discord_user_id, discord_username, discord_display_name,
            source, use_case, notes, user_agent
        )
        VALUES ($1, 'waitlist', $2, $3, $4, $5, $6, $7, $8)
        ON CONFLICT (discord_user_id)
            WHERE discord_user_id IS NOT NULL AND kind = 'waitlist'
        DO UPDATE SET
            discord_username = EXCLUDED.discord_username,
            discord_display_name = EXCLUDED.discord_display_name,
            source = COALESCE(EXCLUDED.source, waitlist_entries.source),
            use_case = COALESCE(EXCLUDED.use_case, waitlist_entries.use_case),
            notes = COALESCE(EXCLUDED.notes, waitlist_entries.notes),
            user_agent = COALESCE(EXCLUDED.user_agent, waitlist_entries.user_agent)
        "#,
    )
    .bind(id)
    .bind(&user.discord_user_id)
    .bind(&user.username)
    .bind(&user.display_name)
    .bind(&source)
    .bind(&use_case)
    .bind(&notes)
    .bind(&user_agent)
    .execute(&state.pool)
    .await
    .map_err(|err| {
        tracing::error!(?err, "failed to insert waitlist entry");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    send_form_email(
        &state,
        "New Guildest waitlist signup",
        "New waitlist signup",
        vec![
            ("Discord display name", Some(user.display_name.as_str())),
            ("Discord username", Some(user.username.as_str())),
            ("Discord user ID", Some(user.discord_user_id.as_str())),
            ("Source", source.as_deref()),
            ("Use case", use_case.as_deref()),
            ("Notes", notes.as_deref()),
            ("User agent", user_agent.as_deref()),
        ],
        None,
    )
    .await
    .map_err(|err| {
        tracing::error!(?err, "failed to send waitlist email");
        StatusCode::BAD_GATEWAY
    })?;

    Ok(Json(WaitlistSubmitResponse { ok: true, id }))
}

async fn public_teams_lead_submit(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<TeamsLeadSubmitRequest>,
) -> Result<Json<WaitlistSubmitResponse>, StatusCode> {
    let email = request.email.trim().to_lowercase();
    if email.is_empty() || !email.contains('@') || email.len() > 320 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let id = Uuid::new_v4();
    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let trim_opt = |s: Option<String>| s.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let name = trim_opt(request.name);
    let company = trim_opt(request.company);
    let message = trim_opt(request.message);

    sqlx::query(
        r#"
        INSERT INTO waitlist_entries (
            id, kind, email, name, company, message, user_agent
        )
        VALUES ($1, 'teams_lead', $2, $3, $4, $5, $6)
        "#,
    )
    .bind(id)
    .bind(&email)
    .bind(&name)
    .bind(&company)
    .bind(&message)
    .bind(&user_agent)
    .execute(&state.pool)
    .await
    .map_err(|err| {
        tracing::error!(?err, "failed to insert teams lead");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    send_form_email(
        &state,
        "New Guildest teams demo request",
        "New teams demo request",
        vec![
            ("Name", name.as_deref()),
            ("Email", Some(email.as_str())),
            ("Company", company.as_deref()),
            ("Message", message.as_deref()),
            ("User agent", user_agent.as_deref()),
        ],
        Some(email.as_str()),
    )
    .await
    .map_err(|err| {
        tracing::error!(?err, "failed to send teams lead email");
        StatusCode::BAD_GATEWAY
    })?;

    Ok(Json(WaitlistSubmitResponse { ok: true, id }))
}

async fn public_subscribe_submit(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<SubscribeSubmitRequest>,
) -> Result<Json<WaitlistSubmitResponse>, StatusCode> {
    let email = request.email.trim().to_lowercase();
    if !is_valid_email(&email) {
        return Err(StatusCode::BAD_REQUEST);
    }

    let id = Uuid::new_v4();
    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    sqlx::query(
        r#"
        INSERT INTO waitlist_entries (
            id, kind, email, user_agent
        )
        VALUES ($1, 'subscribe', $2, $3)
        "#,
    )
    .bind(id)
    .bind(&email)
    .bind(&user_agent)
    .execute(&state.pool)
    .await
    .map_err(|err| {
        tracing::error!(?err, "failed to insert subscriber");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    send_form_email(
        &state,
        "New Guildest subscriber",
        "New subscriber",
        vec![
            ("Email", Some(email.as_str())),
            ("User agent", user_agent.as_deref()),
        ],
        Some(email.as_str()),
    )
    .await
    .map_err(|err| {
        tracing::error!(?err, "failed to send subscriber email");
        StatusCode::BAD_GATEWAY
    })?;

    Ok(Json(WaitlistSubmitResponse { ok: true, id }))
}

async fn send_form_email(
    state: &AppState,
    subject: &str,
    heading: &str,
    fields: Vec<(&str, Option<&str>)>,
    reply_to: Option<&str>,
) -> Result<()> {
    let Some(api_key) = state.settings.resend_api_key.as_deref() else {
        tracing::warn!("RESEND_API_KEY is not configured; skipping form email");
        return Ok(());
    };

    if state.settings.guildest_email_to.is_empty() {
        tracing::warn!("GUILDEST_EMAIL_TO is empty; skipping form email");
        return Ok(());
    }

    let normalized_fields = fields
        .into_iter()
        .map(|(label, value)| (label, value.map(str::trim).filter(|v| !v.is_empty())))
        .collect::<Vec<_>>();
    let text = format_email_text(heading, &normalized_fields);
    let html = format_email_html(heading, &normalized_fields);

    let response = state
        .http_client
        .post(RESEND_EMAILS_URL)
        .bearer_auth(api_key)
        .json(&ResendEmailRequest {
            from: state.settings.resend_from_email.clone(),
            to: state.settings.guildest_email_to.clone(),
            subject: subject.to_string(),
            text,
            html,
            reply_to: reply_to
                .map(str::trim)
                .filter(|email| is_valid_email(email))
                .map(str::to_string),
        })
        .send()
        .await
        .context("failed to call Resend")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("Resend returned {status}: {body}");
    }

    Ok(())
}

fn format_email_text(heading: &str, fields: &[(&str, Option<&str>)]) -> String {
    let mut lines = vec![
        heading.to_string(),
        String::new(),
        "thank you so much: thanks111!!!!".to_string(),
        "Seriously, I am glad you sent this in. Guildest is still being built by a real person, so every signup, note, and tiny bit of context means a lot.".to_string(),
        String::new(),
    ];
    lines.extend(
        fields
            .iter()
            .map(|(label, value)| format!("{label}: {}", value.unwrap_or("Not provided"))),
    );
    lines.join("\n")
}

fn format_email_html(heading: &str, fields: &[(&str, Option<&str>)]) -> String {
    let rows = fields
        .iter()
        .map(|(label, value)| {
            format!(
                r#"<tr><td style="padding:8px 12px;color:#6b625f;font-size:13px;border-bottom:1px solid #eee;">{}</td><td style="padding:8px 12px;color:#211715;font-size:13px;border-bottom:1px solid #eee;white-space:pre-wrap;">{}</td></tr>"#,
                escape_html(label),
                escape_html(value.unwrap_or("Not provided")),
            )
        })
        .collect::<Vec<_>>()
        .join("");

    format!(
        r#"<div style="font-family:'Times New Roman',Times,serif;color:#211715;font-size:16px;line-height:1.5;"><h1 style="font-size:24px;line-height:1.25;font-weight:400;margin:0 0 18px;">{}</h1><p style="margin:0 0 10px;">thank you so much: thanks111!!!!</p><p style="margin:0 0 22px;">Seriously, I am glad you sent this in. Guildest is still being built by a real person, so every signup, note, and tiny bit of context means a lot.</p><table style="border-collapse:collapse;width:100%;max-width:680px;"><tbody>{rows}</tbody></table></div>"#,
        escape_html(heading),
    )
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#039;")
}

fn is_valid_email(email: &str) -> bool {
    let Some((local, domain)) = email.split_once('@') else {
        return false;
    };

    !local.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !email.chars().any(char::is_whitespace)
        && email.len() <= 320
}

async fn public_waitlist_export(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    let expected = std::env::var("GUILDEST_ADMIN_TOKEN").unwrap_or_default();
    if expected.is_empty() {
        return Err(StatusCode::NOT_FOUND);
    }
    let provided = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .unwrap_or("");
    if provided != expected {
        return Err(StatusCode::UNAUTHORIZED);
    }

    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        kind: String,
        discord_user_id: Option<String>,
        discord_username: Option<String>,
        discord_display_name: Option<String>,
        email: Option<String>,
        name: Option<String>,
        company: Option<String>,
        source: Option<String>,
        use_case: Option<String>,
        message: Option<String>,
        notes: Option<String>,
        created_at: chrono::DateTime<chrono::Utc>,
    }

    let rows = sqlx::query_as::<_, Row>(
        r#"
        SELECT id, kind, discord_user_id, discord_username, discord_display_name,
               email, name, company, source, use_case, message, notes, created_at
        FROM waitlist_entries
        ORDER BY created_at DESC
        "#,
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|err| {
        tracing::error!(?err, "failed to export waitlist");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    fn esc(s: &str) -> String {
        let needs = s.contains(',') || s.contains('"') || s.contains('\n');
        if needs {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s.to_string()
        }
    }

    let mut csv = String::from(
        "id,kind,created_at,discord_user_id,discord_username,discord_display_name,email,name,company,source,use_case,message,notes\n",
    );
    for r in rows {
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            r.id,
            esc(&r.kind),
            r.created_at.to_rfc3339(),
            esc(r.discord_user_id.as_deref().unwrap_or("")),
            esc(r.discord_username.as_deref().unwrap_or("")),
            esc(r.discord_display_name.as_deref().unwrap_or("")),
            esc(r.email.as_deref().unwrap_or("")),
            esc(r.name.as_deref().unwrap_or("")),
            esc(r.company.as_deref().unwrap_or("")),
            esc(r.source.as_deref().unwrap_or("")),
            esc(r.use_case.as_deref().unwrap_or("")),
            esc(r.message.as_deref().unwrap_or("")),
            esc(r.notes.as_deref().unwrap_or("")),
        ));
    }

    let mut response = csv.into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        "text/csv; charset=utf-8".parse().unwrap(),
    );
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        "attachment; filename=\"waitlist.csv\"".parse().unwrap(),
    );
    Ok(response)
}

async fn start_login_oauth(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Some(session_id) = read_session_id(&headers) {
        if fetch_dashboard_user(&state.pool, session_id).await.is_ok() {
            return Redirect::temporary(&dashboard_redirect_url(
                &state.settings.public_site_url,
                &[("status", "ready")],
            ));
        }
    }

    match build_login_authorize_url(&state.settings, OAuthFlow::Login) {
        Ok(url) => Redirect::temporary(&url),
        Err(error) => {
            warn!(?error, "failed to build login oauth url");
            Redirect::temporary(&dashboard_redirect_url(
                &state.settings.public_site_url,
                &[("status", "error"), ("reason", "oauth-start-failed")],
            ))
        }
    }
}

async fn start_invite_oauth(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match build_login_authorize_url(&state.settings, OAuthFlow::InviteAuth) {
        Ok(url) => Redirect::temporary(&url),
        Err(error) => {
            warn!(?error, "failed to build invite oauth url");
            Redirect::temporary(&dashboard_redirect_url(
                &state.settings.public_site_url,
                &[("status", "error"), ("reason", "oauth-start-failed")],
            ))
        }
    }
}

async fn start_install(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match build_install_authorize_url(&state.settings) {
        Ok(url) => Redirect::temporary(&url),
        Err(error) => {
            warn!(?error, "failed to build install url");
            Redirect::temporary(&dashboard_redirect_url(
                &state.settings.public_site_url,
                &[("status", "error"), ("reason", "install-start-failed")],
            ))
        }
    }
}

async fn public_stats(State(state): State<Arc<AppState>>) -> Result<Response, StatusCode> {
    let request_started = Instant::now();
    let response = current_public_stats(state.as_ref());
    observe_api_request(
        state.metrics,
        "public_stats",
        StatusCode::OK,
        request_started.elapsed().as_secs_f64(),
        estimate_json_size(&response),
    );

    Ok((
        [(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store, max-age=0"),
        )],
        Json(response),
    )
        .into_response())
}

async fn public_message_heatmap(
    State(state): State<Arc<AppState>>,
    Query(query): Query<DaysQuery>,
) -> Result<Response, StatusCode> {
    let request_started = Instant::now();
    let days = sanitize_heatmap_days(query.days);
    let query_started = Instant::now();
    let heatmap_result = fetch_message_heatmap_days(&state.pool, None, days).await;
    observe_sql_query(
        state.metrics,
        "public_message_heatmap",
        status_label(heatmap_result.as_ref().map(|_| &())),
        query_started.elapsed().as_secs_f64(),
    );
    let (daily, window_start, window_end) =
        heatmap_result.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let total_messages = daily.iter().map(|day| day.message_count).sum();

    let response = PublicMessageHeatmapResponse {
        days: daily,
        days_requested: days,
        scope: "all_guilds",
        time_zone: "UTC",
        total_messages,
        window_end_utc: window_end.to_rfc3339(),
        window_start_utc: window_start.to_rfc3339(),
    };
    observe_api_request(
        state.metrics,
        "public_message_heatmap",
        StatusCode::OK,
        request_started.elapsed().as_secs_f64(),
        estimate_json_size(&response),
    );

    Ok((
        [(
            header::CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=60"),
        )],
        Json(response),
    )
        .into_response())
}

async fn public_stats_stream(
    State(state): State<Arc<AppState>>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, StatusCode> {
    let initial_event = build_public_stats_event(&current_public_stats(state.as_ref()))?;
    let stream = stream::unfold(
        PublicStatsEventStream {
            initial_event: Some(initial_event),
            updates: state.public_stats_cache.subscribe(),
        },
        |mut stream_state| async move {
            if let Some(event) = stream_state.initial_event.take() {
                return Some((Ok(event), stream_state));
            }

            if stream_state.updates.changed().await.is_err() {
                return None;
            }

            let response = stream_state.updates.borrow().clone();
            match build_public_stats_event(&response) {
                Ok(event) => Some((Ok(event), stream_state)),
                Err(status) => {
                    warn!(?status, "failed to build public stats sse event");
                    None
                }
            }
        },
    );

    Ok(Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    ))
}

fn build_public_stats_event(response: &PublicStatsResponse) -> Result<Event, StatusCode> {
    let payload = serde_json::to_string(response).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Event::default().event("public_stats").data(payload))
}

fn current_public_stats(state: &AppState) -> PublicStatsResponse {
    state.public_stats_cache.borrow().clone()
}

async fn load_public_stats_snapshot(
    pool: &PgPool,
    queue: &RedisEventQueue,
) -> Result<PublicStatsResponse> {
    if let Some(response) = load_public_stats_from_hot_cache(queue).await {
        return Ok(response);
    }

    let row = fetch_public_stats_row(pool)
        .await
        .context("failed to fetch public stats row")?;
    Ok(PublicStatsResponse {
        members: row.members,
        messages_tracked: row.messages_tracked,
        servers: row.servers,
    })
}

async fn refresh_public_stats_snapshot(state: &AppState) -> Result<()> {
    let query_started = Instant::now();
    let snapshot = load_public_stats_snapshot(&state.pool, &state.queue).await;
    if snapshot.is_err() {
        observe_sql_query(
            state.metrics,
            "public_stats",
            "error",
            query_started.elapsed().as_secs_f64(),
        );
    }
    let snapshot = snapshot?;
    observe_sql_query(
        state.metrics,
        "public_stats",
        "ok",
        query_started.elapsed().as_secs_f64(),
    );

    if *state.public_stats_cache.borrow() != snapshot {
        state.public_stats_cache.send_replace(snapshot);
    }

    Ok(())
}

async fn run_public_stats_cache_updater(state: Arc<AppState>) {
    loop {
        let mut pubsub = match state.queue.pubsub().await {
            Ok(pubsub) => pubsub,
            Err(error) => {
                warn!(?error, "failed to open public stats pubsub connection");
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }
        };

        if let Err(error) = pubsub.subscribe(PUBLIC_STATS_UPDATES_CHANNEL).await {
            warn!(?error, "failed to subscribe to public stats updates");
            tokio::time::sleep(Duration::from_secs(1)).await;
            continue;
        }

        if let Err(error) = refresh_public_stats_snapshot(state.as_ref()).await {
            warn!(?error, "failed to refresh public stats snapshot");
        }

        let mut refresh_interval = tokio::time::interval(Duration::from_secs(30));
        refresh_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut messages = pubsub.on_message();

        loop {
            tokio::select! {
                _ = refresh_interval.tick() => {
                    if let Err(error) = refresh_public_stats_snapshot(state.as_ref()).await {
                        warn!(?error, "failed to refresh public stats snapshot");
                    }
                }
                next_message = messages.next() => {
                    match next_message {
                        Some(_) => {
                            if let Err(error) = refresh_public_stats_snapshot(state.as_ref()).await {
                                warn!(?error, "failed to refresh public stats snapshot");
                            }
                        }
                        None => {
                            warn!("public stats pubsub connection closed");
                            break;
                        }
                    }
                }
            }
        }

        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

async fn load_public_stats_from_hot_cache(queue: &RedisEventQueue) -> Option<PublicStatsResponse> {
    let values = match queue
        .mget_strings(&[
            PUBLIC_STATS_MESSAGES_KEY,
            PUBLIC_STATS_SERVERS_KEY,
            PUBLIC_STATS_MEMBERS_KEY,
        ])
        .await
    {
        Ok(values) => values,
        Err(error) => {
            warn!(?error, "failed to read public stats hot cache");
            return None;
        }
    };

    let [messages, servers, members] = values.as_slice() else {
        return None;
    };

    Some(PublicStatsResponse {
        messages_tracked: messages.as_deref()?.parse().ok()?,
        servers: servers.as_deref()?.parse().ok()?,
        members: members.as_deref()?.parse().ok()?,
    })
}

async fn oauth_callback(
    State(state): State<Arc<AppState>>,
    Query(query): Query<OAuthCallbackQuery>,
) -> impl IntoResponse {
    let Some(state_value) = query.state.as_deref() else {
        return redirect_response(
            &dashboard_redirect_url(
                &state.settings.public_site_url,
                &[("status", "error"), ("reason", "missing-state")],
            ),
            None,
        );
    };

    let Some(flow) = OAuthFlow::from_state(state_value) else {
        return redirect_response(
            &dashboard_redirect_url(
                &state.settings.public_site_url,
                &[("status", "error"), ("reason", "invalid-state")],
            ),
            None,
        );
    };

    if let Some(error) = query.error.as_deref() {
        warn!(
            flow = %flow.as_str(),
            error,
            description = query.error_description.as_deref().unwrap_or(""),
            "discord oauth callback returned an error"
        );

        return redirect_response(
            &dashboard_redirect_url(
                &state.settings.public_site_url,
                &[
                    ("status", "error"),
                    ("flow", flow.as_str()),
                    ("reason", "discord-denied"),
                ],
            ),
            None,
        );
    }

    match flow {
        OAuthFlow::InviteInstall => {
            let mut query_pairs = vec![("status", "installed")];
            if let Some(guild_id) = query.guild_id.as_deref() {
                query_pairs.push(("guild_id", guild_id));
            }
            if let Some(permissions) = query.permissions.as_deref() {
                query_pairs.push(("permissions", permissions));
            }

            return redirect_response(
                &dashboard_redirect_url(&state.settings.public_site_url, &query_pairs),
                None,
            );
        }
        OAuthFlow::Login | OAuthFlow::InviteAuth => {}
    }

    let Some(code) = query.code.as_deref() else {
        return redirect_response(
            &dashboard_redirect_url(
                &state.settings.public_site_url,
                &[
                    ("status", "error"),
                    ("flow", flow.as_str()),
                    ("reason", "missing-code"),
                ],
            ),
            None,
        );
    };

    let redirect_error = |reason: &'static str| {
        redirect_response(
            &dashboard_redirect_url(
                &state.settings.public_site_url,
                &[
                    ("status", "error"),
                    ("flow", flow.as_str()),
                    ("reason", reason),
                ],
            ),
            None,
        )
    };

    let token = match exchange_oauth_code(&state, code).await {
        Ok(token) => token,
        Err(error) => {
            warn!(flow = %flow.as_str(), ?error, "failed to exchange oauth code");
            return redirect_error("token-exchange-failed");
        }
    };

    let user = match fetch_discord_user(&state, &token.access_token).await {
        Ok(user) => user,
        Err(error) => {
            warn!(flow = %flow.as_str(), ?error, "failed to fetch oauth user");
            return redirect_error("user-fetch-failed");
        }
    };

    let guilds = match fetch_discord_user_guilds(&state, &token.access_token).await {
        Ok(guilds) => guilds,
        Err(error) => {
            warn!(flow = %flow.as_str(), ?error, "failed to fetch oauth guilds");
            return redirect_error("guild-fetch-failed");
        }
    };

    let session_result = match persist_session(&state, &user, &guilds).await {
        Ok(result) => result,
        Err(error) => {
            warn!(flow = %flow.as_str(), ?error, "failed to persist dashboard session");
            return redirect_error("session-persist-failed");
        }
    };

    let session_cookie = match build_session_cookie(&state.settings, session_result.session_id) {
        Ok(cookie) => Some(cookie),
        Err(error) => {
            warn!(?error, "failed to build session cookie");
            None
        }
    };

    let target = match flow {
        OAuthFlow::Login => {
            if session_result.accessible_guilds > 0 {
                dashboard_redirect_url(&state.settings.public_site_url, &[("status", "ready")])
            } else {
                dashboard_redirect_url(
                    &state.settings.public_site_url,
                    &[("status", "logged-in"), ("needs_invite", "1")],
                )
            }
        }
        OAuthFlow::InviteAuth => public_url(&state.settings, "/v1/public/install/start"),
        OAuthFlow::InviteInstall => unreachable!(),
    };

    redirect_response(&target, session_cookie)
}

async fn dashboard_me(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<DashboardMeResponse>, StatusCode> {
    let request_started = Instant::now();
    let session_id = read_session_id(&headers).ok_or(StatusCode::UNAUTHORIZED)?;
    let user = fetch_dashboard_user(&state.pool, session_id)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    let guilds = fetch_accessible_guilds(&state.pool, session_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let response = DashboardMeResponse {
        accessible_guilds: guilds,
        user,
    };
    observe_api_request(
        state.metrics,
        "dashboard_me",
        StatusCode::OK,
        request_started.elapsed().as_secs_f64(),
        estimate_json_size(&response),
    );

    Ok(Json(response))
}

async fn dashboard_guild_message_summary(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(guild_id): Path<String>,
    Query(query): Query<DaysQuery>,
) -> Result<Json<DashboardGuildSummaryResponse>, StatusCode> {
    let request_started = Instant::now();
    let session_id = read_session_id(&headers).ok_or(StatusCode::UNAUTHORIZED)?;
    ensure_guild_access(&state.pool, session_id, &guild_id).await?;

    let days = sanitize_days(query.days);
    let start_at = Utc::now()
        .checked_sub_days(Days::new(days as u64))
        .unwrap_or_else(|| Utc::now() - chrono::Duration::days(days as i64));

    let query_started = Instant::now();
    let daily_rows_result = sqlx::query_as::<_, MessageSummaryDayRow>(
        r#"
        SELECT
            activity_date AS day,
            messages AS message_count
        FROM guild_daily_activity
        WHERE guild_id = $1
          AND activity_date >= DATE($2)
        ORDER BY activity_date DESC
        "#,
    )
    .bind(&guild_id)
    .bind(start_at)
    .fetch_all(&state.pool)
    .await;
    observe_sql_query(
        state.metrics,
        "dashboard_guild_message_summary",
        status_label(daily_rows_result.as_ref().map(|_| &())),
        query_started.elapsed().as_secs_f64(),
    );
    let daily_rows = daily_rows_result.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let total_messages = daily_rows.iter().map(|row| row.message_count).sum();
    let backfill_status = sqlx::query_as::<_, BackfillStatusRow>(
        r#"
        SELECT status
        FROM historical_backfill_jobs
        WHERE guild_id = $1
        ORDER BY requested_at DESC
        LIMIT 1
        "#,
    )
    .bind(&guild_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .map(|row| row.status);

    let response = DashboardGuildSummaryResponse {
        backfill_status,
        daily: daily_rows
            .into_iter()
            .map(|row| MessageSummaryDay {
                date: row.day.to_string(),
                message_count: row.message_count,
            })
            .collect(),
        days_requested: days,
        guild_id,
        total_messages,
    };
    observe_api_request(
        state.metrics,
        "dashboard_guild_message_summary",
        StatusCode::OK,
        request_started.elapsed().as_secs_f64(),
        estimate_json_size(&response),
    );

    Ok(Json(response))
}

async fn dashboard_guild_message_heatmap(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(guild_id): Path<String>,
    Query(query): Query<DaysQuery>,
) -> Result<Json<DashboardMessageHeatmapResponse>, StatusCode> {
    let request_started = Instant::now();
    let session_id = read_session_id(&headers).ok_or(StatusCode::UNAUTHORIZED)?;
    ensure_guild_access(&state.pool, session_id, &guild_id).await?;

    let days = sanitize_heatmap_days(query.days);
    let query_started = Instant::now();
    let heatmap_result = fetch_message_heatmap_days(&state.pool, Some(&guild_id), days).await;
    observe_sql_query(
        state.metrics,
        "dashboard_guild_message_heatmap",
        status_label(heatmap_result.as_ref().map(|_| &())),
        query_started.elapsed().as_secs_f64(),
    );
    let (daily, window_start, window_end) =
        heatmap_result.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let total_messages = daily.iter().map(|day| day.message_count).sum();

    let response = DashboardMessageHeatmapResponse {
        days: daily,
        days_requested: days,
        guild_id,
        time_zone: "UTC",
        total_messages,
        window_end_utc: window_end.to_rfc3339(),
        window_start_utc: window_start.to_rfc3339(),
    };
    observe_api_request(
        state.metrics,
        "dashboard_guild_message_heatmap",
        StatusCode::OK,
        request_started.elapsed().as_secs_f64(),
        estimate_json_size(&response),
    );

    Ok(Json(response))
}

async fn fetch_message_heatmap_days(
    pool: &PgPool,
    guild_id: Option<&str>,
    days: i32,
) -> Result<
    (
        Vec<MessageSummaryDay>,
        chrono::DateTime<Utc>,
        chrono::DateTime<Utc>,
    ),
    sqlx::Error,
> {
    let today = Utc::now().date_naive();
    let start_date = today - chrono::Duration::days((days - 1) as i64);
    let window_start = start_date
        .and_hms_opt(0, 0, 0)
        .expect("UTC midnight must be valid")
        .and_utc();
    let window_end = today
        .and_hms_opt(0, 0, 0)
        .expect("UTC midnight must be valid")
        .and_utc()
        + chrono::Duration::days(1);

    let rows = if let Some(guild_id) = guild_id {
        sqlx::query_as::<_, MessageSummaryDayRow>(
            r#"
            WITH daily_activity AS (
                SELECT
                    activity_date AS day,
                    SUM(messages)::BIGINT AS message_count,
                    MAX(last_message_at) AS last_message_at
                FROM guild_daily_activity
                WHERE guild_id = $1
                  AND activity_date >= DATE($2)
                  AND activity_date < DATE($3)
                GROUP BY activity_date
            ),
            indexed_messages AS (
                SELECT
                    (message.occurred_at AT TIME ZONE 'UTC')::DATE AS day,
                    COUNT(*)::BIGINT AS message_count
                FROM message_index AS message
                LEFT JOIN guild_daily_activity AS daily
                  ON daily.guild_id = message.guild_id
                 AND daily.activity_date = (message.occurred_at AT TIME ZONE 'UTC')::DATE
                WHERE message.guild_id = $1
                  AND message.occurred_at >= $2
                  AND message.occurred_at < $3
                  AND (
                    daily.last_message_at IS NULL
                    OR message.occurred_at > daily.last_message_at
                  )
                GROUP BY day
            )
            SELECT
                COALESCE(daily_activity.day, indexed_messages.day) AS day,
                (
                    COALESCE(daily_activity.message_count, 0)
                    + COALESCE(indexed_messages.message_count, 0)
                )::BIGINT AS message_count
            FROM daily_activity
            FULL OUTER JOIN indexed_messages
              ON indexed_messages.day = daily_activity.day
            ORDER BY day ASC
            "#,
        )
        .bind(guild_id)
        .bind(window_start)
        .bind(window_end)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as::<_, MessageSummaryDayRow>(
            r#"
            WITH daily_activity AS (
                SELECT
                    activity_date AS day,
                    SUM(messages)::BIGINT AS message_count
                FROM guild_daily_activity
                WHERE activity_date >= DATE($1)
                  AND activity_date < DATE($2)
                GROUP BY activity_date
            ),
            indexed_messages AS (
                SELECT
                    (message.occurred_at AT TIME ZONE 'UTC')::DATE AS day,
                    COUNT(*)::BIGINT AS message_count
                FROM message_index AS message
                LEFT JOIN guild_daily_activity AS daily
                  ON daily.guild_id = message.guild_id
                 AND daily.activity_date = (message.occurred_at AT TIME ZONE 'UTC')::DATE
                WHERE message.occurred_at >= $1
                  AND message.occurred_at < $2
                  AND (
                    daily.last_message_at IS NULL
                    OR message.occurred_at > daily.last_message_at
                  )
                GROUP BY day
            )
            SELECT
                COALESCE(daily_activity.day, indexed_messages.day) AS day,
                (
                    COALESCE(daily_activity.message_count, 0)
                    + COALESCE(indexed_messages.message_count, 0)
                )::BIGINT AS message_count
            FROM daily_activity
            FULL OUTER JOIN indexed_messages
              ON indexed_messages.day = daily_activity.day
            ORDER BY day ASC
            "#,
        )
        .bind(window_start)
        .bind(window_end)
        .fetch_all(pool)
        .await?
    };

    let counts_by_day = rows
        .into_iter()
        .map(|row| (row.day, row.message_count))
        .collect::<HashMap<_, _>>();
    let daily = (0..days)
        .map(|offset| {
            let date = start_date + chrono::Duration::days(offset as i64);
            MessageSummaryDay {
                date: date.to_string(),
                message_count: counts_by_day.get(&date).copied().unwrap_or(0),
            }
        })
        .collect::<Vec<_>>();

    Ok((daily, window_start, window_end))
}

async fn dashboard_activation_funnel(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(guild_id): Path<String>,
    Query(query): Query<DaysQuery>,
) -> Result<Json<DashboardActivationFunnelResponse>, StatusCode> {
    let request_started = Instant::now();
    let session_id = read_session_id(&headers).ok_or(StatusCode::UNAUTHORIZED)?;
    ensure_guild_access(&state.pool, session_id, &guild_id).await?;

    let days = sanitize_days(query.days);
    let start_at = Utc::now()
        .checked_sub_days(Days::new(days as u64))
        .unwrap_or_else(|| Utc::now() - chrono::Duration::days(days as i64));
    let start_date = start_at.date_naive();

    let query_started = Instant::now();
    let row_result = sqlx::query_as::<_, ActivationFunnelRow>(
        r#"
        SELECT
            COALESCE(SUM(joined_members), 0)::BIGINT AS joined_count,
            COALESCE(SUM(got_role_members), 0)::BIGINT AS got_role_count,
            COALESCE(SUM(first_message_members), 0)::BIGINT AS first_message_count,
            COALESCE(SUM(first_reaction_members), 0)::BIGINT AS first_reaction_count,
            COALESCE(SUM(first_voice_members), 0)::BIGINT AS first_voice_count,
            COALESCE(SUM(returned_next_week_members), 0)::BIGINT AS returned_next_week_count
        FROM activation_funnel_daily
        WHERE guild_id = $1
          AND cohort_date >= $2
        "#,
    )
    .bind(&guild_id)
    .bind(start_date)
    .fetch_one(&state.pool)
    .await;
    observe_sql_query(
        state.metrics,
        "dashboard_activation_funnel",
        status_label(row_result.as_ref().map(|_| &())),
        query_started.elapsed().as_secs_f64(),
    );
    let row = row_result.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let response = DashboardActivationFunnelResponse {
        days_requested: days,
        guild_id,
        steps: vec![
            DashboardActivationFunnelStep {
                count: row.joined_count,
                key: "joined",
                label: "Joined",
            },
            DashboardActivationFunnelStep {
                count: row.got_role_count,
                key: "got_role",
                label: "Got role",
            },
            DashboardActivationFunnelStep {
                count: row.first_message_count,
                key: "first_message",
                label: "First message",
            },
            DashboardActivationFunnelStep {
                count: row.first_reaction_count,
                key: "first_reaction",
                label: "First reaction",
            },
            DashboardActivationFunnelStep {
                count: row.first_voice_count,
                key: "first_voice",
                label: "First voice",
            },
            DashboardActivationFunnelStep {
                count: row.returned_next_week_count,
                key: "returned_next_week",
                label: "Returned next week",
            },
        ],
    };
    observe_api_request(
        state.metrics,
        "dashboard_activation_funnel",
        StatusCode::OK,
        request_started.elapsed().as_secs_f64(),
        estimate_json_size(&response),
    );

    Ok(Json(response))
}

async fn dashboard_guild_health_summary(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(guild_id): Path<String>,
    Query(query): Query<DaysQuery>,
) -> Result<Json<DashboardGuildHealthSummaryResponse>, StatusCode> {
    let request_started = Instant::now();
    let session_id = read_session_id(&headers).ok_or(StatusCode::UNAUTHORIZED)?;
    ensure_guild_access(&state.pool, session_id, &guild_id).await?;

    let days = sanitize_days(query.days);
    let start_at = Utc::now()
        .checked_sub_days(Days::new(days as u64))
        .unwrap_or_else(|| Utc::now() - chrono::Duration::days(days as i64));
    let start_date = start_at.date_naive();
    let cache_key = dashboard_cache_key(&guild_id, "summary_health", days);

    if let Some(response) =
        read_dashboard_cache::<DashboardGuildHealthSummaryResponse>(state.as_ref(), &cache_key)
            .await
    {
        observe_api_request(
            state.metrics,
            "dashboard_guild_health_summary",
            StatusCode::OK,
            request_started.elapsed().as_secs_f64(),
            estimate_json_size(&response),
        );
        return Ok(Json(response));
    }

    let query_started = Instant::now();
    let row_result = sqlx::query_as::<_, GuildHealthSummaryRow>(
        r#"
        SELECT
            COALESCE((
                SELECT active_members::BIGINT
                FROM guild_summary_daily
                WHERE guild_id = $1
                  AND summary_date = CURRENT_DATE
                LIMIT 1
            ), 0) AS dau,
            COALESCE((
                SELECT COUNT(DISTINCT member_id)::BIGINT
                FROM member_daily_activity
                WHERE guild_id = $1
                  AND activity_date >= CURRENT_DATE - 6
            ), 0) AS wau,
            COALESCE((
                SELECT COUNT(DISTINCT member_id)::BIGINT
                FROM member_daily_activity
                WHERE guild_id = $1
                  AND activity_date >= CURRENT_DATE - 29
            ), 0) AS mau,
            COALESCE((
                SELECT SUM(joined_members)::BIGINT
                FROM guild_summary_daily
                WHERE guild_id = $1
                  AND summary_date >= $2
            ), 0) AS joined_members,
            COALESCE((
                SELECT SUM(left_members)::BIGINT
                FROM guild_summary_daily
                WHERE guild_id = $1
                  AND summary_date >= $2
            ), 0) AS left_members,
            COALESCE((
                SELECT SUM(onboarded_members)::BIGINT
                FROM guild_summary_daily
                WHERE guild_id = $1
                  AND summary_date >= $2
            ), 0) AS onboarded_members
        "#,
    )
    .bind(&guild_id)
    .bind(start_date)
    .fetch_one(&state.pool)
    .await;
    observe_sql_query(
        state.metrics,
        "dashboard_guild_health_summary",
        status_label(row_result.as_ref().map(|_| &())),
        query_started.elapsed().as_secs_f64(),
    );
    let row = row_result.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let response = DashboardGuildHealthSummaryResponse {
        dau: row.dau,
        days_requested: days,
        guild_id,
        join_leave_ratio: if row.left_members > 0 {
            Some(row.joined_members as f64 / row.left_members as f64)
        } else if row.joined_members > 0 {
            Some(row.joined_members as f64)
        } else {
            None
        },
        joined_members: row.joined_members,
        left_members: row.left_members,
        onboarding_completion_rate: if row.joined_members > 0 {
            Some(row.onboarded_members as f64 / row.joined_members as f64)
        } else {
            None
        },
        onboarded_members: row.onboarded_members,
        wau: row.wau,
        mau: row.mau,
    };
    observe_api_request(
        state.metrics,
        "dashboard_guild_health_summary",
        StatusCode::OK,
        request_started.elapsed().as_secs_f64(),
        estimate_json_size(&response),
    );
    write_dashboard_cache(state.as_ref(), &cache_key, &response).await;

    Ok(Json(response))
}

async fn dashboard_retention_cohorts(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(guild_id): Path<String>,
    Query(query): Query<DaysQuery>,
) -> Result<Json<DashboardRetentionCohortsResponse>, StatusCode> {
    let request_started = Instant::now();
    let session_id = read_session_id(&headers).ok_or(StatusCode::UNAUTHORIZED)?;
    ensure_guild_access(&state.pool, session_id, &guild_id).await?;

    let days = sanitize_days(query.days);
    let start_at = Utc::now()
        .checked_sub_days(Days::new(days as u64))
        .unwrap_or_else(|| Utc::now() - chrono::Duration::days(days as i64));
    let start_date = start_at.date_naive();
    let cache_key = dashboard_cache_key(&guild_id, "retention_snapshot", days);

    if let Some(response) =
        read_dashboard_cache::<DashboardRetentionCohortsResponse>(state.as_ref(), &cache_key).await
    {
        observe_api_request(
            state.metrics,
            "dashboard_retention_cohorts",
            StatusCode::OK,
            request_started.elapsed().as_secs_f64(),
            estimate_json_size(&response),
        );
        return Ok(Json(response));
    }

    let query_started = Instant::now();
    let rows_result = sqlx::query_as::<_, RetentionCohortRow>(
        r#"
        SELECT
            cohort_date,
            EXTRACT(DAY FROM (CURRENT_DATE - cohort_date))::INTEGER AS cohort_age_days,
            joined_members AS joined_count,
            d7_retained_members AS d7_retained,
            d30_retained_members AS d30_retained
        FROM retention_cohorts
        WHERE guild_id = $1
          AND cohort_date >= $2
        ORDER BY cohort_date DESC
        "#,
    )
    .bind(&guild_id)
    .bind(start_date)
    .fetch_all(&state.pool)
    .await;
    observe_sql_query(
        state.metrics,
        "dashboard_retention_cohorts",
        status_label(rows_result.as_ref().map(|_| &())),
        query_started.elapsed().as_secs_f64(),
    );
    let rows = rows_result.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut d7_joined_total = 0_i64;
    let mut d7_retained_total = 0_i64;
    let mut d30_joined_total = 0_i64;
    let mut d30_retained_total = 0_i64;

    let cohorts = rows
        .into_iter()
        .map(|row| {
            if row.cohort_age_days >= 7 {
                d7_joined_total += row.joined_count;
                d7_retained_total += row.d7_retained;
            }
            if row.cohort_age_days >= 30 {
                d30_joined_total += row.joined_count;
                d30_retained_total += row.d30_retained;
            }

            DashboardRetentionCohort {
                cohort_age_days: row.cohort_age_days,
                cohort_date: row.cohort_date.to_string(),
                d30_retained: row.d30_retained,
                d30_retention_rate: if row.cohort_age_days >= 30 && row.joined_count > 0 {
                    Some(row.d30_retained as f64 / row.joined_count as f64)
                } else {
                    None
                },
                d7_retained: row.d7_retained,
                d7_retention_rate: if row.cohort_age_days >= 7 && row.joined_count > 0 {
                    Some(row.d7_retained as f64 / row.joined_count as f64)
                } else {
                    None
                },
                joined_count: row.joined_count,
            }
        })
        .collect();

    let response = DashboardRetentionCohortsResponse {
        cohorts,
        d30_retention_rate: if d30_joined_total > 0 {
            Some(d30_retained_total as f64 / d30_joined_total as f64)
        } else {
            None
        },
        d7_retention_rate: if d7_joined_total > 0 {
            Some(d7_retained_total as f64 / d7_joined_total as f64)
        } else {
            None
        },
        days_requested: days,
        guild_id,
    };
    observe_api_request(
        state.metrics,
        "dashboard_retention_cohorts",
        StatusCode::OK,
        request_started.elapsed().as_secs_f64(),
        estimate_json_size(&response),
    );
    write_dashboard_cache(state.as_ref(), &cache_key, &response).await;

    Ok(Json(response))
}

async fn dashboard_guild_users_summary(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(guild_id): Path<String>,
    Query(query): Query<DaysQuery>,
) -> Result<Json<DashboardGuildUsersResponse>, StatusCode> {
    const TOP_USER_LIMIT: i64 = 10;

    let request_started = Instant::now();
    let session_id = read_session_id(&headers).ok_or(StatusCode::UNAUTHORIZED)?;
    ensure_guild_access(&state.pool, session_id, &guild_id).await?;

    let days = sanitize_days(query.days);
    let start_at = Utc::now()
        .checked_sub_days(Days::new(days as u64))
        .unwrap_or_else(|| Utc::now() - chrono::Duration::days(days as i64));

    let summary_query_started = Instant::now();
    let summary_result = sqlx::query_as::<_, UserSummaryRow>(
        r#"
        SELECT
            COUNT(*)::BIGINT AS total_active_users,
            COALESCE(SUM(messages_sent), 0)::BIGINT AS total_messages,
            COALESCE(SUM(voice_seconds), 0)::BIGINT AS total_voice_seconds
        FROM (
            SELECT
                member_id,
                SUM(messages_sent)::BIGINT AS messages_sent,
                SUM(voice_seconds)::BIGINT AS voice_seconds
            FROM member_daily_activity
            WHERE guild_id = $1
              AND activity_date >= DATE($2)
            GROUP BY member_id
        ) AS member_totals
        "#,
    )
    .bind(&guild_id)
    .bind(start_at)
    .fetch_one(&state.pool)
    .await;
    observe_sql_query(
        state.metrics,
        "dashboard_guild_users_summary_totals",
        status_label(summary_result.as_ref().map(|_| &())),
        summary_query_started.elapsed().as_secs_f64(),
    );
    let summary = summary_result.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let query_started = Instant::now();
    let rows_result = sqlx::query_as::<_, TopUserRow>(
        r#"
        SELECT
            member_id AS discord_user_id,
            SUM(messages_sent)::BIGINT AS messages_sent,
            SUM(reactions_added)::BIGINT AS reactions_added,
            SUM(voice_seconds)::BIGINT AS voice_seconds,
            COUNT(*)::BIGINT AS active_days
        FROM member_daily_activity
        WHERE guild_id = $1
          AND activity_date >= DATE($2)
        GROUP BY member_id
        ORDER BY messages_sent DESC, reactions_added DESC, member_id ASC
        LIMIT $3
        "#,
    )
    .bind(&guild_id)
    .bind(start_at)
    .bind(TOP_USER_LIMIT)
    .fetch_all(&state.pool)
    .await;
    observe_sql_query(
        state.metrics,
        "dashboard_guild_users_summary",
        status_label(rows_result.as_ref().map(|_| &())),
        query_started.elapsed().as_secs_f64(),
    );
    let rows = rows_result.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let requested_user_ids = rows
        .iter()
        .map(|row| row.discord_user_id.clone())
        .collect::<Vec<_>>();
    let member_labels = resolve_member_labels(state.as_ref(), &guild_id, &requested_user_ids).await;

    let avg_messages_per_active_user = if summary.total_active_users > 0 {
        summary.total_messages as f64 / summary.total_active_users as f64
    } else {
        0.0
    };

    let response = DashboardGuildUsersResponse {
        avg_messages_per_active_user,
        days_requested: days,
        guild_id,
        total_active_users: summary.total_active_users,
        total_voice_seconds: summary.total_voice_seconds,
        users: rows
            .into_iter()
            .map(|row| DashboardTopUser {
                active_days: row.active_days,
                discord_user_id: row.discord_user_id.clone(),
                label: member_display_label(&member_labels, &row.discord_user_id),
                secondary_label: member_secondary_label(&member_labels, &row.discord_user_id),
                messages_sent: row.messages_sent,
                reactions_added: row.reactions_added,
                voice_seconds: row.voice_seconds,
            })
            .collect(),
    };
    observe_api_request(
        state.metrics,
        "dashboard_guild_users_summary",
        StatusCode::OK,
        request_started.elapsed().as_secs_f64(),
        estimate_json_size(&response),
    );

    Ok(Json(response))
}

async fn dashboard_guild_hotspots(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(guild_id): Path<String>,
    Query(query): Query<DaysQuery>,
) -> Result<Json<DashboardGuildHotspotsResponse>, StatusCode> {
    const TOP_CHANNEL_LIMIT: i64 = 5;
    const TOP_RETENTION_CHANNEL_LIMIT: i64 = 5;

    let request_started = Instant::now();
    let session_id = read_session_id(&headers).ok_or(StatusCode::UNAUTHORIZED)?;
    ensure_guild_access(&state.pool, session_id, &guild_id).await?;

    let days = sanitize_days(query.days);
    let start_at = Utc::now()
        .checked_sub_days(Days::new(days as u64))
        .unwrap_or_else(|| Utc::now() - chrono::Duration::days(days as i64));
    let cache_key = dashboard_cache_key(&guild_id, "hotspots", days);

    if let Some(response) =
        read_dashboard_cache::<DashboardGuildHotspotsResponse>(state.as_ref(), &cache_key).await
    {
        observe_api_request(
            state.metrics,
            "dashboard_guild_hotspots",
            StatusCode::OK,
            request_started.elapsed().as_secs_f64(),
            estimate_json_size(&response),
        );
        return Ok(Json(response));
    }

    let summary_query_started = Instant::now();
    let channel_summary_result = sqlx::query_as::<_, ChannelSummaryRow>(
        r#"
        SELECT
            COUNT(DISTINCT channel_id)::BIGINT AS active_channels,
            COALESCE(SUM(messages), 0)::BIGINT AS total_messages
        FROM channel_health_daily
        WHERE guild_id = $1
          AND activity_date >= DATE($2)
        "#,
    )
    .bind(&guild_id)
    .bind(start_at)
    .fetch_one(&state.pool)
    .await;
    observe_sql_query(
        state.metrics,
        "dashboard_guild_hotspots_totals",
        status_label(channel_summary_result.as_ref().map(|_| &())),
        summary_query_started.elapsed().as_secs_f64(),
    );
    let channel_summary = channel_summary_result.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let channels_query_started = Instant::now();
    let top_channels_result = sqlx::query_as::<_, TopChannelRow>(
        r#"
        SELECT
            channel_id,
            SUM(CASE WHEN activity_date >= DATE($2) THEN messages ELSE 0 END)::BIGINT AS message_count,
            SUM(CASE WHEN activity_date >= DATE($2) THEN unique_senders ELSE 0 END)::BIGINT AS unique_senders,
            SUM(CASE WHEN activity_date >= DATE($2) THEN replies ELSE 0 END)::BIGINT AS replies,
            CASE
                WHEN SUM(CASE WHEN activity_date >= DATE($2) THEN response_samples ELSE 0 END) > 0
                THEN SUM(CASE WHEN activity_date >= DATE($2) THEN response_seconds_total ELSE 0 END)::DOUBLE PRECISION
                    / SUM(CASE WHEN activity_date >= DATE($2) THEN response_samples ELSE 0 END)::DOUBLE PRECISION
                ELSE NULL
            END AS avg_response_seconds
            ,
            SUM(
                CASE
                    WHEN activity_date < DATE($2)
                     AND activity_date >= DATE($2) - $4
                    THEN messages
                    ELSE 0
                END
            )::BIGINT AS previous_period_messages
        FROM channel_health_daily
        WHERE guild_id = $1
          AND activity_date >= DATE($2) - $4
        GROUP BY channel_id
        HAVING SUM(CASE WHEN activity_date >= DATE($2) THEN messages ELSE 0 END) > 0
        ORDER BY message_count DESC, replies DESC, unique_senders DESC, channel_id ASC
        LIMIT $3
        "#,
    )
    .bind(&guild_id)
    .bind(start_at)
    .bind(TOP_CHANNEL_LIMIT)
    .bind(days)
    .fetch_all(&state.pool)
    .await;
    observe_sql_query(
        state.metrics,
        "dashboard_guild_hotspots_channels",
        status_label(top_channels_result.as_ref().map(|_| &())),
        channels_query_started.elapsed().as_secs_f64(),
    );
    let top_channels = top_channels_result.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let retention_query_started = Instant::now();
    let retention_channels_result = sqlx::query_as::<_, RetentionChannelRow>(
        r#"
        SELECT
            channel_id,
            COALESCE(SUM(d7_retained_members), 0)::BIGINT AS d7_retained_members,
            COALESCE(SUM(d30_retained_members), 0)::BIGINT AS d30_retained_members
        FROM channel_retention_daily
        WHERE guild_id = $1
          AND cohort_date >= DATE($2)
        GROUP BY channel_id
        HAVING COALESCE(SUM(d7_retained_members), 0) > 0
            OR COALESCE(SUM(d30_retained_members), 0) > 0
        ORDER BY d30_retained_members DESC, d7_retained_members DESC, channel_id ASC
        LIMIT $3
        "#,
    )
    .bind(&guild_id)
    .bind(start_at)
    .bind(TOP_RETENTION_CHANNEL_LIMIT)
    .fetch_all(&state.pool)
    .await;
    observe_sql_query(
        state.metrics,
        "dashboard_guild_hotspots_retention_channels",
        status_label(retention_channels_result.as_ref().map(|_| &())),
        retention_query_started.elapsed().as_secs_f64(),
    );
    let retention_channels =
        retention_channels_result.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let requested_channel_ids = top_channels
        .iter()
        .map(|row| row.channel_id.clone())
        .chain(retention_channels.iter().map(|row| row.channel_id.clone()))
        .collect::<Vec<_>>();
    let channel_names =
        resolve_channel_names(state.as_ref(), &guild_id, &requested_channel_ids).await;

    let hourly_query_started = Instant::now();
    let hourly_rows_result = sqlx::query_as::<_, HourlyActivityRow>(
        r#"
        SELECT
            EXTRACT(HOUR FROM occurred_at AT TIME ZONE 'UTC')::INTEGER AS hour_of_day,
            COUNT(*)::BIGINT AS message_count
        FROM message_index
        WHERE guild_id = $1
          AND occurred_at >= $2
        GROUP BY hour_of_day
        ORDER BY hour_of_day ASC
        "#,
    )
    .bind(&guild_id)
    .bind(start_at)
    .fetch_all(&state.pool)
    .await;
    observe_sql_query(
        state.metrics,
        "dashboard_guild_hotspots_hourly",
        status_label(hourly_rows_result.as_ref().map(|_| &())),
        hourly_query_started.elapsed().as_secs_f64(),
    );
    let hourly_rows = hourly_rows_result.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let peak_hour_utc = hourly_rows
        .iter()
        .max_by_key(|row| (row.message_count, std::cmp::Reverse(row.hour_of_day)))
        .map(|row| format_hour_label(row.hour_of_day));

    let response = DashboardGuildHotspotsResponse {
        active_channels: channel_summary.active_channels,
        days_requested: days,
        guild_id,
        hourly_activity: hourly_rows
            .into_iter()
            .map(|row| HourlyActivityPoint {
                hour_label: format_hour_label(row.hour_of_day),
                hour_of_day: row.hour_of_day,
                message_count: row.message_count,
            })
            .collect(),
        peak_hour_utc,
        retention_channels: retention_channels
            .into_iter()
            .map(|row| DashboardRetentionChannel {
                channel_id: row.channel_id.clone(),
                d30_retained_members: row.d30_retained_members,
                d7_retained_members: row.d7_retained_members,
                label: channel_label(&channel_names, &row.channel_id),
                retention_score: (row.d7_retained_members + (row.d30_retained_members * 2)),
            })
            .collect(),
        top_channels: top_channels
            .into_iter()
            .map(|row| {
                let messages_per_sender = if row.unique_senders > 0 {
                    Some(row.message_count as f64 / row.unique_senders as f64)
                } else {
                    None
                };
                let trend_percent_change = if row.previous_period_messages > 0 {
                    Some(
                        ((row.message_count - row.previous_period_messages) as f64
                            / row.previous_period_messages as f64)
                            * 100.0,
                    )
                } else {
                    None
                };
                let health_score = channel_health_score(
                    row.message_count,
                    row.unique_senders,
                    row.replies,
                    row.previous_period_messages,
                    row.avg_response_seconds,
                );

                DashboardTopChannel {
                    avg_response_seconds: row.avg_response_seconds,
                    channel_id: row.channel_id.clone(),
                    health_score,
                    label: channel_label(&channel_names, &row.channel_id),
                    message_count: row.message_count,
                    messages_per_sender,
                    previous_period_messages: row.previous_period_messages,
                    replies: row.replies,
                    trend_percent_change,
                    unique_senders: row.unique_senders,
                }
            })
            .collect(),
        total_messages: channel_summary.total_messages,
    };
    observe_api_request(
        state.metrics,
        "dashboard_guild_hotspots",
        StatusCode::OK,
        request_started.elapsed().as_secs_f64(),
        estimate_json_size(&response),
    );
    write_dashboard_cache(state.as_ref(), &cache_key, &response).await;

    Ok(Json(response))
}

async fn dashboard_pipeline_health(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(guild_id): Path<String>,
) -> Result<Json<DashboardPipelineHealthResponse>, StatusCode> {
    let request_started = Instant::now();
    let session_id = read_session_id(&headers).ok_or(StatusCode::UNAUTHORIZED)?;
    ensure_guild_access(&state.pool, session_id, &guild_id).await?;

    let cache_key = dashboard_resource_cache_key(&guild_id, "pipeline_health");
    if let Some(response) =
        read_dashboard_cache::<DashboardPipelineHealthResponse>(state.as_ref(), &cache_key).await
    {
        observe_api_request(
            state.metrics,
            "dashboard_pipeline_health",
            StatusCode::OK,
            request_started.elapsed().as_secs_f64(),
            estimate_json_size(&response),
        );
        return Ok(Json(response));
    }

    let mut streams = Vec::new();
    let mut healthy_streams = 0_i64;
    let mut warning_streams = 0_i64;
    let mut critical_streams = 0_i64;
    let mut total_ready_messages = 0_i64;
    let mut total_pending_messages = 0_i64;
    let mut total_scheduled_retry_messages = 0_i64;
    let mut total_dead_letter_messages = 0_i64;
    let mut max_oldest_ready_age_seconds = 0_i64;
    let mut max_scheduled_retry_overdue_seconds = 0_i64;

    for stream in dashboard_stream_names() {
        let ready_messages = state
            .queue
            .stream_len(stream)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let pending_messages = state
            .queue
            .pending_count(stream, stream)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let dead_letter_messages = state
            .queue
            .stream_len(&dead_letter_stream_name(stream))
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let scheduled_retry_messages = state
            .queue
            .sorted_set_len(&scheduled_retry_set_name(stream))
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let now_ms = Utc::now().timestamp_millis();
        let oldest_ready_age_seconds = state
            .queue
            .oldest_stream_entry_ms(stream)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .map(|timestamp_ms| ((now_ms - timestamp_ms).max(0)) / 1000)
            .unwrap_or(0);
        let oldest_dead_letter_age_seconds = state
            .queue
            .oldest_stream_entry_ms(&dead_letter_stream_name(stream))
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .map(|timestamp_ms| ((now_ms - timestamp_ms).max(0)) / 1000)
            .unwrap_or(0);
        let scheduled_retry_overdue_seconds = state
            .queue
            .earliest_sorted_set_score_ms(&scheduled_retry_set_name(stream))
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .map(|timestamp_ms| ((now_ms - timestamp_ms).max(0)) / 1000)
            .unwrap_or(0);
        let status = pipeline_stream_status(
            ready_messages,
            pending_messages,
            scheduled_retry_messages,
            dead_letter_messages,
            oldest_ready_age_seconds,
            scheduled_retry_overdue_seconds,
        );

        match status {
            "healthy" => healthy_streams += 1,
            "warning" => warning_streams += 1,
            _ => critical_streams += 1,
        }

        total_ready_messages += ready_messages;
        total_pending_messages += pending_messages;
        total_scheduled_retry_messages += scheduled_retry_messages;
        total_dead_letter_messages += dead_letter_messages;
        max_oldest_ready_age_seconds = max_oldest_ready_age_seconds.max(oldest_ready_age_seconds);
        max_scheduled_retry_overdue_seconds =
            max_scheduled_retry_overdue_seconds.max(scheduled_retry_overdue_seconds);

        streams.push(DashboardPipelineStreamHealth {
            dead_letter_messages,
            label: pipeline_stream_label(stream).to_string(),
            oldest_dead_letter_age_seconds,
            oldest_ready_age_seconds,
            pending_messages,
            ready_messages,
            scheduled_retry_messages,
            scheduled_retry_overdue_seconds,
            status: status.to_string(),
            stream: stream.to_string(),
        });
    }

    let overall_status = if critical_streams > 0 {
        "critical"
    } else if warning_streams > 0 {
        "warning"
    } else {
        "healthy"
    };
    let response = DashboardPipelineHealthResponse {
        guild_id,
        healthy_streams,
        max_oldest_ready_age_seconds,
        max_scheduled_retry_overdue_seconds,
        overall_status: overall_status.to_string(),
        streams,
        total_dead_letter_messages,
        total_pending_messages,
        total_ready_messages,
        total_scheduled_retry_messages,
        total_streams: (healthy_streams + warning_streams + critical_streams),
    };
    observe_api_request(
        state.metrics,
        "dashboard_pipeline_health",
        StatusCode::OK,
        request_started.elapsed().as_secs_f64(),
        estimate_json_size(&response),
    );
    write_dashboard_cache(state.as_ref(), &cache_key, &response).await;

    Ok(Json(response))
}

async fn dashboard_pipeline_incidents(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(guild_id): Path<String>,
) -> Result<Json<DashboardPipelineIncidentsResponse>, StatusCode> {
    const RECENT_INCIDENT_LIMIT: usize = 10;

    let request_started = Instant::now();
    let session_id = read_session_id(&headers).ok_or(StatusCode::UNAUTHORIZED)?;
    ensure_guild_access(&state.pool, session_id, &guild_id).await?;

    let cache_key = dashboard_resource_cache_key(&guild_id, "pipeline_incidents");
    if let Some(response) =
        read_dashboard_cache::<DashboardPipelineIncidentsResponse>(state.as_ref(), &cache_key).await
    {
        observe_api_request(
            state.metrics,
            "dashboard_pipeline_incidents",
            StatusCode::OK,
            request_started.elapsed().as_secs_f64(),
            estimate_json_size(&response),
        );
        return Ok(Json(response));
    }

    let now = Utc::now();
    let mut incidents = Vec::new();
    let mut total_dead_letter_messages = 0_i64;

    for stream in dashboard_stream_names() {
        let dead_letter_stream = dead_letter_stream_name(stream);
        total_dead_letter_messages += state
            .queue
            .stream_len(&dead_letter_stream)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let payloads = state
            .queue
            .recent_stream_entries(&dead_letter_stream, RECENT_INCIDENT_LIMIT)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        for entry in payloads {
            let delivery: DeadLetterDeliveryPayload = serde_json::from_str(&entry.payload)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            incidents.push(DashboardPipelineIncident {
                attempts: delivery.attempts,
                age_seconds: ((now.timestamp_millis() - delivery.failed_at.timestamp_millis())
                    .max(0))
                    / 1000,
                dead_letter_entry_id: entry.id,
                delivery_id: delivery.delivery_id,
                error: delivery.error,
                failed_at: delivery.failed_at.to_rfc3339(),
                payload_preview: payload_preview(&delivery.payload),
                retry_key: delivery.retry_key,
                source_stream_label: pipeline_stream_label(&delivery.source_stream).to_string(),
                source_stream: delivery.source_stream,
            });
        }
    }

    incidents.sort_by(|left, right| right.failed_at.cmp(&left.failed_at));
    incidents.truncate(RECENT_INCIDENT_LIMIT);

    let response = DashboardPipelineIncidentsResponse {
        guild_id,
        incidents,
        total_dead_letter_messages,
    };
    observe_api_request(
        state.metrics,
        "dashboard_pipeline_incidents",
        StatusCode::OK,
        request_started.elapsed().as_secs_f64(),
        estimate_json_size(&response),
    );
    write_dashboard_cache(state.as_ref(), &cache_key, &response).await;

    Ok(Json(response))
}

async fn replay_dashboard_pipeline_incident(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(guild_id): Path<String>,
    Json(request): Json<ReplayPipelineIncidentRequest>,
) -> Result<Json<ReplayPipelineIncidentResponse>, StatusCode> {
    let request_started = Instant::now();
    let session_id = read_session_id(&headers).ok_or(StatusCode::UNAUTHORIZED)?;
    ensure_guild_access(&state.pool, session_id, &guild_id).await?;
    let user = fetch_dashboard_user(&state.pool, session_id)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    let operator_reason = normalize_operator_reason(request.operator_reason.as_deref());

    if !is_dashboard_stream(&request.source_stream) {
        return Err(StatusCode::BAD_REQUEST);
    }

    let action_lock_key =
        dead_letter_action_lock_key(&request.source_stream, &request.dead_letter_entry_id);
    let claimed = state
        .queue
        .claim_key_with_ttl(
            &action_lock_key,
            &user.discord_user_id,
            DEAD_LETTER_ACTION_LOCK_TTL_SECONDS,
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if !claimed {
        return Err(StatusCode::CONFLICT);
    }

    let dead_letter_stream = dead_letter_stream_name(&request.source_stream);
    let Some(payload) = state
        .queue
        .stream_entry_payload(&dead_letter_stream, &request.dead_letter_entry_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    else {
        let _ = state.queue.del_key(&action_lock_key).await;
        return Err(StatusCode::NOT_FOUND);
    };

    let delivery: DeadLetterDeliveryPayload =
        serde_json::from_str(&payload).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if delivery.source_stream != request.source_stream {
        let _ = state.queue.del_key(&action_lock_key).await;
        return Err(StatusCode::BAD_REQUEST);
    }

    if let Err(error) = state
        .queue
        .publish(&delivery.source_stream, &delivery.payload)
        .await
    {
        let _ = state.queue.del_key(&action_lock_key).await;
        return Err(match error {
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        });
    }
    if let Err(error) = state
        .queue
        .delete_stream_entry(&dead_letter_stream, &request.dead_letter_entry_id)
        .await
    {
        let _ = state.queue.del_key(&action_lock_key).await;
        return Err(match error {
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        });
    }
    let _ = state.queue.del_key(&delivery.retry_key).await;
    if let Err(error) = sqlx::query(
        r#"
        INSERT INTO dead_letter_replay_audit (
            replay_id,
            guild_id,
            dead_letter_entry_id,
            source_stream,
            delivery_id,
            attempts,
            replayed_by_user_id,
            replayed_by_display_name,
            operator_reason,
            error,
            retry_key,
            replayed_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(&guild_id)
    .bind(&request.dead_letter_entry_id)
    .bind(&delivery.source_stream)
    .bind(&delivery.delivery_id)
    .bind(delivery.attempts)
    .bind(Some(user.discord_user_id.clone()))
    .bind(&user.display_name)
    .bind(operator_reason.as_deref())
    .bind(&delivery.error)
    .bind(&delivery.retry_key)
    .bind(Utc::now())
    .execute(&state.pool)
    .await
    {
        let _ = state.queue.del_key(&action_lock_key).await;
        return Err(match error {
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        });
    }
    invalidate_dashboard_ops_cache(state.as_ref(), &guild_id).await;
    let _ = state.queue.del_key(&action_lock_key).await;

    let response = ReplayPipelineIncidentResponse {
        dead_letter_entry_id: request.dead_letter_entry_id,
        delivery_id: delivery.delivery_id,
        source_stream: delivery.source_stream,
        status: "replayed",
    };
    observe_api_request(
        state.metrics,
        "replay_dashboard_pipeline_incident",
        StatusCode::OK,
        request_started.elapsed().as_secs_f64(),
        estimate_json_size(&response),
    );

    Ok(Json(response))
}

async fn discard_dashboard_pipeline_incident(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(guild_id): Path<String>,
    Json(request): Json<DiscardPipelineIncidentRequest>,
) -> Result<Json<DiscardPipelineIncidentResponse>, StatusCode> {
    let request_started = Instant::now();
    let session_id = read_session_id(&headers).ok_or(StatusCode::UNAUTHORIZED)?;
    ensure_guild_access(&state.pool, session_id, &guild_id).await?;
    let user = fetch_dashboard_user(&state.pool, session_id)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    let operator_reason = normalize_operator_reason(request.operator_reason.as_deref());

    if !is_dashboard_stream(&request.source_stream) {
        return Err(StatusCode::BAD_REQUEST);
    }

    let action_lock_key =
        dead_letter_action_lock_key(&request.source_stream, &request.dead_letter_entry_id);
    let claimed = state
        .queue
        .claim_key_with_ttl(
            &action_lock_key,
            &user.discord_user_id,
            DEAD_LETTER_ACTION_LOCK_TTL_SECONDS,
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if !claimed {
        return Err(StatusCode::CONFLICT);
    }

    let dead_letter_stream = dead_letter_stream_name(&request.source_stream);
    let Some(payload) = state
        .queue
        .stream_entry_payload(&dead_letter_stream, &request.dead_letter_entry_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    else {
        let _ = state.queue.del_key(&action_lock_key).await;
        return Err(StatusCode::NOT_FOUND);
    };

    let delivery: DeadLetterDeliveryPayload =
        serde_json::from_str(&payload).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if delivery.source_stream != request.source_stream {
        let _ = state.queue.del_key(&action_lock_key).await;
        return Err(StatusCode::BAD_REQUEST);
    }

    if let Err(error) = state
        .queue
        .delete_stream_entry(&dead_letter_stream, &request.dead_letter_entry_id)
        .await
    {
        let _ = state.queue.del_key(&action_lock_key).await;
        return Err(match error {
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        });
    }
    let _ = state.queue.del_key(&delivery.retry_key).await;
    if let Err(error) = sqlx::query(
        r#"
        INSERT INTO dead_letter_discard_audit (
            discard_id,
            guild_id,
            dead_letter_entry_id,
            source_stream,
            delivery_id,
            attempts,
            discarded_by_user_id,
            discarded_by_display_name,
            operator_reason,
            error,
            retry_key,
            discarded_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(&guild_id)
    .bind(&request.dead_letter_entry_id)
    .bind(&delivery.source_stream)
    .bind(&delivery.delivery_id)
    .bind(delivery.attempts)
    .bind(Some(user.discord_user_id.clone()))
    .bind(&user.display_name)
    .bind(operator_reason.as_deref())
    .bind(&delivery.error)
    .bind(&delivery.retry_key)
    .bind(Utc::now())
    .execute(&state.pool)
    .await
    {
        let _ = state.queue.del_key(&action_lock_key).await;
        return Err(match error {
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        });
    }
    invalidate_dashboard_ops_cache(state.as_ref(), &guild_id).await;
    let _ = state.queue.del_key(&action_lock_key).await;

    let response = DiscardPipelineIncidentResponse {
        dead_letter_entry_id: request.dead_letter_entry_id,
        delivery_id: delivery.delivery_id,
        source_stream: delivery.source_stream,
        status: "discarded",
    };
    observe_api_request(
        state.metrics,
        "discard_dashboard_pipeline_incident",
        StatusCode::OK,
        request_started.elapsed().as_secs_f64(),
        estimate_json_size(&response),
    );

    Ok(Json(response))
}

async fn dashboard_pipeline_replay_history(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(guild_id): Path<String>,
) -> Result<Json<DashboardPipelineReplayHistoryResponse>, StatusCode> {
    const RECENT_REPLAY_LIMIT: i64 = 10;

    let request_started = Instant::now();
    let session_id = read_session_id(&headers).ok_or(StatusCode::UNAUTHORIZED)?;
    ensure_guild_access(&state.pool, session_id, &guild_id).await?;

    let cache_key = dashboard_resource_cache_key(&guild_id, "pipeline_replays");
    if let Some(response) =
        read_dashboard_cache::<DashboardPipelineReplayHistoryResponse>(state.as_ref(), &cache_key)
            .await
    {
        observe_api_request(
            state.metrics,
            "dashboard_pipeline_replay_history",
            StatusCode::OK,
            request_started.elapsed().as_secs_f64(),
            estimate_json_size(&response),
        );
        return Ok(Json(response));
    }

    let query_started = Instant::now();
    let rows_result = sqlx::query_as::<_, PipelineReplayAuditRow>(
        r#"
        SELECT
            attempts,
            delivery_id,
            operator_reason,
            replayed_at,
            replayed_by_display_name,
            replayed_by_user_id,
            source_stream
        FROM dead_letter_replay_audit
        WHERE guild_id = $1
        ORDER BY replayed_at DESC
        LIMIT $2
        "#,
    )
    .bind(&guild_id)
    .bind(RECENT_REPLAY_LIMIT)
    .fetch_all(&state.pool)
    .await;
    observe_sql_query(
        state.metrics,
        "dashboard_pipeline_replay_history",
        status_label(rows_result.as_ref().map(|_| &())),
        query_started.elapsed().as_secs_f64(),
    );
    let rows = rows_result.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let response = DashboardPipelineReplayHistoryResponse {
        guild_id,
        replays: rows
            .into_iter()
            .map(|row| DashboardPipelineReplay {
                attempts: row.attempts,
                delivery_id: row.delivery_id,
                operator_reason: row.operator_reason,
                replayed_at: row.replayed_at.to_rfc3339(),
                replayed_by_label: row.replayed_by_display_name,
                replayed_by_user_id: row.replayed_by_user_id,
                source_stream_label: pipeline_stream_label(&row.source_stream).to_string(),
                source_stream: row.source_stream,
            })
            .collect(),
    };
    observe_api_request(
        state.metrics,
        "dashboard_pipeline_replay_history",
        StatusCode::OK,
        request_started.elapsed().as_secs_f64(),
        estimate_json_size(&response),
    );
    write_dashboard_cache(state.as_ref(), &cache_key, &response).await;

    Ok(Json(response))
}

async fn dashboard_pipeline_discard_history(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(guild_id): Path<String>,
) -> Result<Json<DashboardPipelineDiscardHistoryResponse>, StatusCode> {
    const RECENT_DISCARD_LIMIT: i64 = 10;

    let request_started = Instant::now();
    let session_id = read_session_id(&headers).ok_or(StatusCode::UNAUTHORIZED)?;
    ensure_guild_access(&state.pool, session_id, &guild_id).await?;

    let cache_key = dashboard_resource_cache_key(&guild_id, "pipeline_discards");
    if let Some(response) =
        read_dashboard_cache::<DashboardPipelineDiscardHistoryResponse>(state.as_ref(), &cache_key)
            .await
    {
        observe_api_request(
            state.metrics,
            "dashboard_pipeline_discard_history",
            StatusCode::OK,
            request_started.elapsed().as_secs_f64(),
            estimate_json_size(&response),
        );
        return Ok(Json(response));
    }

    let query_started = Instant::now();
    let rows_result = sqlx::query_as::<_, PipelineDiscardAuditRow>(
        r#"
        SELECT
            attempts,
            delivery_id,
            discarded_at,
            discarded_by_display_name,
            discarded_by_user_id,
            operator_reason,
            source_stream
        FROM dead_letter_discard_audit
        WHERE guild_id = $1
        ORDER BY discarded_at DESC
        LIMIT $2
        "#,
    )
    .bind(&guild_id)
    .bind(RECENT_DISCARD_LIMIT)
    .fetch_all(&state.pool)
    .await;
    observe_sql_query(
        state.metrics,
        "dashboard_pipeline_discard_history",
        status_label(rows_result.as_ref().map(|_| &())),
        query_started.elapsed().as_secs_f64(),
    );
    let rows = rows_result.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let response = DashboardPipelineDiscardHistoryResponse {
        guild_id,
        discards: rows
            .into_iter()
            .map(|row| DashboardPipelineDiscard {
                attempts: row.attempts,
                delivery_id: row.delivery_id,
                discarded_at: row.discarded_at.to_rfc3339(),
                discarded_by_label: row.discarded_by_display_name,
                discarded_by_user_id: row.discarded_by_user_id,
                operator_reason: row.operator_reason,
                source_stream_label: pipeline_stream_label(&row.source_stream).to_string(),
                source_stream: row.source_stream,
            })
            .collect(),
    };
    observe_api_request(
        state.metrics,
        "dashboard_pipeline_discard_history",
        StatusCode::OK,
        request_started.elapsed().as_secs_f64(),
        estimate_json_size(&response),
    );
    write_dashboard_cache(state.as_ref(), &cache_key, &response).await;

    Ok(Json(response))
}

async fn request_guild_backfill(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(guild_id): Path<String>,
    Query(query): Query<BackfillQuery>,
) -> Result<Json<BackfillRequestResponse>, StatusCode> {
    let request_started = Instant::now();
    let session_id = read_session_id(&headers).ok_or(StatusCode::UNAUTHORIZED)?;
    ensure_guild_access(&state.pool, session_id, &guild_id).await?;

    let user = fetch_dashboard_user(&state.pool, session_id)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    let days = sanitize_days(query.days);
    let end_at = Utc::now();
    let start_at = end_at
        .checked_sub_days(Days::new(days as u64))
        .unwrap_or_else(|| end_at - chrono::Duration::days(days as i64));
    let job = BackfillJob::new(
        guild_id.clone(),
        Some(user.discord_user_id.clone()),
        days,
        start_at,
        end_at,
        "manual",
    );

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
    .execute(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let payload = serde_json::to_string(&job).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    state
        .queue
        .publish(BACKFILL_STREAM, &payload)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let response = BackfillRequestResponse {
        job_id: job.job_id.to_string(),
        status: "queued",
    };
    observe_api_request(
        state.metrics,
        "request_guild_backfill",
        StatusCode::OK,
        request_started.elapsed().as_secs_f64(),
        estimate_json_size(&response),
    );

    Ok(Json(response))
}

fn build_cors(allowed_origin: &str) -> CorsLayer {
    let methods = [Method::GET, Method::POST];
    let headers = [header::CONTENT_TYPE, header::COOKIE];

    if allowed_origin == "*" {
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(methods)
            .allow_headers(headers)
    } else {
        let origin =
            HeaderValue::from_str(allowed_origin).unwrap_or_else(|_| HeaderValue::from_static("*"));
        CorsLayer::new()
            .allow_origin(origin)
            .allow_methods(methods)
            .allow_headers(headers)
            .allow_credentials(true)
    }
}

fn build_discord_authorize_url(
    settings: &Settings,
    callback_url: &str,
    scope: &str,
    flow: OAuthFlow,
    permissions: Option<&str>,
    response_type: Option<&str>,
) -> Result<String> {
    let mut url =
        reqwest::Url::parse(DISCORD_AUTHORIZE_URL).context("invalid discord oauth url")?;
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("client_id", &settings.discord_application_id.to_string());
        query.append_pair("redirect_uri", callback_url);
        query.append_pair("scope", scope);
        query.append_pair("state", flow.as_str());
        query.append_pair("prompt", "consent");

        if let Some(response_type) = response_type {
            query.append_pair("response_type", response_type);
        }

        if let Some(permissions) = permissions {
            query.append_pair("permissions", permissions);
            query.append_pair("integration_type", "0");
        }
    }

    Ok(url.to_string())
}

fn build_install_authorize_url(settings: &Settings) -> Result<String> {
    build_discord_authorize_url(
        settings,
        &oauth_callback_url(settings),
        "bot applications.commands",
        OAuthFlow::InviteInstall,
        Some(DEFAULT_DISCORD_BOT_PERMISSIONS),
        None,
    )
}

fn build_login_authorize_url(settings: &Settings, flow: OAuthFlow) -> Result<String> {
    build_discord_authorize_url(
        settings,
        &oauth_callback_url(settings),
        "identify guilds",
        flow,
        None,
        Some("code"),
    )
}

fn dashboard_redirect_url(site_url: &str, query_pairs: &[(&str, &str)]) -> String {
    let mut url = reqwest::Url::parse(&format!("{}/dashboard", site_url.trim_end_matches('/')))
        .expect("PUBLIC_SITE_URL must be a valid URL");
    {
        let mut query = url.query_pairs_mut();
        for (key, value) in query_pairs {
            query.append_pair(key, value);
        }
    }
    url.to_string()
}

fn redirect_response(target: &str, cookie: Option<String>) -> Response {
    let mut response = Redirect::temporary(target).into_response();
    if let Some(cookie) = cookie {
        if let Ok(value) = HeaderValue::from_str(&cookie) {
            response.headers_mut().append(header::SET_COOKIE, value);
        }
    }
    response
}

async fn ensure_guild_access(
    pool: &PgPool,
    session_id: Uuid,
    guild_id: &str,
) -> Result<(), StatusCode> {
    let has_access = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM dashboard_session_guilds AS dsg
            INNER JOIN guild_inventory AS gi
                ON gi.guild_id = dsg.guild_id
            WHERE dsg.session_id = $1
              AND dsg.guild_id = $2
              AND gi.is_active = TRUE
              AND (dsg.is_owner = TRUE OR dsg.has_admin = TRUE)
        )
        "#,
    )
    .bind(session_id)
    .bind(guild_id)
    .fetch_one(pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if has_access {
        Ok(())
    } else {
        Err(StatusCode::FORBIDDEN)
    }
}

async fn ensure_public_schema(pool: &PgPool) -> Result<()> {
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
        CREATE TABLE IF NOT EXISTS member_inventory (
            guild_id TEXT NOT NULL,
            member_id TEXT NOT NULL,
            username TEXT NOT NULL,
            global_name TEXT NULL,
            nickname TEXT NULL,
            last_synced_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            PRIMARY KEY (guild_id, member_id)
        );
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create member_inventory")?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_member_inventory_guild_username
            ON member_inventory (guild_id, username ASC);
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create member_inventory index")?;

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
            FROM member_daily_activity
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
            FROM member_lifecycle
            WHERE joined_at IS NOT NULL
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
            FROM member_lifecycle
            WHERE left_at IS NOT NULL
            GROUP BY guild_id, DATE(left_at)
            UNION ALL
            SELECT
                guild_id,
                DATE(joined_at) AS summary_date,
                0::BIGINT AS messages,
                0::BIGINT AS active_members,
                0::BIGINT AS joined_members,
                0::BIGINT AS left_members,
                COUNT(*)::BIGINT AS onboarded_members,
                DATE(joined_at)::timestamp AT TIME ZONE 'UTC' AS last_message_at
            FROM member_lifecycle
            WHERE joined_at IS NOT NULL
              AND first_role_at IS NOT NULL
              AND first_message_at IS NOT NULL
            GROUP BY guild_id, DATE(joined_at)
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
                lifecycle.guild_id,
                DATE(lifecycle.joined_at) AS cohort_date,
                0::BIGINT AS joined_members,
                COUNT(DISTINCT lifecycle.member_id)::BIGINT AS d7_retained_members,
                0::BIGINT AS d30_retained_members
            FROM member_lifecycle AS lifecycle
            INNER JOIN member_daily_activity AS activity
                ON activity.guild_id = lifecycle.guild_id
               AND activity.member_id = lifecycle.member_id
            WHERE lifecycle.joined_at IS NOT NULL
              AND activity.activity_date >= DATE(lifecycle.joined_at) + 7
              AND activity.activity_date < DATE(lifecycle.joined_at) + 14
            GROUP BY lifecycle.guild_id, DATE(lifecycle.joined_at)
            UNION ALL
            SELECT
                lifecycle.guild_id,
                DATE(lifecycle.joined_at) AS cohort_date,
                0::BIGINT AS joined_members,
                0::BIGINT AS d7_retained_members,
                COUNT(DISTINCT lifecycle.member_id)::BIGINT AS d30_retained_members
            FROM member_lifecycle AS lifecycle
            INNER JOIN member_daily_activity AS activity
                ON activity.guild_id = lifecycle.guild_id
               AND activity.member_id = lifecycle.member_id
            WHERE lifecycle.joined_at IS NOT NULL
              AND activity.activity_date >= DATE(lifecycle.joined_at) + 30
              AND activity.activity_date < DATE(lifecycle.joined_at) + 37
            GROUP BY lifecycle.guild_id, DATE(lifecycle.joined_at)
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
                lifecycle.guild_id,
                message.channel_id,
                DATE(lifecycle.joined_at) AS cohort_date,
                COUNT(DISTINCT lifecycle.member_id)::BIGINT AS d7_retained_members,
                0::BIGINT AS d30_retained_members
            FROM member_lifecycle AS lifecycle
            INNER JOIN message_index AS message
                ON message.guild_id = lifecycle.guild_id
               AND message.author_id = lifecycle.member_id
            INNER JOIN member_daily_activity AS activity
                ON activity.guild_id = lifecycle.guild_id
               AND activity.member_id = lifecycle.member_id
            WHERE lifecycle.joined_at IS NOT NULL
              AND message.channel_id <> ''
              AND message.occurred_at >= lifecycle.joined_at
              AND message.occurred_at < lifecycle.joined_at + INTERVAL '7 days'
              AND activity.activity_date >= DATE(lifecycle.joined_at) + 7
              AND activity.activity_date < DATE(lifecycle.joined_at) + 14
            GROUP BY lifecycle.guild_id, message.channel_id, DATE(lifecycle.joined_at)
            UNION ALL
            SELECT
                lifecycle.guild_id,
                message.channel_id,
                DATE(lifecycle.joined_at) AS cohort_date,
                0::BIGINT AS d7_retained_members,
                COUNT(DISTINCT lifecycle.member_id)::BIGINT AS d30_retained_members
            FROM member_lifecycle AS lifecycle
            INNER JOIN message_index AS message
                ON message.guild_id = lifecycle.guild_id
               AND message.author_id = lifecycle.member_id
            INNER JOIN member_daily_activity AS activity
                ON activity.guild_id = lifecycle.guild_id
               AND activity.member_id = lifecycle.member_id
            WHERE lifecycle.joined_at IS NOT NULL
              AND message.channel_id <> ''
              AND message.occurred_at >= lifecycle.joined_at
              AND message.occurred_at < lifecycle.joined_at + INTERVAL '7 days'
              AND activity.activity_date >= DATE(lifecycle.joined_at) + 30
              AND activity.activity_date < DATE(lifecycle.joined_at) + 37
            GROUP BY lifecycle.guild_id, message.channel_id, DATE(lifecycle.joined_at)
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
                DATE(joined_at) AS cohort_date,
                0::BIGINT AS joined_members,
                COUNT(*)::BIGINT AS got_role_members,
                0::BIGINT AS first_message_members,
                0::BIGINT AS first_reaction_members,
                0::BIGINT AS first_voice_members,
                0::BIGINT AS returned_next_week_members
            FROM member_lifecycle
            WHERE joined_at IS NOT NULL
              AND first_role_at IS NOT NULL
              AND first_role_at >= joined_at
            GROUP BY guild_id, DATE(joined_at)
            UNION ALL
            SELECT
                guild_id,
                DATE(joined_at) AS cohort_date,
                0::BIGINT AS joined_members,
                0::BIGINT AS got_role_members,
                COUNT(*)::BIGINT AS first_message_members,
                0::BIGINT AS first_reaction_members,
                0::BIGINT AS first_voice_members,
                0::BIGINT AS returned_next_week_members
            FROM member_lifecycle
            WHERE joined_at IS NOT NULL
              AND first_message_at IS NOT NULL
              AND first_message_at >= joined_at
            GROUP BY guild_id, DATE(joined_at)
            UNION ALL
            SELECT
                guild_id,
                DATE(joined_at) AS cohort_date,
                0::BIGINT AS joined_members,
                0::BIGINT AS got_role_members,
                0::BIGINT AS first_message_members,
                COUNT(*)::BIGINT AS first_reaction_members,
                0::BIGINT AS first_voice_members,
                0::BIGINT AS returned_next_week_members
            FROM member_lifecycle
            WHERE joined_at IS NOT NULL
              AND first_reaction_at IS NOT NULL
              AND first_reaction_at >= joined_at
            GROUP BY guild_id, DATE(joined_at)
            UNION ALL
            SELECT
                guild_id,
                DATE(joined_at) AS cohort_date,
                0::BIGINT AS joined_members,
                0::BIGINT AS got_role_members,
                0::BIGINT AS first_message_members,
                0::BIGINT AS first_reaction_members,
                COUNT(*)::BIGINT AS first_voice_members,
                0::BIGINT AS returned_next_week_members
            FROM member_lifecycle
            WHERE joined_at IS NOT NULL
              AND first_voice_at IS NOT NULL
              AND first_voice_at >= joined_at
            GROUP BY guild_id, DATE(joined_at)
            UNION ALL
            SELECT
                lifecycle.guild_id,
                DATE(lifecycle.joined_at) AS cohort_date,
                0::BIGINT AS joined_members,
                0::BIGINT AS got_role_members,
                0::BIGINT AS first_message_members,
                0::BIGINT AS first_reaction_members,
                0::BIGINT AS first_voice_members,
                COUNT(DISTINCT lifecycle.member_id)::BIGINT AS returned_next_week_members
            FROM member_lifecycle AS lifecycle
            INNER JOIN member_daily_activity AS activity
                ON activity.guild_id = lifecycle.guild_id
               AND activity.member_id = lifecycle.member_id
            WHERE lifecycle.joined_at IS NOT NULL
              AND activity.activity_date >= DATE(lifecycle.joined_at) + 7
              AND activity.activity_date < DATE(lifecycle.joined_at) + 14
            GROUP BY lifecycle.guild_id, DATE(lifecycle.joined_at)
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

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS dashboard_sessions (
            session_id UUID PRIMARY KEY,
            discord_user_id TEXT NOT NULL,
            username TEXT NOT NULL,
            global_name TEXT NULL,
            avatar TEXT NULL,
            expires_at TIMESTAMPTZ NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            last_seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        );
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create dashboard_sessions")?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS dashboard_session_guilds (
            session_id UUID NOT NULL,
            guild_id TEXT NOT NULL,
            guild_name TEXT NOT NULL,
            icon TEXT NULL,
            is_owner BOOLEAN NOT NULL DEFAULT FALSE,
            has_admin BOOLEAN NOT NULL DEFAULT FALSE,
            permissions_text TEXT NOT NULL,
            PRIMARY KEY (session_id, guild_id)
        );
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create dashboard_session_guilds")?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS dead_letter_replay_audit (
            replay_id UUID PRIMARY KEY,
            guild_id TEXT NOT NULL,
            dead_letter_entry_id TEXT NOT NULL,
            source_stream TEXT NOT NULL,
            delivery_id TEXT NOT NULL,
            attempts BIGINT NOT NULL,
            replayed_by_user_id TEXT NULL,
            replayed_by_display_name TEXT NOT NULL,
            operator_reason TEXT NULL,
            error TEXT NOT NULL,
            retry_key TEXT NOT NULL,
            replayed_at TIMESTAMPTZ NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create dead_letter_replay_audit")?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_dead_letter_replay_audit_guild_replayed_at
            ON dead_letter_replay_audit (guild_id, replayed_at DESC);
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create dead_letter_replay_audit index")?;

    sqlx::query(
        r#"
        ALTER TABLE dead_letter_replay_audit
        ADD COLUMN IF NOT EXISTS operator_reason TEXT NULL;
        "#,
    )
    .execute(pool)
    .await
    .context("failed to alter dead_letter_replay_audit operator_reason")?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS dead_letter_discard_audit (
            discard_id UUID PRIMARY KEY,
            guild_id TEXT NOT NULL,
            dead_letter_entry_id TEXT NOT NULL,
            source_stream TEXT NOT NULL,
            delivery_id TEXT NOT NULL,
            attempts BIGINT NOT NULL,
            discarded_by_user_id TEXT NULL,
            discarded_by_display_name TEXT NOT NULL,
            operator_reason TEXT NULL,
            error TEXT NOT NULL,
            retry_key TEXT NOT NULL,
            discarded_at TIMESTAMPTZ NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create dead_letter_discard_audit")?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_dead_letter_discard_audit_guild_discarded_at
            ON dead_letter_discard_audit (guild_id, discarded_at DESC);
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create dead_letter_discard_audit index")?;

    sqlx::query(
        r#"
        ALTER TABLE dead_letter_discard_audit
        ADD COLUMN IF NOT EXISTS operator_reason TEXT NULL;
        "#,
    )
    .execute(pool)
    .await
    .context("failed to alter dead_letter_discard_audit operator_reason")?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS waitlist_entries (
            id UUID PRIMARY KEY,
            kind TEXT NOT NULL,
            discord_user_id TEXT NULL,
            discord_username TEXT NULL,
            discord_display_name TEXT NULL,
            email TEXT NULL,
            name TEXT NULL,
            company TEXT NULL,
            source TEXT NULL,
            use_case TEXT NULL,
            message TEXT NULL,
            notes TEXT NULL,
            ip TEXT NULL,
            user_agent TEXT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        );
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create waitlist_entries")?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS waitlist_entries_kind_created_at_idx
        ON waitlist_entries (kind, created_at DESC);
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create waitlist_entries index")?;

    sqlx::query(
        r#"
        CREATE UNIQUE INDEX IF NOT EXISTS waitlist_entries_discord_user_id_idx
        ON waitlist_entries (discord_user_id)
        WHERE discord_user_id IS NOT NULL AND kind = 'waitlist';
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create waitlist_entries discord uniq index")?;

    Ok(())
}

async fn exchange_oauth_code(state: &AppState, code: &str) -> Result<DiscordTokenResponse> {
    let callback_url = oauth_callback_url(&state.settings);
    let response = state
        .http_client
        .post(DISCORD_TOKEN_URL)
        .form(&[
            (
                "client_id",
                state.settings.discord_application_id.to_string(),
            ),
            (
                "client_secret",
                state.settings.discord_client_secret.clone(),
            ),
            ("grant_type", "authorization_code".to_string()),
            ("code", code.to_string()),
            ("redirect_uri", callback_url),
        ])
        .send()
        .await
        .context("failed to call discord oauth token endpoint")?;

    let status = response.status();
    if !status.is_success() {
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "failed to read response body".to_string());
        anyhow::bail!("discord oauth token exchange failed with {status}: {body}");
    }

    let token = response
        .json::<DiscordTokenResponse>()
        .await
        .context("failed to parse discord oauth token response")?;
    if token.access_token.is_empty() || token.token_type.is_empty() {
        anyhow::bail!("discord oauth token response was missing token data");
    }

    Ok(token)
}

async fn fetch_discord_user(state: &AppState, access_token: &str) -> Result<DiscordCurrentUser> {
    let response = state
        .http_client
        .get(DISCORD_USERS_ME_URL)
        .bearer_auth(access_token)
        .send()
        .await
        .context("failed to request current discord user")?;

    let status = response.status();
    if !status.is_success() {
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "failed to read response body".to_string());
        anyhow::bail!("discord current user request failed with {status}: {body}");
    }

    response
        .json::<DiscordCurrentUser>()
        .await
        .context("failed to decode current discord user")
}

async fn fetch_discord_user_guilds(
    state: &AppState,
    access_token: &str,
) -> Result<Vec<DiscordCurrentUserGuild>> {
    let response = state
        .http_client
        .get(DISCORD_USERS_ME_GUILDS_URL)
        .bearer_auth(access_token)
        .send()
        .await
        .context("failed to request current discord user guilds")?;

    let status = response.status();
    if !status.is_success() {
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "failed to read response body".to_string());
        anyhow::bail!("discord current user guilds request failed with {status}: {body}");
    }

    response
        .json::<Vec<DiscordCurrentUserGuild>>()
        .await
        .context("failed to decode current discord user guilds")
}

async fn resolve_channel_names(
    state: &AppState,
    guild_id: &str,
    channel_ids: &[String],
) -> HashMap<String, String> {
    if channel_ids.is_empty() {
        return HashMap::new();
    }

    let mut names = match fetch_channel_inventory_map(&state.pool, guild_id).await {
        Ok(names) => names,
        Err(error) => {
            warn!(guild_id, ?error, "failed to load channel inventory");
            HashMap::new()
        }
    };

    if channel_ids
        .iter()
        .any(|channel_id| !names.contains_key(channel_id))
    {
        if let Err(error) = sync_channel_inventory_from_discord(state, guild_id).await {
            warn!(
                guild_id,
                ?error,
                "failed to sync channel inventory from discord"
            );
        } else if let Ok(refreshed) = fetch_channel_inventory_map(&state.pool, guild_id).await {
            names = refreshed;
        }
    }

    let missing_channel_ids = channel_ids
        .iter()
        .filter(|channel_id| !names.contains_key(*channel_id))
        .cloned()
        .collect::<Vec<_>>();
    if !missing_channel_ids.is_empty() {
        if let Err(error) =
            sync_missing_channel_inventory_from_discord(state, guild_id, &missing_channel_ids).await
        {
            warn!(
                guild_id,
                ?error,
                "failed to sync missing channels from discord"
            );
        } else if let Ok(refreshed) = fetch_channel_inventory_map(&state.pool, guild_id).await {
            names = refreshed;
        }
    }

    names
}

async fn resolve_member_labels(
    state: &AppState,
    guild_id: &str,
    member_ids: &[String],
) -> HashMap<String, MemberDirectoryEntry> {
    if member_ids.is_empty() {
        return HashMap::new();
    }

    let mut members = match fetch_member_inventory_map(&state.pool, guild_id).await {
        Ok(members) => members,
        Err(error) => {
            warn!(guild_id, ?error, "failed to load member inventory");
            HashMap::new()
        }
    };

    let missing_member_ids = member_ids
        .iter()
        .filter(|member_id| !members.contains_key(*member_id))
        .cloned()
        .collect::<Vec<_>>();
    if !missing_member_ids.is_empty() {
        if let Err(error) =
            sync_missing_member_inventory_from_discord(state, guild_id, &missing_member_ids).await
        {
            warn!(
                guild_id,
                ?error,
                "failed to sync member inventory from discord"
            );
        } else if let Ok(refreshed) = fetch_member_inventory_map(&state.pool, guild_id).await {
            members = refreshed;
        }
    }

    members
}

async fn fetch_member_inventory_map(
    pool: &PgPool,
    guild_id: &str,
) -> Result<HashMap<String, MemberDirectoryEntry>> {
    let rows = sqlx::query_as::<_, MemberInventoryRow>(
        r#"
        SELECT member_id, username, global_name, nickname
        FROM member_inventory
        WHERE guild_id = $1
        "#,
    )
    .bind(guild_id)
    .fetch_all(pool)
    .await
    .context("failed to fetch member inventory")?;

    Ok(rows
        .into_iter()
        .map(|row| {
            (
                row.member_id,
                MemberDirectoryEntry {
                    username: row.username,
                    global_name: row.global_name,
                    nickname: row.nickname,
                },
            )
        })
        .collect())
}

async fn sync_missing_member_inventory_from_discord(
    state: &AppState,
    guild_id: &str,
    member_ids: &[String],
) -> Result<()> {
    for member_id in member_ids {
        let url = discord_api_url(
            &state.settings.discord_api_base_url,
            &format!("/guilds/{guild_id}/members/{member_id}"),
        )?;
        let response = state
            .http_client
            .get(url)
            .header(
                header::AUTHORIZATION,
                format!("Bot {}", state.settings.discord_token),
            )
            .send()
            .await
            .with_context(|| format!("failed to request member {member_id} from discord"))?;

        if !response.status().is_success() {
            continue;
        }

        let member = response
            .json::<DiscordGuildMember>()
            .await
            .with_context(|| format!("failed to decode discord member {member_id}"))?;
        upsert_member_inventory_entry(&state.pool, guild_id, &member).await?;
    }

    Ok(())
}

async fn fetch_channel_inventory_map(
    pool: &PgPool,
    guild_id: &str,
) -> Result<HashMap<String, String>> {
    let rows = sqlx::query_as::<_, ChannelInventoryRow>(
        r#"
        SELECT channel_id, channel_name
        FROM channel_inventory
        WHERE guild_id = $1
        "#,
    )
    .bind(guild_id)
    .fetch_all(pool)
    .await
    .context("failed to fetch channel inventory")?;

    Ok(rows
        .into_iter()
        .map(|row| (row.channel_id, row.channel_name))
        .collect())
}

async fn sync_channel_inventory_from_discord(state: &AppState, guild_id: &str) -> Result<()> {
    let url = discord_api_url(
        &state.settings.discord_api_base_url,
        &format!("/guilds/{guild_id}/channels"),
    )?;
    let response = state
        .http_client
        .get(url)
        .header(
            header::AUTHORIZATION,
            format!("Bot {}", state.settings.discord_token),
        )
        .send()
        .await
        .context("failed to request guild channels from discord")?;

    let status = response.status();
    if !status.is_success() {
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "failed to read response body".to_string());
        anyhow::bail!("discord guild channels request failed with {status}: {body}");
    }

    let channels = response
        .json::<Vec<DiscordGuildChannel>>()
        .await
        .context("failed to decode discord guild channels")?;

    for channel in channels {
        upsert_channel_inventory_entry(&state.pool, guild_id, &channel).await?;
    }

    Ok(())
}

async fn sync_missing_channel_inventory_from_discord(
    state: &AppState,
    guild_id: &str,
    channel_ids: &[String],
) -> Result<()> {
    for channel_id in channel_ids {
        let url = discord_api_url(
            &state.settings.discord_api_base_url,
            &format!("/channels/{channel_id}"),
        )?;
        let response = state
            .http_client
            .get(url)
            .header(
                header::AUTHORIZATION,
                format!("Bot {}", state.settings.discord_token),
            )
            .send()
            .await
            .with_context(|| format!("failed to request channel {channel_id} from discord"))?;

        if !response.status().is_success() {
            continue;
        }

        let channel = response
            .json::<DiscordGuildChannel>()
            .await
            .with_context(|| format!("failed to decode discord channel {channel_id}"))?;
        upsert_channel_inventory_entry(&state.pool, guild_id, &channel).await?;
    }

    Ok(())
}

async fn upsert_channel_inventory_entry(
    pool: &PgPool,
    guild_id: &str,
    channel: &DiscordGuildChannel,
) -> Result<()> {
    let Some(channel_name) = channel.name.as_deref().map(str::trim) else {
        return Ok(());
    };
    if channel_name.is_empty() {
        return Ok(());
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

    Ok(())
}

async fn upsert_member_inventory_entry(
    pool: &PgPool,
    guild_id: &str,
    member: &DiscordGuildMember,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO member_inventory (
            guild_id,
            member_id,
            username,
            global_name,
            nickname,
            last_synced_at
        )
        VALUES ($1, $2, $3, $4, $5, NOW())
        ON CONFLICT (guild_id, member_id) DO UPDATE
        SET username = EXCLUDED.username,
            global_name = EXCLUDED.global_name,
            nickname = EXCLUDED.nickname,
            last_synced_at = NOW()
        "#,
    )
    .bind(guild_id)
    .bind(&member.user.id)
    .bind(&member.user.username)
    .bind(&member.user.global_name)
    .bind(&member.nick)
    .execute(pool)
    .await
    .with_context(|| format!("failed to upsert member inventory for {}", member.user.id))?;

    Ok(())
}

async fn fetch_dashboard_user(pool: &PgPool, session_id: Uuid) -> Result<DashboardUser> {
    let row = sqlx::query_as::<_, DashboardUserRow>(
        r#"
        SELECT discord_user_id, username, global_name
        FROM dashboard_sessions
        WHERE session_id = $1
          AND expires_at > NOW()
        "#,
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await
    .context("failed to fetch dashboard session user")?
    .context("dashboard session not found")?;

    sqlx::query(
        r#"
        UPDATE dashboard_sessions
        SET last_seen_at = NOW()
        WHERE session_id = $1
        "#,
    )
    .bind(session_id)
    .execute(pool)
    .await
    .context("failed to update dashboard session last_seen_at")?;

    Ok(DashboardUser {
        display_name: row.global_name.unwrap_or_else(|| row.username.clone()),
        discord_user_id: row.discord_user_id,
        username: row.username,
    })
}

async fn fetch_accessible_guilds(pool: &PgPool, session_id: Uuid) -> Result<Vec<AccessibleGuild>> {
    let rows = sqlx::query_as::<_, AccessibleGuildRow>(
        r#"
        SELECT
            gi.guild_id,
            gi.guild_name,
            dsg.is_owner,
            gi.member_count
        FROM dashboard_session_guilds AS dsg
        INNER JOIN guild_inventory AS gi
            ON gi.guild_id = dsg.guild_id
        WHERE dsg.session_id = $1
          AND gi.is_active = TRUE
          AND (dsg.is_owner = TRUE OR dsg.has_admin = TRUE)
        ORDER BY gi.guild_name ASC
        "#,
    )
    .bind(session_id)
    .fetch_all(pool)
    .await
    .context("failed to fetch accessible guilds")?;

    Ok(rows
        .into_iter()
        .map(|row| AccessibleGuild {
            guild_id: row.guild_id,
            guild_name: row.guild_name,
            is_owner: row.is_owner,
            member_count: row.member_count,
        })
        .collect())
}

async fn persist_session(
    state: &AppState,
    user: &DiscordCurrentUser,
    guilds: &[DiscordCurrentUserGuild],
) -> Result<SessionPersistResult> {
    let session_id = Uuid::new_v4();
    let expires_at = Utc::now() + chrono::Duration::seconds(SESSION_TTL_SECONDS);
    let mut transaction = state
        .pool
        .begin()
        .await
        .context("failed to begin transaction")?;

    sqlx::query(
        r#"
        INSERT INTO dashboard_sessions (
            session_id,
            discord_user_id,
            username,
            global_name,
            avatar,
            expires_at
        )
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(session_id)
    .bind(&user.id)
    .bind(&user.username)
    .bind(&user.global_name)
    .bind(&user.avatar)
    .bind(expires_at)
    .execute(&mut *transaction)
    .await
    .context("failed to insert dashboard session")?;

    for guild in guilds {
        sqlx::query(
            r#"
            INSERT INTO dashboard_session_guilds (
                session_id,
                guild_id,
                guild_name,
                icon,
                is_owner,
                has_admin,
                permissions_text
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(session_id)
        .bind(&guild.id)
        .bind(&guild.name)
        .bind(&guild.icon)
        .bind(guild.owner)
        .bind(has_administrator_permission(&guild.permissions))
        .bind(&guild.permissions)
        .execute(&mut *transaction)
        .await
        .with_context(|| format!("failed to persist session guild {}", guild.id))?;
    }

    let accessible_guilds = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)::BIGINT
        FROM dashboard_session_guilds AS dsg
        INNER JOIN guild_inventory AS gi
            ON gi.guild_id = dsg.guild_id
        WHERE dsg.session_id = $1
          AND gi.is_active = TRUE
          AND (dsg.is_owner = TRUE OR dsg.has_admin = TRUE)
        "#,
    )
    .bind(session_id)
    .fetch_one(&mut *transaction)
    .await
    .context("failed to count accessible guilds")?;

    transaction
        .commit()
        .await
        .context("failed to commit session transaction")?;

    Ok(SessionPersistResult {
        accessible_guilds,
        session_id,
    })
}

fn build_session_cookie(settings: &Settings, session_id: Uuid) -> Result<String> {
    let mut parts = vec![
        format!("{SESSION_COOKIE_NAME}={session_id}"),
        "Path=/".to_string(),
        "HttpOnly".to_string(),
        "SameSite=Lax".to_string(),
        format!("Max-Age={SESSION_TTL_SECONDS}"),
    ];

    if let Some(domain) = cookie_domain(settings) {
        parts.push(format!("Domain={domain}"));
    }

    if is_secure_cookie(settings) {
        parts.push("Secure".to_string());
    }

    Ok(parts.join("; "))
}

fn cookie_domain(settings: &Settings) -> Option<String> {
    let site_url = reqwest::Url::parse(&settings.public_site_url).ok()?;
    let host = site_url.host_str()?;
    if host == "localhost" || host.parse::<std::net::IpAddr>().is_ok() {
        None
    } else {
        Some(host.to_string())
    }
}

fn has_administrator_permission(permissions: &str) -> bool {
    permissions
        .parse::<u64>()
        .map(|value| (value & ADMINISTRATOR_PERMISSION) == ADMINISTRATOR_PERMISSION)
        .unwrap_or(false)
}

fn init_tracing(rust_log: &str) {
    let filter = EnvFilter::try_new(rust_log).unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

fn api_metrics() -> &'static ApiMetrics {
    static API_METRICS: OnceLock<ApiMetrics> = OnceLock::new();
    API_METRICS.get_or_init(|| {
        let registry = Registry::new_custom(Some("guildest".to_string()), None)
            .expect("failed to create api metrics registry");
        let request_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "api_request_duration_seconds",
                "HTTP handler duration in seconds",
            ),
            &["endpoint", "status"],
        )
        .expect("failed to create api request duration metric");
        let requests_total = IntCounterVec::new(
            prometheus::Opts::new("api_requests_total", "Total API requests observed"),
            &["endpoint", "status"],
        )
        .expect("failed to create api request counter");
        let response_size_bytes = HistogramVec::new(
            HistogramOpts::new(
                "api_response_size_bytes",
                "Approximate serialized JSON response size in bytes",
            ),
            &["endpoint"],
        )
        .expect("failed to create api response size metric");
        let sql_query_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "api_sql_query_duration_seconds",
                "Observed SQL query duration in seconds",
            ),
            &["query", "status"],
        )
        .expect("failed to create api sql query duration metric");

        registry
            .register(Box::new(request_duration_seconds.clone()))
            .expect("failed to register api request duration metric");
        registry
            .register(Box::new(requests_total.clone()))
            .expect("failed to register api request counter");
        registry
            .register(Box::new(response_size_bytes.clone()))
            .expect("failed to register api response size metric");
        registry
            .register(Box::new(sql_query_duration_seconds.clone()))
            .expect("failed to register api sql query duration metric");

        ApiMetrics {
            registry,
            request_duration_seconds,
            requests_total,
            response_size_bytes,
            sql_query_duration_seconds,
        }
    })
}

fn observe_api_request(
    metrics: &ApiMetrics,
    endpoint: &'static str,
    status: StatusCode,
    duration_seconds: f64,
    response_size_bytes: usize,
) {
    let status = status.as_u16().to_string();
    metrics
        .request_duration_seconds
        .with_label_values(&[endpoint, &status])
        .observe(duration_seconds);
    metrics
        .requests_total
        .with_label_values(&[endpoint, &status])
        .inc();
    metrics
        .response_size_bytes
        .with_label_values(&[endpoint])
        .observe(response_size_bytes as f64);
}

fn observe_sql_query(
    metrics: &ApiMetrics,
    query: &'static str,
    status: &'static str,
    duration_seconds: f64,
) {
    metrics
        .sql_query_duration_seconds
        .with_label_values(&[query, status])
        .observe(duration_seconds);
}

fn status_label<T, E>(result: Result<T, E>) -> &'static str {
    if result.is_ok() { "ok" } else { "error" }
}

fn estimate_json_size<T: Serialize>(value: &T) -> usize {
    serde_json::to_vec(value)
        .map(|body| body.len())
        .unwrap_or(0)
}

fn dashboard_resource_cache_key(guild_id: &str, resource: &str) -> String {
    format!("guild:{guild_id}:{resource}")
}

fn dashboard_cache_key(guild_id: &str, resource: &str, days: i32) -> String {
    format!("guild:{guild_id}:{resource}:{days}")
}

async fn read_dashboard_cache<T: DeserializeOwned>(state: &AppState, cache_key: &str) -> Option<T> {
    match state.queue.get_json(cache_key).await {
        Ok(value) => value,
        Err(error) => {
            warn!(cache_key, ?error, "failed to read dashboard cache");
            None
        }
    }
}

async fn write_dashboard_cache<T: Serialize>(state: &AppState, cache_key: &str, value: &T) {
    if let Err(error) = state
        .queue
        .set_json(cache_key, value, DASHBOARD_CACHE_TTL_SECONDS)
        .await
    {
        warn!(cache_key, ?error, "failed to write dashboard cache");
    }
}

async fn invalidate_dashboard_ops_cache(state: &AppState, guild_id: &str) {
    for cache_key in [
        dashboard_resource_cache_key(guild_id, "pipeline_health"),
        dashboard_resource_cache_key(guild_id, "pipeline_incidents"),
        dashboard_resource_cache_key(guild_id, "pipeline_discards"),
        dashboard_resource_cache_key(guild_id, "pipeline_replays"),
    ] {
        if let Err(error) = state.queue.del_key(&cache_key).await {
            warn!(
                cache_key,
                ?error,
                "failed to invalidate dashboard ops cache"
            );
        }
    }
}

fn short_member_label(member_id: &str) -> String {
    format!("@user-{}", short_id_suffix(member_id))
}

fn member_display_label(
    member_labels: &HashMap<String, MemberDirectoryEntry>,
    member_id: &str,
) -> String {
    member_labels
        .get(member_id)
        .map(|entry| {
            entry
                .nickname
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .or_else(|| {
                    entry
                        .global_name
                        .as_deref()
                        .filter(|value| !value.trim().is_empty())
                })
                .unwrap_or(entry.username.as_str())
                .to_string()
        })
        .unwrap_or_else(|| short_member_label(member_id))
}

fn member_secondary_label(
    member_labels: &HashMap<String, MemberDirectoryEntry>,
    member_id: &str,
) -> Option<String> {
    let entry = member_labels.get(member_id)?;
    let display = member_display_label(member_labels, member_id);
    if display == entry.username {
        None
    } else {
        Some(format!("@{}", entry.username))
    }
}

fn channel_label(channel_names: &HashMap<String, String>, channel_id: &str) -> String {
    channel_names
        .get(channel_id)
        .cloned()
        .unwrap_or_else(|| short_channel_label(channel_id))
}

fn short_channel_label(channel_id: &str) -> String {
    format!("#channel-{}", short_id_suffix(channel_id))
}

fn discord_api_url(base: &str, path: &str) -> Result<String> {
    let path = path.trim_start_matches('/');
    let base = base.trim_end_matches('/');
    Ok(format!("{base}/{path}"))
}

fn payload_preview(payload: &str) -> String {
    const MAX_PREVIEW_CHARS: usize = 140;

    let mut preview = String::new();
    let mut char_count = 0usize;
    let mut saw_whitespace = false;
    let mut truncated = false;

    for ch in payload.chars() {
        if ch.is_whitespace() {
            saw_whitespace = true;
            continue;
        }

        if saw_whitespace && !preview.is_empty() {
            if char_count == MAX_PREVIEW_CHARS {
                truncated = true;
                break;
            }
            preview.push(' ');
            char_count += 1;
        }
        saw_whitespace = false;

        if char_count == MAX_PREVIEW_CHARS {
            truncated = true;
            break;
        }

        preview.push(ch);
        char_count += 1;
    }

    if truncated {
        preview.push_str("...");
    }

    preview
}

fn normalize_operator_reason(reason: Option<&str>) -> Option<String> {
    const MAX_REASON_CHARS: usize = 280;

    let trimmed = reason?.trim();
    if trimmed.is_empty() {
        return None;
    }

    let normalized = trimmed.chars().take(MAX_REASON_CHARS).collect::<String>();
    Some(normalized)
}

fn dashboard_stream_names() -> [&'static str; 5] {
    [
        "events.guild",
        "events.member",
        "events.message",
        "events.voice",
        BACKFILL_STREAM,
    ]
}

fn is_dashboard_stream(stream: &str) -> bool {
    dashboard_stream_names().contains(&stream)
}

fn pipeline_stream_label(stream: &str) -> &'static str {
    match stream {
        "events.guild" => "Guild events",
        "events.member" => "Member events",
        "events.message" => "Message events",
        "events.voice" => "Voice events",
        BACKFILL_STREAM => "Backfill jobs",
        _ => "Unknown stream",
    }
}

fn pipeline_stream_status(
    ready_messages: i64,
    pending_messages: i64,
    scheduled_retry_messages: i64,
    dead_letter_messages: i64,
    oldest_ready_age_seconds: i64,
    scheduled_retry_overdue_seconds: i64,
) -> &'static str {
    if dead_letter_messages > 0
        || scheduled_retry_overdue_seconds > 0
        || oldest_ready_age_seconds >= 300
    {
        "critical"
    } else if scheduled_retry_messages > 0
        || pending_messages > 0
        || ready_messages > 0
        || oldest_ready_age_seconds >= 60
    {
        "warning"
    } else {
        "healthy"
    }
}

fn dead_letter_stream_name(stream: &str) -> String {
    format!("dead_letter.{stream}")
}

fn scheduled_retry_set_name(stream: &str) -> String {
    format!("scheduled_retry.{stream}")
}

fn dead_letter_action_lock_key(stream: &str, entry_id: &str) -> String {
    format!("dead_letter_action_lock:{stream}:{entry_id}")
}

fn short_id_suffix(id: &str) -> &str {
    let start = id.len().saturating_sub(6);
    &id[start..]
}

fn format_hour_label(hour_of_day: i32) -> String {
    format!("{:02}:00 UTC", hour_of_day.clamp(0, 23))
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

async fn fetch_public_stats_row(pool: &PgPool) -> Result<PublicStatsRow, sqlx::Error> {
    let cached = sqlx::query_as::<_, PublicStatsRow>(
        r#"
        SELECT messages_tracked, servers, members
        FROM public_stats_cache
        WHERE cache_key = 'global'
        "#,
    )
    .fetch_optional(pool)
    .await?;

    if let Some(row) = cached {
        Ok(row)
    } else {
        sqlx::query_as::<_, PublicStatsRow>(
            r#"
            SELECT
                COALESCE((
                    SELECT COUNT(*)
                    FROM message_index
                ), 0)::BIGINT AS messages_tracked,
                COALESCE((
                    SELECT COUNT(*)
                    FROM guild_inventory
                    WHERE is_active = TRUE
                ), 0)::BIGINT AS servers,
                COALESCE((
                    SELECT SUM(member_count)
                    FROM guild_inventory
                    WHERE is_active = TRUE
                ), 0)::BIGINT AS members
            "#,
        )
        .fetch_one(pool)
        .await
    }
}

fn is_secure_cookie(settings: &Settings) -> bool {
    settings.public_site_url.starts_with("https://")
}

fn oauth_callback_url(settings: &Settings) -> String {
    format!(
        "{}/v1/public/oauth/callback",
        settings.public_api_base_url.trim_end_matches('/')
    )
}

fn public_url(settings: &Settings, path: &str) -> String {
    format!(
        "{}{}",
        settings.public_api_base_url.trim_end_matches('/'),
        path
    )
}

async fn dashboard_ai_settings(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(guild_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let session_id = read_session_id(&headers).ok_or(StatusCode::UNAUTHORIZED)?;
    ensure_guild_access(&state.pool, session_id, &guild_id).await?;

    match state.ai_store.get_guild_settings(&guild_id).await {
        Ok(Some(settings)) => Ok(Json(serde_json::to_value(settings).unwrap_or_default())),
        Ok(None) => {
            // Return defaults — no row yet means AI is off with default config.
            let defaults = serde_json::json!({
                "guild_id": guild_id,
                "advisor_mode_enabled": true,
                "approval_required": true,
                "owner_dm_enabled": false,
                "live_pulse_enabled": true,
                "live_pulse_interval_minutes": 60,
                "real_time_alerts_enabled": true,
                "daily_briefing_enabled": true,
                "weekly_report_enabled": true,
                "retention_days": 30
            });
            Ok(Json(defaults))
        }
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn dashboard_ai_settings_update(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(guild_id): Path<String>,
    Json(update): Json<UpdateAiGuildSettings>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let session_id = read_session_id(&headers).ok_or(StatusCode::UNAUTHORIZED)?;
    ensure_guild_access(&state.pool, session_id, &guild_id).await?;

    match state
        .ai_store
        .upsert_guild_settings(&guild_id, &update)
        .await
    {
        Ok(settings) => Ok(Json(serde_json::to_value(settings).unwrap_or_default())),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

#[derive(Debug, Deserialize)]
struct LivePulseQuery {
    window_minutes: Option<i32>,
}

async fn dashboard_ai_live_pulse(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(guild_id): Path<String>,
    Query(query): Query<LivePulseQuery>,
) -> Result<Json<LivePulseResponse>, StatusCode> {
    let session_id = read_session_id(&headers).ok_or(StatusCode::UNAUTHORIZED)?;
    ensure_guild_access(&state.pool, session_id, &guild_id).await?;

    let window_minutes = query.window_minutes.unwrap_or(60).clamp(10, 1440);

    match state
        .ai_store
        .live_pulse_stats(&guild_id, window_minutes)
        .await
    {
        Ok(stats) => Ok(Json(stats)),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

fn read_session_id(headers: &HeaderMap) -> Option<Uuid> {
    let cookie_header = headers.get(header::COOKIE)?.to_str().ok()?;
    cookie_header
        .split(';')
        .find_map(|cookie| {
            let mut parts = cookie.trim().splitn(2, '=');
            let key = parts.next()?;
            let value = parts.next()?;
            (key == SESSION_COOKIE_NAME).then_some(value)
        })
        .and_then(|value| Uuid::parse_str(value).ok())
}

fn sanitize_days(days: Option<i32>) -> i32 {
    days.unwrap_or(7).clamp(1, 90)
}

fn sanitize_heatmap_days(days: Option<i32>) -> i32 {
    days.unwrap_or(365).clamp(1, 366)
}

#[derive(Clone, Copy)]
enum OAuthFlow {
    Login,
    InviteAuth,
    InviteInstall,
}

impl OAuthFlow {
    fn as_str(self) -> &'static str {
        match self {
            Self::Login => "login",
            Self::InviteAuth => "invite-auth",
            Self::InviteInstall => "invite-install",
        }
    }

    fn from_state(state: &str) -> Option<Self> {
        match state {
            "login" => Some(Self::Login),
            "invite-auth" => Some(Self::InviteAuth),
            "invite-install" => Some(Self::InviteInstall),
            _ => None,
        }
    }
}
