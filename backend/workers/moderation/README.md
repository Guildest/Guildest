# Moderation Worker

Consumes queue events and evaluates content safety.

## Current mode
- Read from Redis Streams, call LLM safety (e.g., Groq Llama Guard 4) if available.
- Log outcomes and write to Postgres `moderation_logs` when `DATABASE_URL` is set.

## Target behavior
- Store moderation decisions in `moderation_logs`.
- Emit alerts to guild mod-log channels.

## Notes
- Avoid storing full message content unless required for audit.
- Keep latency low; consider short timeouts on external calls.
