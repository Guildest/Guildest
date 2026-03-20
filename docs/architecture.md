# Discord Analytics Bot Architecture

This document describes the initial architecture for a Discord analytics bot built with Rust and `serenity`, using a local Postgres database and local Redis-compatible queue/cache on the bot host, with a dashboard deployed on Vercel.

## Goals

- Keep the Discord gateway process thin and reliable
- Separate event ingestion from analytics computation
- Preserve raw data so metrics can be recomputed later
- Make local infrastructure easy to replace with managed services later
- Serve dashboard reads from aggregated data, not raw event logs

## High-level design

The system is split into four logical parts:

- `gateway`: Discord shards and event normalization
- `queue`: event fan-out and background job dispatch
- `workers`: analytics and aggregation consumers
- `web`: dashboard and read APIs

## Process diagram

```text
Discord Gateway
    |
    v
[gateway service / serenity]
    | normalize + persist raw event
    +---------------------> [Postgres raw_events]
    |
    +---------------------> [Redis/Valkey queue]
                                 |
                                 v
                         [domain workers]
                                 |
                                 +------> [fact tables]
                                 |
                                 +------> [rollup tables / materialized views]
                                                      |
                                                      v
                                               [Vercel dashboard]
```

## Deployment layout

### EC2 host

Runs the stateful backend components:

- `gateway`
- `worker` processes
- local Postgres
- local Redis or Valkey

### Vercel

Runs the stateless frontend components:

- Next.js dashboard
- Discord OAuth install and callback routes
- read-only analytics APIs
- scheduled report triggers if needed

## Core rule

The gateway should not compute analytics. It should only:

- receive Discord events
- convert them into internal event types
- persist raw events
- enqueue work for downstream consumers

This keeps Discord connectivity isolated from analytics CPU load.

## Service boundaries

## `gateway`

Responsibilities:

- maintain the `serenity` connection
- receive Discord events
- normalize Discord payloads into internal event types
- write raw events to Postgres
- enqueue domain-specific jobs to Redis

Non-responsibilities:

- heavy analytics logic
- dashboard queries
- rollup generation
- content analysis

## `workers`

Responsibilities:

- consume queue messages
- update fact tables
- build derived analytics state
- run scheduled rollups
- retry transient failures safely

Recommended worker split:

- `member-worker`
- `message-worker`
- `voice-worker`
- `moderation-worker`
- `rollup-worker`

## `web`

Responsibilities:

- install flow and guild linking
- dashboard rendering
- read APIs
- exports and reports

The web layer should read primarily from aggregated tables and materialized views.

## Internal event model

Do not make raw Discord SDK structs the long-term system contract. Define internal typed events with stable names.

Examples:

- `member_joined`
- `member_left`
- `member_role_added`
- `member_role_removed`
- `message_created`
- `message_deleted`
- `message_reaction_added`
- `voice_session_started`
- `voice_session_ended`
- `thread_created`
- `moderation_action_taken`

Each event should include a common envelope:

```rust
pub struct EventEnvelope<T> {
    pub event_id: uuid::Uuid,
    pub event_name: String,
    pub guild_id: String,
    pub channel_id: Option<String>,
    pub user_id: Option<String>,
    pub occurred_at: chrono::DateTime<chrono::Utc>,
    pub received_at: chrono::DateTime<chrono::Utc>,
    pub version: i32,
    pub payload: T,
}
```

Guidelines:

- use string IDs for Discord snowflakes at system boundaries
- version event schemas
- keep payloads minimal and explicit
- prefer metadata over message content

## Queue layout

Redis or Valkey is used for queueing and lightweight caching. Even if both use the same backend today, treat queue and cache as separate abstractions in code.

Suggested queue streams:

- `events.member`
- `events.message`
- `events.voice`
- `events.moderation`
- `jobs.rollup`

Consumer groups:

- `member-workers`
- `message-workers`
- `voice-workers`
- `moderation-workers`
- `rollup-workers`

Queue message shape:

```json
{
  "event_id": "uuid",
  "event_name": "message_created",
  "guild_id": "123",
  "occurred_at": "2026-03-15T18:00:00Z",
  "raw_event_row_id": 98765
}
```

Prefer queueing references to persisted events instead of large payloads where possible.

## Database strategy

Use Postgres as the durable source of truth.

The data model has three layers:

- raw events
- fact tables
- metric tables or materialized views

## Raw events

Purpose:

- immutable audit trail
- replay and backfill source
- debugging and metric redefinition support

Suggested table:

### `raw_events`

- `id`
- `event_id`
- `event_name`
- `guild_id`
- `channel_id`
- `user_id`
- `occurred_at`
- `received_at`
- `schema_version`
- `payload_json`
- `processed_at` nullable

Indexes:

- `event_id`
- `guild_id, occurred_at`
- `event_name, occurred_at`

## Fact tables

These tables hold cleaned, queryable, domain-specific records.

Suggested tables:

### `member_daily_activity`

- `guild_id`
- `member_id`
- `date`
- `messages_sent`
- `reactions_added`
- `voice_seconds`
- `active_channels`
- `was_active`

### `channel_daily_activity`

- `guild_id`
- `channel_id`
- `date`
- `messages`
- `unique_senders`
- `replies`
- `median_response_seconds`
- `unanswered_threads`

### `member_lifecycle`

- `guild_id`
- `member_id`
- `joined_at`
- `left_at`
- `invite_code`
- `acquisition_source`
- `first_message_at`
- `first_reaction_at`
- `first_voice_at`
- `first_role_at`

### `voice_sessions`

- `guild_id`
- `member_id`
- `channel_id`
- `started_at`
- `ended_at`
- `duration_seconds`

### `moderation_actions`

- `guild_id`
- `target_user_id`
- `moderator_user_id`
- `action_type`
- `reason`
- `occurred_at`

## Metrics and rollups

These tables or materialized views power the product.

Suggested rollups:

### `retention_cohorts`

- retention by join date
- retention by invite source
- retention by campaign

### `activation_funnel_daily`

- joined
- completed rules
- got starter role
- first message
- first voice
- returned in 7 days

### `channel_health_daily`

- active members
- response latency
- participation concentration
- decay score

### `guild_summary_daily`

- DAU
- WAU
- MAU
- join/leave ratio
- onboarding completion rate
- retained-member contribution by channel

## Caching strategy

Use Redis or Valkey for:

- queue streams
- short-lived dashboard cache
- hot guild summary cache
- rate-limiting or dedupe helpers if needed

Do not use Redis as the only durable event store.

Reasonable cache candidates:

- `guild:{guild_id}:summary`
- `guild:{guild_id}:top_channels:{date}`
- `guild:{guild_id}:retention_snapshot`

## Reliability and failure handling

### Gateway rules

- persist before enqueue where possible
- include idempotency identifiers on events
- never block shard health on expensive downstream work
- degrade gracefully if workers are behind

### Worker rules

- make consumers idempotent
- update facts using upserts
- keep retries safe
- send poison or repeatedly failing jobs to a dead-letter mechanism

### Operational signals

Monitor:

- shard reconnect frequency
- queue depth by stream
- worker lag
- Postgres CPU and memory
- Redis memory
- raw event insert rate
- rollup duration

## Privacy and data minimization

Default to storing metadata instead of message content.

Store:

- IDs
- timestamps
- role changes
- reply/reference flags
- attachment counts
- reaction counts
- voice durations

Avoid storing full message text unless a feature explicitly requires it and the server has opted in.

## Scaling path

The application should be written so local infrastructure can be replaced without major code changes.

Expected migration order:

1. move Postgres to managed infrastructure
2. move Redis or Valkey to managed infrastructure
3. split workers onto separate compute
4. increase shard and worker counts

This requires clear interfaces:

- `EventStore`
- `EventQueue`
- `AnalyticsRepository`
- `CacheStore`

## Limits of `t2.micro`

This setup is acceptable for local development and a small MVP, but the host is likely to become unstable under sustained load once it runs:

- Discord shards
- Postgres
- Redis
- multiple workers
- rollup jobs

Likely failure points:

- memory pressure
- CPU credit exhaustion
- shard instability during rollups
- degraded DB performance under dashboard load

For this reason:

- keep the gateway lightweight
- batch worker writes
- schedule rollups outside hot ingestion paths
- avoid querying raw event tables from the UI

## Recommended Rust stack

- `serenity` for Discord gateway integration
- `tokio` for async runtime
- `sqlx` for Postgres access
- `serde` for event serialization
- `tracing` for logs and instrumentation
- `axum` if an internal admin or API service is needed
- `redis` or `fred` for Redis or Valkey integration

## Initial implementation sequence

1. build `gateway` with internal event normalization
2. persist `raw_events`
3. enqueue to Redis streams
4. implement one worker end-to-end, starting with member lifecycle
5. add fact tables
6. add daily rollups
7. build dashboard read APIs against rollups

## Non-goals for v1

- full-text message analytics
- AI summaries of all conversations
- complex graph analytics
- real-time dashboards built directly on raw events
- analytics computed inside the gateway process
