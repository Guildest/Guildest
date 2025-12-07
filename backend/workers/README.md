# Worker Pool

Workers consume events from the Redis Streams queue and perform specialized
tasks (moderation, analytics, sentiment).

## General pattern
- Create a consumer group per worker service.
- Read batches, process, ack; on failure, log and optionally dead-letter.
- Keep processing idempotent; dedupe by message ID if needed later.

## Dependencies
- `REDIS_URL` for queue access.
- `QUEUE_STREAM` (default `guildest:events`).
- `DATABASE_URL` to enable persistence.
- Optional: `API_BASE` for cross-service calls.
