# Backend Layers

- `discord_gateway/`: Discord gateway listener; normalizes events and pushes to the queue (no heavy work).
- `queue/`: Queue client and helpers (Redis Streams).
- `workers/`: Worker services (analytics, moderation, sentiment).
- `api/`: FastAPI HTTP layer for OAuth login, onboarding, settings, and dashboards.
- `database/`: Postgres schema init + persistence helpers.
- `common/`: Shared libs/models/config.

## What’s implemented

- Discord OAuth login (user `identify` + `guilds`) with server-side sessions.
- Per-user subscription plan (`free` vs `plus`/`premium`) and per-guild billing ownership (`/guilds/{id}/connect`).
- Subscription-gated endpoints:
  - Plus/Premium moderation audit history
  - Plus/Premium daily sentiment report + event recommendations
- Analytics: per-minute message counts (time series).

## OpenRouter model

Sentiment “daily agent” uses OpenRouter:
- `OPENROUTER_API_KEY`
- `OPENROUTER_MODEL=deepseek/deepseek-v3.2`
