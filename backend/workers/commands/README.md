# Commands Worker

Consumes `MESSAGE_CREATE` events and responds to prefix commands (e.g. `!help`).

## Why a worker?

Commands can be scaled independently from the Discord gateway listener. The gateway stays “thin” (only ingestion + enqueue).

## Current commands

- `!help`
- `!ping`
- `!dashboard` (prints the web dashboard URL)
- `!stats` (message counts last hour / 24 hours)
- `!sentiment` (latest sentiment label/score)
- `!modlogs` (Plus/Premium only; last few moderation log entries)

## Required env

- `DISCORD_TOKEN` (bot token for Discord REST API)
- `DATABASE_URL` (for prefix + metrics)
- `REDIS_URL`, `QUEUE_STREAM`, `QUEUE_GROUP`, `QUEUE_CONSUMER`
