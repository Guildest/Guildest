# HTTP API (FastAPI)

Provides Discord OAuth login + per-guild dashboards/settings for the web frontend.

## Required env

- `DATABASE_URL`
- `SESSION_SECRET` (HMAC signing secret for session tokens)
- Discord OAuth:
  - `DISCORD_CLIENT_ID`
  - `DISCORD_CLIENT_SECRET`
  - `DISCORD_OAUTH_REDIRECT_URI` (must match your Discord application settings)
- `FRONTEND_BASE_URL` (e.g. `http://localhost:3000`)

## Auth

The API accepts the session token via:
- `Authorization: Bearer <token>`
- or cookie `guildest_session=<token>`

OAuth flow:
- `GET /auth/discord/login?redirect=/dashboard`
- `GET /auth/discord/callback?code=...&state=...` (Discord redirects here)
- `GET /me` (current user + guild list)

## Guild onboarding

- `POST /guilds/{guild_id}/connect` marks the guild as owned/billed by the current user (requires Manage Guild permissions).

## Dashboards (subscription-gated)

- `GET /guilds/{guild_id}/dashboard/overview` → feature flags for UI locking
- `GET /guilds/{guild_id}/analytics/message-counts?hours=168`
- `GET /guilds/{guild_id}/sentiment/daily?days=30`
- `GET /guilds/{guild_id}/sentiment/report?day=YYYY-MM-DD` (Pro)
- `GET /guilds/{guild_id}/moderation/logs?limit=200` (Pro)

## Settings

- `GET /guilds/{guild_id}/settings`
- `PATCH /guilds/{guild_id}/settings`

