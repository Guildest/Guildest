# Guildest

Discord analytics bot built in Rust with `serenity`, local Postgres, local Redis or Valkey, and a Vercel-friendly read layer.

## Services

- `api`: public stats API and backend read endpoints
- `gateway`: Discord gateway intake and event normalization
- `worker`: queue consumers that build analytics state
- `common`: shared event, config, queue, and storage primitives

## Local setup

1. Copy `.env.example` to `.env` and fill in the Discord values.
2. Start the full stack with `docker-compose up --build`.

If you want to run only the local infrastructure and keep the Rust processes on your host:

1. Start Postgres and Redis with `docker-compose up -d postgres redis`.
2. Run the API with `cargo run -p api`.
3. Run the gateway with `cargo run -p gateway`.
4. Run the workers with `cargo run -p worker`.

For synthetic worker backfill load without Discord REST, see [docs/worker-backfill-benchmark.md](/Users/ace/projects/guildest-worktrees/parse-footprint-lab/docs/worker-backfill-benchmark.md).

## Required Discord intents

Enable these bot intents in the Discord developer portal:

- Server Members Intent if you set `DISCORD_ENABLE_GUILD_MEMBERS_INTENT=true`
- Message Content Intent if you later decide to use message content

The current implementation tracks metadata and does not require message content to function. Member join and leave lifecycle tracking is disabled unless `DISCORD_ENABLE_GUILD_MEMBERS_INTENT=true`.

## Public stats endpoint

The public landing-page stats are served from:

- `GET /v1/public/stats`
- `GET /v1/public/stats/stream`
- `GET /v1/public/links`
- `GET /v1/public/oauth/start/login`
- `GET /v1/public/oauth/start/invite`
- `GET /v1/public/install/start`

`/v1/public/stats/stream` is a Server-Sent Events endpoint that pushes updated stats as soon as
the worker publishes a refresh notification, which avoids client polling delay.

The OAuth callback used by both landing-page CTAs is:

- `GET /v1/public/oauth/callback`

Expected deployment split:

- `guildest.site`: Next.js site
- `api.guildest.site`: backend API

## Discord OAuth setup

Add these redirect URLs in the Discord developer portal:

- `https://api.guildest.site/v1/public/oauth/callback`
- `https://guildest.site/dashboard`

Required environment variables for the public OAuth flow:

- `DISCORD_CLIENT_SECRET`
- `PUBLIC_API_BASE_URL`
- `PUBLIC_SITE_URL`

The bot invite permission bitset is currently defined in code in [crates/api/src/main.rs](/Users/ace/projects/guildest/crates/api/src/main.rs) so permission changes ship alongside feature changes.
