from __future__ import annotations

import asyncio
from dataclasses import dataclass
from datetime import datetime, timezone
from typing import Optional

from psycopg_pool import AsyncConnectionPool

from backend.common.models import GuildSettings, QueueMessage


@dataclass
class Database:
    """Wrapper around a Postgres connection pool."""

    pool: AsyncConnectionPool

    async def close(self) -> None:
        await self.pool.close()


async def create_pool(database_url: str, min_size: int = 1, max_size: int = 10) -> Database:
    pool = AsyncConnectionPool(
        conninfo=database_url,
        min_size=min_size,
        max_size=max_size,
        timeout=10,
        open=True,
    )
    return Database(pool=pool)


async def init_db(db: Database) -> None:
    """Initialize required tables if they do not exist."""

    ddl = """
    CREATE TABLE IF NOT EXISTS guild_settings (
        guild_id TEXT PRIMARY KEY,
        prefix TEXT NOT NULL DEFAULT '!',
        moderation_enabled BOOLEAN NOT NULL DEFAULT TRUE,
        analytics_enabled BOOLEAN NOT NULL DEFAULT TRUE,
        sentiment_enabled BOOLEAN NOT NULL DEFAULT TRUE,
        updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
    );

    CREATE TABLE IF NOT EXISTS moderation_logs (
        id BIGSERIAL PRIMARY KEY,
        message_id TEXT,
        guild_id TEXT,
        channel_id TEXT,
        author_id TEXT,
        action TEXT,
        reason TEXT,
        created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
    );

    CREATE TABLE IF NOT EXISTS analytics_message_counts (
        time_bucket TIMESTAMPTZ NOT NULL,
        guild_id TEXT NOT NULL,
        count BIGINT NOT NULL DEFAULT 0,
        PRIMARY KEY (time_bucket, guild_id)
    );

    CREATE TABLE IF NOT EXISTS analytics_sentiment (
        day DATE NOT NULL,
        guild_id TEXT NOT NULL,
        sentiment TEXT NOT NULL,
        PRIMARY KEY (day, guild_id)
    );
    """
    async with db.pool.connection() as conn:
        await conn.execute(ddl)


async def fetch_guild_settings(db: Database, guild_id: str) -> GuildSettings:
    """Get settings; insert defaults if not present."""

    query = """
    INSERT INTO guild_settings (guild_id)
    VALUES (%(guild_id)s)
    ON CONFLICT (guild_id) DO NOTHING;

    SELECT guild_id, prefix, moderation_enabled, analytics_enabled, sentiment_enabled
    FROM guild_settings
    WHERE guild_id = %(guild_id)s;
    """
    async with db.pool.connection() as conn:
        async with conn.cursor() as cur:
            await cur.execute(query, {"guild_id": guild_id})
            row = await cur.fetchone()
            return GuildSettings(
                guild_id=row[0],
                prefix=row[1],
                moderation_enabled=row[2],
                analytics_enabled=row[3],
                sentiment_enabled=row[4],
            )


async def upsert_guild_settings(db: Database, settings: GuildSettings) -> GuildSettings:
    query = """
    INSERT INTO guild_settings (guild_id, prefix, moderation_enabled, analytics_enabled, sentiment_enabled, updated_at)
    VALUES (%(guild_id)s, %(prefix)s, %(moderation_enabled)s, %(analytics_enabled)s, %(sentiment_enabled)s, NOW())
    ON CONFLICT (guild_id)
    DO UPDATE SET
        prefix = EXCLUDED.prefix,
        moderation_enabled = EXCLUDED.moderation_enabled,
        analytics_enabled = EXCLUDED.analytics_enabled,
        sentiment_enabled = EXCLUDED.sentiment_enabled,
        updated_at = NOW();

    SELECT guild_id, prefix, moderation_enabled, analytics_enabled, sentiment_enabled
    FROM guild_settings WHERE guild_id = %(guild_id)s;
    """
    async with db.pool.connection() as conn:
        async with conn.cursor() as cur:
            await cur.execute(
                query,
                {
                    "guild_id": settings.guild_id,
                    "prefix": settings.prefix,
                    "moderation_enabled": settings.moderation_enabled,
                    "analytics_enabled": settings.analytics_enabled,
                    "sentiment_enabled": settings.sentiment_enabled,
                },
            )
            row = await cur.fetchone()
            return GuildSettings(
                guild_id=row[0],
                prefix=row[1],
                moderation_enabled=row[2],
                analytics_enabled=row[3],
                sentiment_enabled=row[4],
            )


async def log_moderation_event(
    db: Database, message: QueueMessage, action: str, reason: Optional[str] = None
) -> None:
    query = """
    INSERT INTO moderation_logs (message_id, guild_id, channel_id, author_id, action, reason)
    VALUES (%(message_id)s, %(guild_id)s, %(channel_id)s, %(author_id)s, %(action)s, %(reason)s);
    """
    async with db.pool.connection() as conn:
        async with conn.cursor() as cur:
            await cur.execute(
                query,
                {
                    "message_id": message.message_id,
                    "guild_id": message.guild_id,
                    "channel_id": message.channel_id,
                    "author_id": message.author_id,
                    "action": action,
                    "reason": reason,
                },
            )


async def bump_message_count(db: Database, message: QueueMessage) -> None:
    """Increment per-minute message counts."""

    try:
        timestamp = datetime.fromisoformat(message.timestamp)
    except ValueError:
        timestamp = datetime.now(timezone.utc)

    bucket = timestamp.replace(second=0, microsecond=0)
    query = """
    INSERT INTO analytics_message_counts (time_bucket, guild_id, count)
    VALUES (%(time_bucket)s, %(guild_id)s, 1)
    ON CONFLICT (time_bucket, guild_id) DO UPDATE
      SET count = analytics_message_counts.count + 1;
    """
    async with db.pool.connection() as conn:
        async with conn.cursor() as cur:
            await cur.execute(query, {"time_bucket": bucket, "guild_id": message.guild_id})


async def record_sentiment(db: Database, guild_id: str, day: datetime, sentiment: str) -> None:
    query = """
    INSERT INTO analytics_sentiment (day, guild_id, sentiment)
    VALUES (%(day)s, %(guild_id)s, %(sentiment)s)
    ON CONFLICT (day, guild_id) DO UPDATE SET sentiment = EXCLUDED.sentiment;
    """
    async with db.pool.connection() as conn:
        async with conn.cursor() as cur:
            await cur.execute(query, {"day": day.date(), "guild_id": guild_id, "sentiment": sentiment})


# Convenience helper for tests to wait for DB readiness.
async def wait_for_db(db: Database, timeout: float = 10.0) -> None:
    async def _ping() -> None:
        async with db.pool.connection() as conn:
            async with conn.cursor() as cur:
                await cur.execute("SELECT 1;")
                await cur.fetchone()

    await asyncio.wait_for(_ping(), timeout=timeout)
