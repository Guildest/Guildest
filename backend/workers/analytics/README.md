# Analytics Worker
- Tracks message volume, streaks, trends.
- Aggregates into 1m/1h/1d buckets.
- Writes rollups to Postgres tables:
  - analytics_message_counts(time_bucket, guild_id, count)
  - analytics_sentiment(day, guild_id, sentiment)
- Avoid per-user analytics to comply with Discord ToS.
