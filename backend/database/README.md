# Database Layer (Postgres)

Postgres schema for settings, moderation logs, and analytics rollups.

## Tables
- `guild_settings (guild_id PK, prefix, moderation_enabled, analytics_enabled, sentiment_enabled, warn_decay_days, warn_policy, updated_at)`
- `moderation_logs (id PK, message_id, guild_id, channel_id, author_id, action, reason, actor_id, actor_type, target_id, bot_id, source, metadata, created_at)`
- `analytics_message_counts (time_bucket, guild_id, count, PK(time_bucket, guild_id))`
- `analytics_sentiment (day, guild_id, sentiment, PK(day, guild_id))`
- `guild_warns (id PK, guild_id, user_id, moderator_id, reason, created_at, expires_at)`
- `guild_ban_records (id PK, guild_id, user_id, moderator_id, moderator_name, reason, created_at)`
- `guild_appeals (id PK, guild_id, user_id, user_name, user_avatar, moderator_id, moderator_name, ban_reason, appeal_text, status, summary, resolved_by, resolved_at, created_at, updated_at)`
- `guild_appeal_blocks (guild_id, user_id, reason, created_at)`

## Bootstrapping
- API and workers call `init_db()` on startup to ensure tables exist.
- Requires `DATABASE_URL` (e.g., `postgresql://postgres:postgres@localhost:5432/postgres`).

## Notes
- Keep heavy joins out of runtime paths.
- Future tables (guilds, channels) can be added in `backend/database/db.py`.
