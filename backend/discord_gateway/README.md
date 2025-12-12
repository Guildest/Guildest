# Discord Gateway Layer

Fast, minimal Discord gateway listener using Hikari. Listens for guild message
events, normalizes payloads, and pushes them to the queue for workers.

## How it works
- Uses `hikari.GatewayBot` with `GUILD_MESSAGES` + `MESSAGE_CONTENT` intents.
- On `GuildMessageCreate`:
  - Ignore bots/webhooks; skip empty messages.
  - Normalize payload into the shared queue schema.
  - Push to Redis Streams via `QueuePublisher`.
- On `InteractionCreate`:
  - Handles slash commands (ephemeral replies).
  - Registers application commands at startup when `DISCORD_APPLICATION_ID` is set.
- No API/DB/LLM calls here; keep connection fast and predictable.

## Running locally
```bash
cd backend/discord_gateway
python -m venv .venv
source .venv/bin/activate  # or .venv\Scripts\activate on Windows
pip install -r requirements.txt
DISCORD_TOKEN=your_bot_token \
DISCORD_APPLICATION_ID=your_app_id \
DISCORD_COMMANDS_GUILD_ID=your_test_guild_id \
FRONTEND_BASE_URL=http://localhost:3000 \
REDIS_URL=redis://localhost:6379/0 \
QUEUE_STREAM=guildest:events \
python -m backend.discord_gateway.main
```

## Environment variables
- `DISCORD_TOKEN` (required): Bot token.
- `DISCORD_APPLICATION_ID` (optional): Enables registering slash commands at startup.
- `DISCORD_COMMANDS_GUILD_ID` (optional): Register commands as guild commands (faster propagation).
- `FRONTEND_BASE_URL` (optional, default `http://localhost:3000`): Used by `/dashboard`.
- `DATABASE_URL` (optional): Enables DB-backed slash commands (`/stats`, `/sentiment`, `/modlogs`).
- `REDIS_URL` (required): Redis connection string.
- `QUEUE_STREAM` (optional, default `guildest:events`): Stream name for events.
- `QUEUE_MAX_LENGTH` (optional, default `5000`): Max stream length (approximate).
- `LOG_LEVEL` (optional, default `INFO`): Logging level.
