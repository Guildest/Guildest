# Queue Layer
- Use Redis Streams (preferred) or RabbitMQ.
- Standard payload:
  {
    "event": "MESSAGE_CREATE",
    "guild_id": "...",
    "channel_id": "...",
    "author_id": "...",
    "content": "...",
    "timestamp": "...",
    "metadata": {...}
  }
- Decouples gateway from workers; enables scaling and resilience.
