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

Optional (Discord bot integration):
- `DISCORD_TOKEN` (required for bot presence checks + appeal unbans)

Optional (Stripe billing):
- `STRIPE_SECRET_KEY`
- `STRIPE_WEBHOOK_SECRET`
- `STRIPE_PLUS_PRICE_ID`
- `STRIPE_PREMIUM_PRICE_ID`

Optional (Appeal summaries):
- `OPENROUTER_API_KEY`
- `OPENROUTER_MODEL` (default `deepseek/deepseek-v3.2`)

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
- `POST /guilds/{guild_id}/disconnect` removes the guild from the current user's billing account.

## Dashboards (subscription-gated)

- `GET /guilds/{guild_id}/dashboard/overview` → feature flags for UI locking
- `GET /guilds/{guild_id}/analytics/message-counts?hours=168`
- `GET /guilds/{guild_id}/sentiment/daily?days=30`
- `GET /guilds/{guild_id}/sentiment/report?day=YYYY-MM-DD` (Plus/Premium)
- `GET /guilds/{guild_id}/moderation/logs?limit=200` (Plus/Premium)

## Settings

- `GET /guilds/{guild_id}/settings`
- `PATCH /guilds/{guild_id}/settings`

## Appeals

- `GET /guilds/{guild_id}/appeals`
- `POST /guilds/{guild_id}/appeals/{appeal_id}/unban`
- `POST /guilds/{guild_id}/appeals/{appeal_id}/delete`
- `POST /guilds/{guild_id}/appeals/{appeal_id}/block`
- `POST /guilds/{guild_id}/appeals/{appeal_id}/summarize` (Plus/Premium only)

## Billing (Stripe)

- `GET /billing/subscription`
- `POST /billing/checkout` (body: `{ "plan": "plus" | "premium" }`)
- `POST /billing/portal`
- `POST /webhooks/stripe` (alias: `/subscriptions/stripe`)
