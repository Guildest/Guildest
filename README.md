# Guildest

![Status](https://img.shields.io/badge/status-active%20development-2ea44f)
![Rust](https://img.shields.io/badge/Rust-2024-orange?logo=rust)
![Next.js](https://img.shields.io/badge/Next.js-16-black?logo=nextdotjs)
![Postgres](https://img.shields.io/badge/Postgres-17-336791?logo=postgresql&logoColor=white)
![Valkey](https://img.shields.io/badge/Valkey-8-a41e11)
![License](https://img.shields.io/badge/license-MIT-blue)

Guildest is a Discord community intelligence platform. Today it ingests Discord events, builds analytics from message/member/voice activity, and serves a web dashboard. The long-term direction is an AI community operator: a bot that understands the shape of a Discord server, surfaces what needs attention, and helps owners turn conversation into action.

The project is built around a simple rule: keep Discord ingestion reliable, preserve enough event history to recompute analytics, and do expensive work in background workers.

## What It Does Today

- Captures Discord gateway events with a thin Rust `serenity` gateway service
- Stores normalized raw events in Postgres
- Publishes event references through Redis or Valkey streams
- Builds analytics in worker processes
- Tracks member lifecycle, message activity, channel health, retention, activation, voice sessions, and queue health
- Serves dashboard APIs from aggregated tables and cache-friendly read models
- Provides a Next.js dashboard for installed guilds and authenticated admins
- Includes local backfill and intake labs for testing pipeline behavior

## Where It Is Going

Guildest is being shaped into an AI-first Discord operator rather than a plain stats bot.

Planned AI direction:

- **Live Pulse**: an hourly view of what changed and what needs attention
- **Real-time alerts**: high-signal alerts for urgent support, pricing, launch, onboarding, or moderation issues
- **Server Map**: an agent-built understanding of each server's type, channels, roles, bots, gates, and owner goals
- **Adaptive dashboard**: show only the modules a server actually needs
- **Ask Guildest**: admin-only questions over server context
- **Action drafts**: suggested replies, announcements, FAQs, and follow-ups for owner approval

The AI product plan is documented in [docs/ai-agent-pivot.md](docs/ai-agent-pivot.md).

## Architecture

```text
Discord Gateway
    |
    v
gateway service
    | normalize + persist
    +---------------------> Postgres raw_events
    |
    +---------------------> Redis/Valkey streams
                                 |
                                 v
                              workers
                                 |
                                 +------> fact tables
                                 +------> rollups / cache
                                                 |
                                                 v
                                           API + dashboard
```

Services:

- `crates/gateway`: Discord gateway connection, event normalization, raw event persistence, queue publishing
- `crates/worker`: queue consumers, historical backfill, analytics updates, rollups, retry/dead-letter handling
- `crates/api`: public stats, OAuth, dashboard APIs, install flow, operational endpoints
- `crates/common`: shared config, event types, queue, job, and storage primitives
- `web`: Next.js dashboard and landing/read layer

## Repository Layout

```text
.
├── crates/
│   ├── api/          # Axum API service
│   ├── common/       # Shared Rust library
│   ├── gateway/      # Discord gateway intake
│   ├── intake-lab/   # Synthetic intake/load experiments
│   └── worker/       # Analytics and backfill workers
├── docs/             # Architecture, experiments, roadmap notes
├── infra/            # Local Prometheus and backfill lab compose files
├── scripts/          # Local helper scripts
├── web/              # Next.js dashboard
├── docker-compose.yml
└── README.md
```

## Quick Start

Requirements:

- Rust toolchain from [rust-toolchain.toml](rust-toolchain.toml)
- Docker and Docker Compose
- Node.js and npm for the dashboard
- A Discord application/bot token

Copy the environment file:

```bash
cp .env.example .env
```

Fill in:

```text
DISCORD_TOKEN
DISCORD_APPLICATION_ID
DISCORD_CLIENT_SECRET
PUBLIC_API_BASE_URL
PUBLIC_SITE_URL
RESEND_API_KEY
RESEND_FROM_EMAIL
GUILDEST_EMAIL_TO
```

Start the backend stack:

```bash
docker-compose up --build
```

The API listens on:

```text
http://127.0.0.1:8080
```

Run the dashboard:

```bash
cd web
cp .env.example .env.local
npm install
npm run dev
```

The dashboard listens on:

```text
http://127.0.0.1:3000
```

## Temporary Vercel Frontend

The `web` app can be deployed on Vercel while the Rust API runs elsewhere. Set
the Vercel project root directory to `web` and configure:

```text
GUILDEST_API_BASE_URL=https://your-public-api-host
```

The Next.js API routes proxy form submissions and dashboard requests to that
Rust API. Keep `RESEND_API_KEY`, `RESEND_FROM_EMAIL`, and `GUILDEST_EMAIL_TO`
on the backend API host, not in Vercel.

## API Deployment

The production API deploy path is in
[.github/workflows/deploy-api.yml](.github/workflows/deploy-api.yml). It builds
the API Docker image in GitHub Actions, uploads the deployment bundle to S3, and
uses SSM Run Command to update the EC2 host. The host runs the production compose
stack in [infra/production](infra/production):

- `api`: Rust API service
- `postgres`: local Postgres data store
- `redis`: local Valkey/Redis queue
- `caddy`: HTTPS reverse proxy for `api.guildest.site`

Provision a free-tier-sized EC2 host with:

```bash
AWS_REGION=us-east-1 \
IAM_INSTANCE_PROFILE=guildest-api-ec2-profile \
./scripts/provision-api-ec2.sh
```

The workflow expects these GitHub repository secrets:

```text
AWS_ACCESS_KEY_ID
AWS_SECRET_ACCESS_KEY
DISCORD_TOKEN
DISCORD_APPLICATION_ID
DISCORD_CLIENT_SECRET
OPENROUTER_API_KEY
RESEND_API_KEY
RESEND_FROM_EMAIL
GUILDEST_EMAIL_TO
```

And these GitHub repository variables:

```text
API_DOMAIN=api.guildest.site
PUBLIC_API_BASE_URL=https://api.guildest.site
PUBLIC_API_ALLOWED_ORIGIN=https://guildest.site
PUBLIC_SITE_URL=https://guildest.site
AI_CLASSIFY_MODEL=stepfun/step-3.5-flash
AI_SYNTHESIS_MODEL=minimax/minimax-m2.7
DISCORD_ENABLE_GUILD_MEMBERS_INTENT=false
DISCORD_ENABLE_MESSAGE_CONTENT_INTENT=false
RUST_LOG=info
AWS_REGION=us-east-1
AWS_DEPLOY_BUCKET=guildest-api-deploy-<account>-us-east-1
AWS_INSTANCE_ID=i-...
```

Point `api.guildest.site` to the EC2 public IP with an `A` record. Caddy will
issue the HTTPS certificate once DNS resolves to the instance.

## Running Services Manually

If you want Postgres and Valkey in Docker but Rust services on your host:

```bash
docker-compose up -d postgres redis
cargo run -p api
cargo run -p gateway
cargo run -p worker
```

Useful health and metrics endpoints:

- `GET /health` on the API service
- `GET /metrics` on the API service
- `GET /metrics` on the worker metrics listener
- `GET /metrics` on the gateway metrics listener

## Discord Setup

Add these OAuth redirect URLs in the Discord developer portal for production:

```text
https://api.guildest.site/v1/public/oauth/callback
https://guildest.site/dashboard
```

For local development, set `PUBLIC_API_BASE_URL` and `PUBLIC_SITE_URL` in `.env` to your local API and dashboard URLs.

Required Discord intents:

- `GUILDS`
- `GUILD_MESSAGES`
- `GUILD_MESSAGE_REACTIONS`
- `GUILD_VOICE_STATES`
- `GUILD_MEMBERS` only when `DISCORD_ENABLE_GUILD_MEMBERS_INTENT=true`

The current implementation tracks message metadata and does not require Message Content Intent. The AI roadmap introduces opt-in content analysis with redaction and retention controls.

## API Highlights

Public endpoints:

- `GET /v1/public/stats`
- `GET /v1/public/stats/stream`
- `GET /v1/public/messages/heatmap`
- `GET /v1/public/links`
- `GET /v1/public/oauth/start/login`
- `GET /v1/public/oauth/start/invite`
- `GET /v1/public/install/start`
- `GET /v1/public/oauth/callback`

Dashboard endpoints include:

- `GET /v1/dashboard/me`
- `GET /v1/dashboard/guilds/{guild_id}/messages/summary`
- `GET /v1/dashboard/guilds/{guild_id}/messages/heatmap`
- `GET /v1/dashboard/guilds/{guild_id}/summary/health`
- `GET /v1/dashboard/guilds/{guild_id}/retention/cohorts`
- `GET /v1/dashboard/guilds/{guild_id}/activation/funnel`
- `GET /v1/dashboard/guilds/{guild_id}/channels/hotspots`
- `GET /v1/dashboard/guilds/{guild_id}/users/summary`
- `GET /v1/dashboard/guilds/{guild_id}/ops/pipeline`

## Development Notes

Format Rust code:

```bash
cargo fmt
```

Check Rust code:

```bash
cargo check --workspace
```

Run the dashboard lint:

```bash
cd web
npm run lint
```

Run a local historical backfill lab without calling Discord:

```bash
scripts/backfill-lab-serve.sh
scripts/backfill-lab-trigger.sh
```

See [docs/backfill-lab.md](docs/backfill-lab.md) for details.

## Documentation

- [Architecture](docs/architecture.md)
- [AI agent pivot](docs/ai-agent-pivot.md)
- [Analytics tracking plan](docs/analytics-tracking.md)
- [Backfill lab](docs/backfill-lab.md)
- [Intake footprint lab](docs/intake-footprint-lab.md)
- [Worker backfill benchmark](docs/worker-backfill-benchmark.md)

## Project Status

Guildest is early and moving quickly. The core ingestion, worker, API, and dashboard pieces are in active development. The AI-agent work is currently planned in docs and should be treated as roadmap unless the code says otherwise.

Issues and pull requests are welcome.

## License

MIT. See the workspace package metadata in [Cargo.toml](Cargo.toml).
