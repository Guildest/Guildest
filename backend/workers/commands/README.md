# Commands Worker

Prefix commands are deprecated. Use Discord app commands (slash commands) instead.

## Why a worker?

Commands can be scaled independently from the Discord gateway listener. The gateway stays “thin” (only ingestion + enqueue).

## Current commands

Slash commands are registered by the gateway:

- `/stats`
- `/sentiment`
- `/modlogs` (Plus/Premium only)

## Required env

- `DISCORD_TOKEN` (required by the gateway for app commands)
- `DATABASE_URL` (metrics)
- `REDIS_URL`, `QUEUE_STREAM`, `QUEUE_GROUP`, `QUEUE_CONSUMER`
