# Backfill Lab

This worktree includes a small local sandbox for testing historical message backfill without connecting the real Discord API.

## Components

- `postgres` and `redis` from [`infra/docker-compose.backfill-lab.yml`](/Users/ace/projects/guildest-backfill-lab/infra/docker-compose.backfill-lab.yml)
- `api` and `worker` from the existing Rust workspace
- a local mock Discord API from [`infra/mock_discord_api.py`](/Users/ace/projects/guildest-backfill-lab/infra/mock_discord_api.py)

## Quick start

1. Run [`scripts/backfill-lab-serve.sh`](/Users/ace/projects/guildest-backfill-lab/scripts/backfill-lab-serve.sh) in a terminal to keep the lab running.
2. Run [`scripts/backfill-lab-trigger.sh`](/Users/ace/projects/guildest-backfill-lab/scripts/backfill-lab-trigger.sh).
3. Stop everything with [`scripts/backfill-lab-down.sh`](/Users/ace/projects/guildest-backfill-lab/scripts/backfill-lab-down.sh).

The first run creates `.env.backfill-lab` from [`infra/backfill-lab.env.example`](/Users/ace/projects/guildest-backfill-lab/infra/backfill-lab.env.example).

## Rate-limit simulation

Set these in `.env.backfill-lab` before starting the lab:

- `MOCK_RATE_LIMIT_EVERY_N_MESSAGES_REQUESTS=3`
- `MOCK_RATE_LIMIT_RETRY_AFTER_MS=750`

That makes every third `/channels/{id}/messages` request return a synthetic Discord-style `429` so the worker's retry path can be exercised locally.
