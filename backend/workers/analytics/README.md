# Analytics Worker

Consumes queue events and computes lightweight aggregates.

## Current behavior
- Read from Redis Streams and increment per-minute counters.
- Writes to Postgres `analytics_message_counts` when `DATABASE_URL` is set.

## Target behavior
- Additional buckets: 1h/1d rollups.
- Tables:
  - analytics_message_counts(time_bucket, guild_id, count)
  - analytics_sentiment(day, guild_id, sentiment)

## Notes
- Avoid per-user analytics to comply with Discord ToS.
- Keep logic idempotent; handle replayed messages gracefully.
