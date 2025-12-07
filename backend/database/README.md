# Database Layer (Postgres)

Postgres schema for settings, moderation logs, and analytics rollups.

## Tables
- `guild_settings (guild_id PK, prefix, moderation_enabled, analytics_enabled, sentiment_enabled, updated_at)`
- `moderation_logs (id PK, message_id, guild_id, channel_id, author_id, action, reason, created_at)`
- `analytics_message_counts (time_bucket, guild_id, count, PK(time_bucket, guild_id))`
- `analytics_sentiment (day, guild_id, sentiment, PK(day, guild_id))`

## Bootstrapping
- API and workers call `init_db()` on startup to ensure tables exist.
- Requires `DATABASE_URL` (e.g., `postgresql://postgres:postgres@localhost:5432/postgres`).

## Notes
- Keep heavy joins out of runtime paths.
- Future tables (guilds, channels) can be added in `backend/database/db.py`.
