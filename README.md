# Guildest

Monorepo for the Guildest Discord bot and supporting web services.

- `frontend/`: Next.js dashboard for analytics, settings, and billing (with nginx reverse proxy)
- `backend/`: Services for Discord gateway, queue, workers, HTTP API, and database schemas
- `docs/`: Docusaurus documentation site

## Stack Overview
- Discord gateway: Hikari (Python)
- Queue: Redis Streams (preferred) or RabbitMQ
- Workers: Python microservices (moderation, analytics, sentiment)
- API: FastAPI
- DB: Postgres (Neon/Supabase/RDS)
- Frontend: Next.js

## Running locally (Docker Compose)

Required env vars:
- `DISCORD_TOKEN` (bot token)
- `DISCORD_APPLICATION_ID` (required for slash commands registration)
- `SESSION_SECRET` (any long random string)
- Discord OAuth (for dashboard login):
  - `DISCORD_CLIENT_ID`
  - `DISCORD_CLIENT_SECRET`
  - `DISCORD_OAUTH_REDIRECT_URI` (e.g. `http://localhost:8000/auth/discord/callback`)
- `FRONTEND_BASE_URL` (e.g. `http://localhost:3000`)

Start backend services:

```bash
docker compose up --build
```

Start frontend:

```bash
cd frontend
npm install
npm run dev
```

## Deployment notes (ARM64 / t4g.nano)

- `python:3.11-slim`, `redis:7`, and `postgres:16` are multi-arch and work on `linux/arm64`.
- A `t4g.nano` is extremely resource constrained; running Postgres + Redis + API + gateway + multiple workers + Next.js on the same box will likely OOM.
  - Recommended: use managed Postgres + managed Redis, and run only the app containers on the instance.
  - Recommended: build images in CI on an ARM64 runner (or `buildx --platform linux/arm64`) and deploy prebuilt images (avoid compiling on the nano).

## Production (docker-compose.prod.yml)

`docker-compose.prod.yml` assumes Postgres + Redis are managed externally and provided via `.env`.

Required `.env` keys:
- `DATABASE_URL` (for API + workers; include SSL params if your provider requires it, e.g. `?sslmode=require`)
- `REDIS_URL`
- `SESSION_SECRET`
- `FRONTEND_BASE_URL` (the public URL, e.g. `https://yourdomain.com`)
- `DISCORD_TOKEN`

Also required for Discord OAuth login:
- `DISCORD_CLIENT_ID`
- `DISCORD_CLIENT_SECRET`
- `DISCORD_OAUTH_REDIRECT_URI`

Optional (Stripe billing):
- `STRIPE_SECRET_KEY`
- `STRIPE_WEBHOOK_SECRET`
- `STRIPE_PRO_PRICE_ID`
