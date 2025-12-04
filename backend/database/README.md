# Database Layer (Postgres)
- Tables: guilds, guild_settings, channels, analytics_message_counts, analytics_sentiment, moderation_logs.
- Indexes: (guild_id, timestamp), (guild_id, channel_id), time_bucket for analytics queries.
- Keep heavy joins out of runtime paths.
