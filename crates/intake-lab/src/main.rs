use std::{
    alloc::{GlobalAlloc, Layout, System},
    borrow::Cow,
    env,
    hint::black_box,
    net::SocketAddr,
    str::FromStr,
    sync::{
        OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use axum::{Router, extract::State, response::IntoResponse, routing::get};
use chrono::{DateTime, Utc};
use prometheus::{
    Encoder, HistogramOpts, HistogramVec, IntCounterVec, IntGauge, IntGaugeVec, Registry,
    TextEncoder,
};
use serde::{
    Deserialize,
    de::{self, Deserializer, IgnoredAny, SeqAccess, Visitor},
};
use tokio::{signal, task, time::MissedTickBehavior};
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

#[global_allocator]
static TRACKING_ALLOCATOR: TrackingAllocator = TrackingAllocator::new();

static ALLOCATOR_STATS: AllocatorStats = AllocatorStats::new();
static METRICS: OnceLock<IntakeLabMetrics> = OnceLock::new();
const DISCORD_EPOCH_MS: i64 = 1_420_070_400_000;

#[derive(Clone, Copy, Debug)]
enum ParserMode {
    Thin,
    Owned,
}

impl ParserMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Thin => "thin",
            Self::Owned => "owned",
        }
    }
}

impl FromStr for ParserMode {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "thin" => Ok(Self::Thin),
            "owned" => Ok(Self::Owned),
            other => Err(anyhow::anyhow!(
                "unsupported INTAKE_LAB_PARSER_MODE {other}; expected thin or owned"
            )),
        }
    }
}

#[derive(Clone, Debug)]
struct Settings {
    bind_addr: SocketAddr,
    parser_mode: ParserMode,
    message_rate: u64,
    parse_duration_sample_rate: u64,
    worker_count: usize,
    tick_ms: u64,
    sample_count: usize,
    attachment_count: usize,
    content_bytes: usize,
    author_pool_size: usize,
    guild_count: usize,
    channels_per_guild: usize,
    shard_count: usize,
    runtime_seconds: Option<u64>,
    log_every_seconds: u64,
    rust_log: String,
}

impl Settings {
    fn from_env() -> Result<Self> {
        let worker_count = env_usize("INTAKE_LAB_WORKERS", 4)?.max(1);
        let tick_ms = env_u64("INTAKE_LAB_TICK_MS", 20)?.max(1);
        let sample_count = env_usize("INTAKE_LAB_SAMPLE_COUNT", 2048)?.max(1);
        let author_pool_size = env_usize("INTAKE_LAB_AUTHOR_POOL_SIZE", 25_000_000)?.max(1);
        let guild_count = env_usize("INTAKE_LAB_GUILD_COUNT", 1)?.max(1);
        let channels_per_guild = env_usize("INTAKE_LAB_CHANNELS_PER_GUILD", 64)?.max(1);
        let shard_count = env_usize("INTAKE_LAB_SHARD_COUNT", 1)?.max(1);

        Ok(Self {
            bind_addr: env::var("INTAKE_LAB_BIND_ADDR")
                .unwrap_or_else(|_| "127.0.0.1:29091".to_string())
                .parse()
                .context("invalid INTAKE_LAB_BIND_ADDR")?,
            parser_mode: env::var("INTAKE_LAB_PARSER_MODE")
                .unwrap_or_else(|_| "thin".to_string())
                .parse()?,
            message_rate: env_u64("INTAKE_LAB_MESSAGE_RATE", 500)?,
            parse_duration_sample_rate: env_u64("INTAKE_LAB_PARSE_DURATION_SAMPLE_RATE", 64)?
                .max(1),
            worker_count,
            tick_ms,
            sample_count,
            attachment_count: env_usize("INTAKE_LAB_ATTACHMENT_COUNT", 1)?,
            content_bytes: env_usize("INTAKE_LAB_CONTENT_BYTES", 96)?,
            author_pool_size,
            guild_count,
            channels_per_guild,
            shard_count,
            runtime_seconds: match env::var("INTAKE_LAB_RUNTIME_SECONDS") {
                Ok(value) if !value.is_empty() => Some(
                    value
                        .parse()
                        .context("invalid INTAKE_LAB_RUNTIME_SECONDS")?,
                ),
                _ => None,
            },
            log_every_seconds: env_u64("INTAKE_LAB_LOG_EVERY_SECONDS", 5)?.max(1),
            rust_log: env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()),
        })
    }
}

#[derive(Debug)]
struct IntakeLabMetrics {
    allocator_allocated_bytes: IntGauge,
    allocator_allocation_count: IntGauge,
    allocator_deallocated_bytes: IntGauge,
    allocator_live_bytes: IntGauge,
    allocator_peak_live_bytes: IntGauge,
    config: IntGaugeVec,
    input_bytes_total: IntCounterVec,
    messages_parsed_total: IntCounterVec,
    parse_duration_seconds: HistogramVec,
    parse_failures_total: IntCounterVec,
    registry: Registry,
}

#[derive(Debug)]
struct NormalizedMessage {
    attachment_count: u16,
    author_id: u64,
    channel_id: u64,
    content_length: u16,
    guild_id: u64,
    is_bot: bool,
    is_reply: bool,
    message_id: u64,
    occurred_at_unix_ms: i64,
}

#[derive(Debug, Deserialize)]
struct ThinDispatch<'a> {
    #[serde(borrow)]
    d: ThinMessage<'a>,
}

#[derive(Debug, Deserialize)]
struct ThinMessage<'a> {
    #[serde(deserialize_with = "deserialize_snowflake")]
    id: u64,
    #[serde(rename = "guild_id", deserialize_with = "deserialize_snowflake")]
    guild_id: u64,
    #[serde(rename = "channel_id", deserialize_with = "deserialize_snowflake")]
    channel_id: u64,
    author: ThinAuthor,
    #[serde(borrow)]
    content: Cow<'a, str>,
    #[serde(rename = "attachments", deserialize_with = "deserialize_array_len")]
    attachment_count: usize,
    #[serde(
        rename = "message_reference",
        default,
        deserialize_with = "deserialize_presence_flag"
    )]
    has_message_reference: bool,
    #[serde(
        rename = "referenced_message",
        default,
        deserialize_with = "deserialize_presence_flag"
    )]
    has_referenced_message: bool,
}

#[derive(Debug, Deserialize)]
struct ThinAuthor {
    #[serde(default)]
    bot: bool,
    #[serde(deserialize_with = "deserialize_snowflake")]
    id: u64,
}

#[derive(Debug, Deserialize)]
struct OwnedDispatch {
    d: OwnedMessage,
}

#[derive(Debug, Deserialize)]
struct OwnedMessage {
    id: String,
    guild_id: String,
    channel_id: String,
    author: OwnedAuthor,
    content: String,
    attachments: Vec<OwnedAttachment>,
    #[serde(default)]
    message_reference: Option<OwnedMessageReference>,
    #[serde(default)]
    referenced_message: Option<Box<OwnedReferencedMessage>>,
    timestamp: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct OwnedAuthor {
    #[serde(default)]
    bot: bool,
    id: String,
}

#[derive(Debug, Deserialize)]
struct OwnedAttachment {
    id: String,
    size: u64,
}

#[derive(Debug, Deserialize)]
struct OwnedMessageReference {
    channel_id: String,
    guild_id: String,
    message_id: String,
}

#[derive(Debug, Deserialize)]
struct OwnedReferencedMessage {
    id: String,
}

#[derive(Debug)]
struct AllocatorStats {
    allocated_bytes: AtomicU64,
    allocation_count: AtomicU64,
    deallocated_bytes: AtomicU64,
    live_bytes: AtomicU64,
    peak_live_bytes: AtomicU64,
}

impl AllocatorStats {
    const fn new() -> Self {
        Self {
            allocated_bytes: AtomicU64::new(0),
            allocation_count: AtomicU64::new(0),
            deallocated_bytes: AtomicU64::new(0),
            live_bytes: AtomicU64::new(0),
            peak_live_bytes: AtomicU64::new(0),
        }
    }

    fn record_alloc(&self, size: usize) {
        let size = size as u64;
        self.allocated_bytes.fetch_add(size, Ordering::Relaxed);
        self.allocation_count.fetch_add(1, Ordering::Relaxed);
        let live = self.live_bytes.fetch_add(size, Ordering::Relaxed) + size;
        let mut peak = self.peak_live_bytes.load(Ordering::Relaxed);
        while live > peak {
            match self.peak_live_bytes.compare_exchange_weak(
                peak,
                live,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(current) => peak = current,
            }
        }
    }

    fn record_dealloc(&self, size: usize) {
        let size = size as u64;
        self.deallocated_bytes.fetch_add(size, Ordering::Relaxed);
        self.live_bytes.fetch_sub(size, Ordering::Relaxed);
    }
}

struct TrackingAllocator;

impl TrackingAllocator {
    const fn new() -> Self {
        Self
    }
}

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            ALLOCATOR_STATS.record_alloc(layout.size());
        }
        ptr
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if !ptr.is_null() {
            ALLOCATOR_STATS.record_alloc(layout.size());
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
        ALLOCATOR_STATS.record_dealloc(layout.size());
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_ptr = unsafe { System.realloc(ptr, layout, new_size) };
        if !new_ptr.is_null() {
            let old_size = layout.size() as u64;
            let new_size = new_size as u64;
            if new_size >= old_size {
                ALLOCATOR_STATS.record_alloc((new_size - old_size) as usize);
            } else {
                ALLOCATOR_STATS.record_dealloc((old_size - new_size) as usize);
            }
        }
        new_ptr
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let settings = Settings::from_env()?;
    init_tracing(&settings.rust_log);
    let metrics = intake_lab_metrics();
    configure_metrics(metrics, &settings);

    let samples = std::sync::Arc::new(build_samples(&settings));
    info!(
        bind_addr = %settings.bind_addr,
        parser_mode = settings.parser_mode.as_str(),
        message_rate = settings.message_rate,
        parse_duration_sample_rate = settings.parse_duration_sample_rate,
        worker_count = settings.worker_count,
        tick_ms = settings.tick_ms,
        guild_count = settings.guild_count,
        shard_count = settings.shard_count,
        sample_count = settings.sample_count,
        "starting intake lab"
    );

    let metrics_addr = settings.bind_addr;
    let metrics_task = task::spawn(async move {
        if let Err(error) = run_metrics_server(metrics_addr).await {
            error!(?error, "metrics server exited");
        }
    });

    let deadline = settings
        .runtime_seconds
        .map(|seconds| Instant::now() + Duration::from_secs(seconds));
    let mut tasks = Vec::new();

    for worker_index in 0..settings.worker_count {
        let rate = assigned_rate(settings.message_rate, settings.worker_count, worker_index);
        if rate == 0 {
            continue;
        }

        let task_settings = settings.clone();
        let task_samples = samples.clone();
        tasks.push(task::spawn(async move {
            if let Err(error) =
                run_worker(worker_index, rate, task_settings, task_samples, deadline).await
            {
                error!(worker_index, ?error, "worker exited");
            }
        }));
    }

    let reporter_settings = settings.clone();
    tasks.push(task::spawn(async move {
        if let Err(error) = run_reporter(reporter_settings).await {
            error!(?error, "reporter exited");
        }
    }));

    wait_for_shutdown(deadline).await?;
    info!("intake lab stopping");

    for task in tasks {
        task.abort();
    }
    metrics_task.abort();

    Ok(())
}

fn init_tracing(rust_log: &str) {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(rust_log))
        .with_target(false)
        .compact()
        .init();
}

fn intake_lab_metrics() -> &'static IntakeLabMetrics {
    METRICS.get_or_init(|| {
        let registry = Registry::new();
        let allocator_allocated_bytes = IntGauge::new(
            "intake_lab_allocator_allocated_bytes",
            "Total bytes allocated by the lab process allocator",
        )
        .expect("failed to build allocator_allocated_bytes");
        let allocator_allocation_count = IntGauge::new(
            "intake_lab_allocator_allocation_count",
            "Total allocator calls observed by the lab process",
        )
        .expect("failed to build allocator_allocation_count");
        let allocator_deallocated_bytes = IntGauge::new(
            "intake_lab_allocator_deallocated_bytes",
            "Total bytes deallocated by the lab process allocator",
        )
        .expect("failed to build allocator_deallocated_bytes");
        let allocator_live_bytes = IntGauge::new(
            "intake_lab_allocator_live_bytes",
            "Approximate live bytes owned by the lab allocator",
        )
        .expect("failed to build allocator_live_bytes");
        let allocator_peak_live_bytes = IntGauge::new(
            "intake_lab_allocator_peak_live_bytes",
            "Peak live bytes observed by the lab allocator",
        )
        .expect("failed to build allocator_peak_live_bytes");
        let config = IntGaugeVec::new(
            prometheus::Opts::new(
                "intake_lab_config",
                "Intake lab configuration values keyed by parser mode",
            ),
            &["mode", "name"],
        )
        .expect("failed to build config gauge");
        let input_bytes_total = IntCounterVec::new(
            prometheus::Opts::new(
                "intake_lab_input_bytes_total",
                "Total input bytes submitted to the parser",
            ),
            &["mode"],
        )
        .expect("failed to build input_bytes_total");
        let messages_parsed_total = IntCounterVec::new(
            prometheus::Opts::new(
                "intake_lab_messages_parsed_total",
                "Total messages parsed successfully",
            ),
            &["mode"],
        )
        .expect("failed to build messages_parsed_total");
        let parse_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "intake_lab_parse_duration_seconds",
                "End-to-end parse and normalization latency per message",
            )
            .buckets(vec![
                0.000_01, 0.000_025, 0.000_05, 0.000_1, 0.000_25, 0.000_5, 0.001, 0.0025, 0.005,
                0.01,
            ]),
            &["mode"],
        )
        .expect("failed to build parse_duration_seconds");
        let parse_failures_total = IntCounterVec::new(
            prometheus::Opts::new(
                "intake_lab_parse_failures_total",
                "Total messages that failed to parse",
            ),
            &["mode"],
        )
        .expect("failed to build parse_failures_total");

        for collector in [
            Box::new(allocator_allocated_bytes.clone()) as Box<dyn prometheus::core::Collector>,
            Box::new(allocator_allocation_count.clone()),
            Box::new(allocator_deallocated_bytes.clone()),
            Box::new(allocator_live_bytes.clone()),
            Box::new(allocator_peak_live_bytes.clone()),
            Box::new(config.clone()),
            Box::new(input_bytes_total.clone()),
            Box::new(messages_parsed_total.clone()),
            Box::new(parse_duration_seconds.clone()),
            Box::new(parse_failures_total.clone()),
        ] {
            registry
                .register(collector)
                .expect("failed to register intake lab collector");
        }

        IntakeLabMetrics {
            allocator_allocated_bytes,
            allocator_allocation_count,
            allocator_deallocated_bytes,
            allocator_live_bytes,
            allocator_peak_live_bytes,
            config,
            input_bytes_total,
            messages_parsed_total,
            parse_duration_seconds,
            parse_failures_total,
            registry,
        }
    })
}

fn configure_metrics(metrics: &IntakeLabMetrics, settings: &Settings) {
    let mode = settings.parser_mode.as_str();
    metrics
        .config
        .with_label_values(&[mode, "message_rate"])
        .set(saturating_i64(settings.message_rate));
    metrics
        .config
        .with_label_values(&[mode, "worker_count"])
        .set(saturating_i64(settings.worker_count as u64));
    metrics
        .config
        .with_label_values(&[mode, "parse_duration_sample_rate"])
        .set(saturating_i64(settings.parse_duration_sample_rate));
    metrics
        .config
        .with_label_values(&[mode, "tick_ms"])
        .set(saturating_i64(settings.tick_ms));
    metrics
        .config
        .with_label_values(&[mode, "sample_count"])
        .set(saturating_i64(settings.sample_count as u64));
    metrics
        .config
        .with_label_values(&[mode, "attachment_count"])
        .set(saturating_i64(settings.attachment_count as u64));
    metrics
        .config
        .with_label_values(&[mode, "content_bytes"])
        .set(saturating_i64(settings.content_bytes as u64));
    metrics
        .config
        .with_label_values(&[mode, "author_pool_size"])
        .set(saturating_i64(settings.author_pool_size as u64));
    metrics
        .config
        .with_label_values(&[mode, "guild_count"])
        .set(saturating_i64(settings.guild_count as u64));
    metrics
        .config
        .with_label_values(&[mode, "channels_per_guild"])
        .set(saturating_i64(settings.channels_per_guild as u64));
    metrics
        .config
        .with_label_values(&[mode, "shard_count"])
        .set(saturating_i64(settings.shard_count as u64));
}

async fn run_metrics_server(addr: SocketAddr) -> Result<()> {
    let app = Router::new()
        .route("/metrics", get(metrics_handler))
        .route("/readyz", get(|| async { "ok" }))
        .with_state(intake_lab_metrics());

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("failed to bind metrics listener to {addr}"))?;
    info!(address = %addr, "intake lab metrics listening");
    axum::serve(listener, app)
        .await
        .context("intake lab metrics server crashed")
}

async fn metrics_handler(State(metrics): State<&'static IntakeLabMetrics>) -> impl IntoResponse {
    refresh_allocator_metrics(metrics);

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

fn refresh_allocator_metrics(metrics: &IntakeLabMetrics) {
    metrics.allocator_allocated_bytes.set(saturating_i64(
        ALLOCATOR_STATS.allocated_bytes.load(Ordering::Relaxed),
    ));
    metrics.allocator_allocation_count.set(saturating_i64(
        ALLOCATOR_STATS.allocation_count.load(Ordering::Relaxed),
    ));
    metrics.allocator_deallocated_bytes.set(saturating_i64(
        ALLOCATOR_STATS.deallocated_bytes.load(Ordering::Relaxed),
    ));
    metrics.allocator_live_bytes.set(saturating_i64(
        ALLOCATOR_STATS.live_bytes.load(Ordering::Relaxed),
    ));
    metrics.allocator_peak_live_bytes.set(saturating_i64(
        ALLOCATOR_STATS.peak_live_bytes.load(Ordering::Relaxed),
    ));
}

async fn run_worker(
    worker_index: usize,
    rate: u64,
    settings: Settings,
    samples: std::sync::Arc<Vec<Vec<u8>>>,
    deadline: Option<Instant>,
) -> Result<()> {
    let tick_every = Duration::from_millis(settings.tick_ms);
    let mut interval = tokio::time::interval(tick_every);
    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let metrics = intake_lab_metrics();
    let mode = settings.parser_mode.as_str();
    let messages_parsed_total = metrics.messages_parsed_total.with_label_values(&[mode]);
    let input_bytes_total = metrics.input_bytes_total.with_label_values(&[mode]);
    let parse_duration_seconds = metrics.parse_duration_seconds.with_label_values(&[mode]);
    let parse_failures_total = metrics.parse_failures_total.with_label_values(&[mode]);
    let mut sample_index = worker_index % samples.len();
    let mut burst_remainder = 0_u64;
    let mut parse_duration_cursor = worker_index as u64 % settings.parse_duration_sample_rate;

    loop {
        interval.tick().await;

        if deadline.is_some_and(|limit| Instant::now() >= limit) {
            return Ok(());
        }

        let mut batch_size = rate.saturating_mul(settings.tick_ms) / 1_000;
        burst_remainder += rate.saturating_mul(settings.tick_ms) % 1_000;
        if burst_remainder >= 1_000 {
            batch_size += burst_remainder / 1_000;
            burst_remainder %= 1_000;
        }
        if batch_size == 0 && rate > 0 {
            batch_size = 1;
        }

        let mut batch_successes = 0_u64;
        let mut batch_failures = 0_u64;
        let mut batch_input_bytes = 0_u64;

        for _ in 0..batch_size {
            if deadline.is_some_and(|limit| Instant::now() >= limit) {
                return Ok(());
            }

            let payload = &samples[sample_index];
            sample_index = (sample_index + 1) % samples.len();
            let should_sample_duration = parse_duration_cursor == 0;
            parse_duration_cursor =
                (parse_duration_cursor + 1) % settings.parse_duration_sample_rate;

            let started = should_sample_duration.then(Instant::now);
            let parsed = match settings.parser_mode {
                ParserMode::Thin => parse_thin(payload),
                ParserMode::Owned => parse_owned(payload),
            };

            match parsed {
                Ok(message) => {
                    batch_successes += 1;
                    batch_input_bytes += payload.len() as u64;
                    if let Some(started) = started {
                        parse_duration_seconds.observe(started.elapsed().as_secs_f64());
                    }
                    consume_normalized_message(&message);
                }
                Err(error) => {
                    batch_failures += 1;
                    if let Some(started) = started {
                        parse_duration_seconds.observe(started.elapsed().as_secs_f64());
                    }
                    warn!(worker_index, ?error, "failed to parse sample");
                }
            }
        }

        if batch_successes > 0 {
            messages_parsed_total.inc_by(batch_successes);
            input_bytes_total.inc_by(batch_input_bytes);
        }
        if batch_failures > 0 {
            parse_failures_total.inc_by(batch_failures);
        }
    }
}

fn consume_normalized_message(message: &NormalizedMessage) {
    black_box((
        message.attachment_count,
        message.author_id,
        message.channel_id,
        message.content_length,
        message.guild_id,
        message.is_bot,
        message.is_reply,
        message.message_id,
        message.occurred_at_unix_ms,
    ));
}

async fn run_reporter(settings: Settings) -> Result<()> {
    let metrics = intake_lab_metrics();
    let mode = settings.parser_mode.as_str();
    let mut last_count = 0_u64;
    let mut interval =
        tokio::time::interval(Duration::from_secs(settings.log_every_seconds.max(1)));

    loop {
        interval.tick().await;
        let total = metrics
            .messages_parsed_total
            .with_label_values(&[mode])
            .get() as u64;
        let delta = total.saturating_sub(last_count);
        last_count = total;
        refresh_allocator_metrics(metrics);
        let messages_per_second = delta as f64 / settings.log_every_seconds as f64;
        info!(
            mode,
            total_messages = total,
            messages_per_window = delta,
            messages_per_second,
            live_bytes = ALLOCATOR_STATS.live_bytes.load(Ordering::Relaxed),
            peak_live_bytes = ALLOCATOR_STATS.peak_live_bytes.load(Ordering::Relaxed),
            "intake lab progress"
        );
    }
}

async fn wait_for_shutdown(deadline: Option<Instant>) -> Result<()> {
    match deadline {
        Some(limit) => {
            tokio::select! {
                result = signal::ctrl_c() => {
                    result.context("failed to listen for ctrl-c")?;
                }
                _ = tokio::time::sleep_until(tokio::time::Instant::from_std(limit)) => {}
            }
        }
        None => {
            signal::ctrl_c()
                .await
                .context("failed to listen for ctrl-c")?;
        }
    }

    Ok(())
}

fn build_samples(settings: &Settings) -> Vec<Vec<u8>> {
    let attachment_template = build_attachment_template(settings.attachment_count);
    let content = "x".repeat(settings.content_bytes);
    let mut samples = Vec::with_capacity(settings.sample_count);
    let author_pool_size = settings.author_pool_size as u64;
    let shard_count = settings.shard_count.max(1);
    let base_message_time_ms = 1_742_070_400_000_u64;

    for index in 0..settings.sample_count {
        let guild_index = index % settings.guild_count.max(1);
        let channel_index = index % settings.channels_per_guild.max(1);
        let shard_id = guild_index % shard_count;
        let guild_id = 9_000_000_000_000_000_u64 + guild_index as u64;
        let channel_id =
            8_000_000_000_000_000_u64 + (guild_index as u64 * 10_000) + channel_index as u64;
        let message_id =
            build_message_snowflake(base_message_time_ms + index as u64, shard_id, index);
        let author_offset = ((guild_index as u64 * 131) + index as u64) % author_pool_size;
        let author_id = 6_000_000_000_000_000_u64 + author_offset;
        let reply_json = if index % 5 == 0 {
            format!(
                ",\"message_reference\":{{\"channel_id\":\"{channel_id}\",\"guild_id\":\"{guild_id}\",\"message_id\":\"{}\"}},\"referenced_message\":{{\"id\":\"{}\"}}",
                message_id.saturating_sub(1),
                message_id.saturating_sub(1),
            )
        } else {
            String::new()
        };
        let is_bot = if index % 17 == 0 { "true" } else { "false" };
        let payload = format!(
            "{{\"op\":0,\"t\":\"MESSAGE_CREATE\",\"s\":{index},\"shard_id\":{shard_id},\"d\":{{\"id\":\"{message_id}\",\"guild_id\":\"{guild_id}\",\"channel_id\":\"{channel_id}\",\"author\":{{\"id\":\"{author_id}\",\"bot\":{is_bot}}},\"content\":\"{content}\",\"attachments\":[{attachment_template}],\"timestamp\":\"2026-03-19T00:00:00Z\"{reply_json}}}}}"
        );
        samples.push(payload.into_bytes());
    }

    samples
}

fn build_attachment_template(attachment_count: usize) -> String {
    (0..attachment_count)
        .map(|index| {
            format!(
                "{{\"id\":\"{}\",\"size\":{},\"filename\":\"sample-{index}.png\"}}",
                5_000_000_000_000_000_u64 + index as u64,
                16_384_u64 + index as u64,
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn build_message_snowflake(timestamp_ms: u64, shard_id: usize, sequence: usize) -> u64 {
    let timestamp_component = (timestamp_ms.saturating_sub(DISCORD_EPOCH_MS as u64)) << 22;
    let worker_component = ((shard_id as u64) & 0x1F) << 17;
    let process_component = ((shard_id as u64) & 0x1F) << 12;
    let sequence_component = (sequence as u64) & 0xFFF;
    timestamp_component | worker_component | process_component | sequence_component
}

fn discord_unix_ms_from_snowflake(snowflake: u64) -> i64 {
    ((snowflake >> 22) as i64) + DISCORD_EPOCH_MS
}

fn parse_thin(payload: &[u8]) -> Result<NormalizedMessage> {
    let dispatch: ThinDispatch<'_> =
        serde_json::from_slice(payload).context("failed to deserialize thin dispatch")?;
    let message = dispatch.d;
    Ok(NormalizedMessage {
        attachment_count: u16::try_from(message.attachment_count).unwrap_or(u16::MAX),
        author_id: message.author.id,
        channel_id: message.channel_id,
        content_length: u16::try_from(message.content.len()).unwrap_or(u16::MAX),
        guild_id: message.guild_id,
        is_bot: message.author.bot,
        is_reply: message.has_message_reference || message.has_referenced_message,
        message_id: message.id,
        occurred_at_unix_ms: discord_unix_ms_from_snowflake(message.id),
    })
}

fn parse_owned(payload: &[u8]) -> Result<NormalizedMessage> {
    let dispatch: OwnedDispatch =
        serde_json::from_slice(payload).context("failed to deserialize owned dispatch")?;
    let message = dispatch.d;
    let attachment_size_bytes: u64 = message
        .attachments
        .iter()
        .map(|attachment| attachment.size)
        .sum();
    let attachment_id_bytes: usize = message
        .attachments
        .iter()
        .map(|attachment| attachment.id.len())
        .sum();
    let reply_ref_bytes = message
        .message_reference
        .as_ref()
        .map(|reference| {
            reference.channel_id.len() + reference.guild_id.len() + reference.message_id.len()
        })
        .unwrap_or(0);
    let referenced_message_id_bytes = message
        .referenced_message
        .as_ref()
        .map(|message| message.id.len())
        .unwrap_or(0);
    let normalized = NormalizedMessage {
        attachment_count: u16::try_from(message.attachments.len()).unwrap_or(u16::MAX),
        author_id: message
            .author
            .id
            .parse()
            .context("invalid owned author snowflake")?,
        channel_id: message
            .channel_id
            .parse()
            .context("invalid owned channel snowflake")?,
        content_length: u16::try_from(message.content.len()).unwrap_or(u16::MAX),
        guild_id: message
            .guild_id
            .parse()
            .context("invalid owned guild snowflake")?,
        is_bot: message.author.bot,
        is_reply: message.message_reference.is_some() || message.referenced_message.is_some(),
        message_id: message
            .id
            .parse()
            .context("invalid owned message snowflake")?,
        occurred_at_unix_ms: message.timestamp.timestamp_millis(),
    };
    black_box(attachment_size_bytes);
    black_box(attachment_id_bytes);
    black_box(reply_ref_bytes);
    black_box(referenced_message_id_bytes);
    Ok(normalized)
}

fn deserialize_snowflake<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Cow::<str>::deserialize(deserializer)?;
    value.parse().map_err(de::Error::custom)
}

fn deserialize_presence_flag<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Option::<IgnoredAny>::deserialize(deserializer)?.is_some())
}

fn deserialize_array_len<'de, D>(deserializer: D) -> Result<usize, D::Error>
where
    D: Deserializer<'de>,
{
    struct ArrayLenVisitor;

    impl<'de> Visitor<'de> for ArrayLenVisitor {
        type Value = usize;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a JSON array")
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let mut len = 0;
            while seq.next_element::<IgnoredAny>()?.is_some() {
                len += 1;
            }
            Ok(len)
        }
    }

    deserializer.deserialize_seq(ArrayLenVisitor)
}

fn assigned_rate(total_rate: u64, workers: usize, worker_index: usize) -> u64 {
    let workers = workers.max(1) as u64;
    let base = total_rate / workers;
    let remainder = total_rate % workers;
    base + u64::from((worker_index as u64) < remainder)
}

fn env_u64(name: &str, default: u64) -> Result<u64> {
    env::var(name)
        .ok()
        .map(|value| value.parse().with_context(|| format!("invalid {name}")))
        .transpose()
        .map(|value| value.unwrap_or(default))
}

fn env_usize(name: &str, default: usize) -> Result<usize> {
    env::var(name)
        .ok()
        .map(|value| value.parse().with_context(|| format!("invalid {name}")))
        .transpose()
        .map(|value| value.unwrap_or(default))
}

fn saturating_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}
