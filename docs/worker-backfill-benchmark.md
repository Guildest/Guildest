# Worker Backfill Benchmark

This flow benchmarks the worker's Discord REST backfill path against a local fake Discord API instead of the real Discord service.

## What this is for

- Stressing `fetch_channel_messages` and downstream indexing logic without Discord rate limits
- Measuring worker throughput and response size under large synthetic guilds
- Reproducing reply-heavy, attachment-heavy, and 429-heavy histories on demand

This does not benchmark the websocket gateway intake path. Use the intake lab for that.

## Start infrastructure

Run Postgres and Redis locally:

```bash
docker-compose up -d postgres redis
```

## Start the fake Discord API

Example large benchmark shape:

```bash
MOCK_GUILD_IDS=900000000000000001,900000000000000002 \
MOCK_CHANNEL_COUNT=128 \
MOCK_MESSAGES_PER_CHANNEL=5000 \
MOCK_MESSAGE_CONTENT_BYTES=512 \
MOCK_ATTACHMENTS_PER_MESSAGE=3 \
MOCK_AUTHOR_COUNT=250000 \
MOCK_REPLY_EVERY=5 \
MOCK_BOT_EVERY=17 \
MOCK_RATE_LIMIT_EVERY=0 \
python3 infra/mock_discord_api.py
```

Useful knobs:

- `MOCK_GUILD_IDS`: comma-separated guild IDs the worker can backfill
- `MOCK_GUILD_COUNT`: generate numeric guild IDs automatically when `MOCK_GUILD_IDS` is unset
- `MOCK_CHANNEL_COUNT`: channels per guild
- `MOCK_MESSAGES_PER_CHANNEL`: messages returned by each channel history
- `MOCK_MESSAGE_CONTENT_BYTES`: content size for each message
- `MOCK_ATTACHMENTS_PER_MESSAGE`: attachment fanout
- `MOCK_REPLY_EVERY`: every Nth message becomes a reply
- `MOCK_BOT_EVERY`: every Nth message is authored by a bot
- `MOCK_RATE_LIMIT_EVERY`: every Nth message-history request returns `429`
- `MOCK_RATE_LIMIT_RETRY_AFTER_MS`: retry-after for synthetic `429`s

The mock also serves:

- `GET /api/v10/guilds/{guild_id}/channels`
- `GET /api/v10/channels/{channel_id}`
- `GET /api/v10/channels/{channel_id}/messages`
- `GET /healthz`

## Run the worker against the fake API

Use dummy Discord auth values because the worker config requires them even when the mock is local:

```bash
DATABASE_URL=postgres://guildest:guildest@127.0.0.1:5432/guildest \
REDIS_URL=redis://127.0.0.1:6379 \
DISCORD_TOKEN=fake-token \
DISCORD_APPLICATION_ID=1 \
DISCORD_CLIENT_SECRET=fake-secret \
DISCORD_API_BASE_URL=http://127.0.0.1:9191/api/v10 \
WORKER_BACKFILL_PAGE_DELAY_MS=0 \
WORKER_METRICS_BIND_ADDR=127.0.0.1:19091 \
cargo run -p worker
```

## Enqueue a backfill job

Pick one of the guild IDs served by the mock and enqueue a job with the helper script:

```bash
python3 scripts/enqueue_backfill_job.py \
  --redis-url redis://127.0.0.1:6379 \
  --guild-id 900000000000000001 \
  --days 7
```

Useful knobs:

- `--count`: enqueue multiple jobs in one command
- `--job-spacing-seconds`: offset the requested timestamps between jobs
- `--requested-by-user-id`: attach a fake dashboard user ID
- `--trigger-source`: change the stored trigger source from the default `benchmark`

## Watch metrics

Prometheus can scrape the worker and the fake API is deterministic enough that the worker metrics are usually enough.

Useful queries:

```promql
rate(worker_discord_requests_total{endpoint="channel_messages"}[30s])
```

```promql
rate(worker_discord_response_size_bytes_sum{endpoint="channel_messages"}[30s])
/
rate(worker_discord_response_size_bytes_count{endpoint="channel_messages"}[30s])
```

```promql
rate(worker_messages_indexed_per_backfill_job_sum[5m])
```

```promql
histogram_quantile(0.95, rate(worker_discord_request_duration_seconds_bucket[30s]))
```

```promql
worker_queue_ready_messages{stream="jobs.backfill"}
```

## Notes

- Set `MOCK_RATE_LIMIT_EVERY` above zero if you want to exercise the worker retry path for Discord `429`s.
- Set `WORKER_BACKFILL_PAGE_DELAY_MS=0` for max throughput tests. Restore a delay if you want a more production-like profile.
- The worker still computes `content_length` with `chars().count()` during backfill, so very large unicode-heavy payloads will still cost CPU even with the lighter JSON decoding.
