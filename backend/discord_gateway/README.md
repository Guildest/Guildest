# Discord Gateway Layer

Fast, minimal Discord gateway listener using Hikari. Listens for guild message
events, normalizes payloads, and pushes them to the queue for workers.

## How it works
- Uses `hikari.GatewayBot` with `GUILD_MESSAGES` + `MESSAGE_CONTENT` intents.
- On `GuildMessageCreate`:
  - Ignore bots/webhooks; skip empty messages.
  - Normalize payload into the shared queue schema.
  - Push to Redis Streams via `QueuePublisher`.
- No API/DB/LLM calls here; keep connection fast and predictable.

## Running locally
```bash
cd backend/discord_gateway
python -m venv .venv
source .venv/bin/activate  # or .venv\Scripts\activate on Windows
pip install -r requirements.txt
DISCORD_TOKEN=your_bot_token \
REDIS_URL=redis://localhost:6379/0 \
QUEUE_STREAM=guildest:events \
python -m backend.discord_gateway.main
```

## Environment variables
- `DISCORD_TOKEN` (required): Bot token.
- `REDIS_URL` (required): Redis connection string.
- `QUEUE_STREAM` (optional, default `guildest:events`): Stream name for events.
- `QUEUE_MAX_LENGTH` (optional, default `5000`): Max stream length (approximate).
- `LOG_LEVEL` (optional, default `INFO`): Logging level.
