# Queue Layer

Lightweight event bus using Redis Streams to decouple the Discord gateway from
downstream workers.

## Status
- Requires Redis. Set `REDIS_URL` (e.g., `redis://localhost:6379/0`).
- Postgres is supported but optional for queueing; workers/API use it for persistence.

## Stream contract (approximate)
```json
{
  "event": "MESSAGE_CREATE",
  "message_id": "123",
  "guild_id": "456",
  "channel_id": "789",
  "author_id": "321",
  "content": "hello world",
  "timestamp": "2025-01-01T00:00:00Z",
  "metadata": {
    "is_webhook": false,
    "mentions_self": false
  }
}
```

## Operations
- Producer: Discord gateway pushes to stream `QUEUE_STREAM` (default `guildest:events`).
- Consumers: Workers read with consumer groups; acknowledge after processing.
- Backpressure: Stream capped by `QUEUE_MAX_LENGTH` (approximate trim).

## Local run checklist
1) Start Redis locally or in Docker (`docker run -p 6379:6379 redis:7`).
2) Export `REDIS_URL=redis://localhost:6379/0`.
3) Start the gateway; verify entries appear via `XREAD` or `XRANGE`.
