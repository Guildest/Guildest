from __future__ import annotations

import asyncio
import json
import uuid
from dataclasses import dataclass
from datetime import datetime, timedelta, timezone
from typing import Any, Optional

from psycopg_pool import AsyncConnectionPool

from backend.common.models import GuildSettings, QueueMessage


DEFAULT_WARN_POLICY = [
    {"threshold": 3, "action": "timeout", "duration_hours": 24},
    {"threshold": 5, "action": "ban"},
]


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

    ALTER TABLE guild_settings
      ADD COLUMN IF NOT EXISTS warn_decay_days INT NOT NULL DEFAULT 90;
    ALTER TABLE guild_settings
      ADD COLUMN IF NOT EXISTS warn_policy JSONB NOT NULL DEFAULT '[]'::jsonb;

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

    ALTER TABLE analytics_sentiment
      ADD COLUMN IF NOT EXISTS score DOUBLE PRECISION;

    CREATE TABLE IF NOT EXISTS users (
        user_id TEXT PRIMARY KEY,
        username TEXT NOT NULL,
        avatar TEXT,
        created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
        updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
    );

    ALTER TABLE users
      ADD COLUMN IF NOT EXISTS stripe_customer_id TEXT;

    CREATE UNIQUE INDEX IF NOT EXISTS users_stripe_customer_id_uq
      ON users (stripe_customer_id)
      WHERE stripe_customer_id IS NOT NULL;

    CREATE TABLE IF NOT EXISTS sessions (
        session_id UUID PRIMARY KEY,
        user_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
        expires_at TIMESTAMPTZ NOT NULL,
        created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
    );

    CREATE TABLE IF NOT EXISTS subscriptions (
        user_id TEXT PRIMARY KEY REFERENCES users(user_id) ON DELETE CASCADE,
        plan TEXT NOT NULL DEFAULT 'free',
        status TEXT NOT NULL DEFAULT 'active',
        stripe_subscription_id TEXT,
        stripe_price_id TEXT,
        current_period_end TIMESTAMPTZ,
        cancel_at_period_end BOOLEAN NOT NULL DEFAULT FALSE,
        updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
    );

    CREATE TABLE IF NOT EXISTS guild_accounts (
        guild_id TEXT PRIMARY KEY,
        billing_user_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
        connected_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
    );

    CREATE TABLE IF NOT EXISTS user_guilds (
        user_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
        guild_id TEXT NOT NULL,
        guild_name TEXT,
        guild_icon TEXT,
        is_owner BOOLEAN NOT NULL DEFAULT FALSE,
        permissions BIGINT NOT NULL DEFAULT 0,
        updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
        PRIMARY KEY (user_id, guild_id)
    );

    CREATE TABLE IF NOT EXISTS guild_warns (
        id BIGSERIAL PRIMARY KEY,
        guild_id TEXT NOT NULL,
        user_id TEXT NOT NULL,
        moderator_id TEXT NOT NULL,
        reason TEXT,
        created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
        expires_at TIMESTAMPTZ
    );

    CREATE INDEX IF NOT EXISTS guild_warns_lookup_idx
      ON guild_warns (guild_id, user_id, expires_at);

    CREATE TABLE IF NOT EXISTS guild_ban_records (
        id BIGSERIAL PRIMARY KEY,
        guild_id TEXT NOT NULL,
        user_id TEXT NOT NULL,
        moderator_id TEXT NOT NULL,
        moderator_name TEXT,
        reason TEXT,
        created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
    );

    CREATE INDEX IF NOT EXISTS guild_ban_records_lookup_idx
      ON guild_ban_records (guild_id, user_id, created_at DESC);

    CREATE TABLE IF NOT EXISTS guild_appeals (
        id UUID PRIMARY KEY,
        guild_id TEXT NOT NULL,
        user_id TEXT NOT NULL,
        user_name TEXT,
        user_avatar TEXT,
        moderator_id TEXT,
        moderator_name TEXT,
        ban_reason TEXT,
        appeal_text TEXT NOT NULL,
        status TEXT NOT NULL DEFAULT 'open',
        summary TEXT,
        resolved_by TEXT,
        resolved_at TIMESTAMPTZ,
        created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
        updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
    );

    CREATE INDEX IF NOT EXISTS guild_appeals_lookup_idx
      ON guild_appeals (guild_id, status, created_at DESC);

    CREATE TABLE IF NOT EXISTS guild_appeal_blocks (
        guild_id TEXT NOT NULL,
        user_id TEXT NOT NULL,
        reason TEXT,
        created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
        PRIMARY KEY (guild_id, user_id)
    );

    CREATE TABLE IF NOT EXISTS sentiment_daily_samples (
        id BIGSERIAL PRIMARY KEY,
        guild_id TEXT NOT NULL,
        day DATE NOT NULL,
        content TEXT NOT NULL,
        created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
    );

    CREATE INDEX IF NOT EXISTS sentiment_daily_samples_guild_day_idx
      ON sentiment_daily_samples (guild_id, day);

    CREATE TABLE IF NOT EXISTS sentiment_daily_reports (
        guild_id TEXT NOT NULL,
        day DATE NOT NULL,
        model TEXT NOT NULL,
        report JSONB NOT NULL,
        created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
        updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
        PRIMARY KEY (guild_id, day)
    );
    """
    async with db.pool.connection() as conn:
        await conn.execute(ddl)


async def fetch_user_stripe_customer_id(db: Database, user_id: str) -> Optional[str]:
    query = "SELECT stripe_customer_id FROM users WHERE user_id = %(user_id)s;"
    async with db.pool.connection() as conn:
        async with conn.cursor() as cur:
            await cur.execute(query, {"user_id": user_id})
            row = await cur.fetchone()
            return row[0] if row and row[0] else None


async def set_user_stripe_customer_id(db: Database, user_id: str, customer_id: str) -> None:
    query = """
    UPDATE users
    SET stripe_customer_id = %(customer_id)s,
        updated_at = NOW()
    WHERE user_id = %(user_id)s;
    """
    async with db.pool.connection() as conn:
        async with conn.cursor() as cur:
            await cur.execute(query, {"user_id": user_id, "customer_id": customer_id})


async def fetch_user_id_by_stripe_customer_id(db: Database, customer_id: str) -> Optional[str]:
    query = "SELECT user_id FROM users WHERE stripe_customer_id = %(customer_id)s;"
    async with db.pool.connection() as conn:
        async with conn.cursor() as cur:
            await cur.execute(query, {"customer_id": customer_id})
            row = await cur.fetchone()
            return row[0] if row else None


async def fetch_guild_settings(db: Database, guild_id: str) -> GuildSettings:
    """Get settings; insert defaults if not present."""

    insert_query = """
    INSERT INTO guild_settings (guild_id)
    VALUES (%(guild_id)s)
    ON CONFLICT (guild_id) DO NOTHING;
    """
    select_query = """
    SELECT guild_id,
           prefix,
           moderation_enabled,
           analytics_enabled,
           sentiment_enabled,
           warn_decay_days,
           warn_policy
    FROM guild_settings
    WHERE guild_id = %(guild_id)s;
    """
    async with db.pool.connection() as conn:
        async with conn.cursor() as cur:
            await cur.execute(insert_query, {"guild_id": guild_id})
            await cur.execute(select_query, {"guild_id": guild_id})
            row = await cur.fetchone()
            warn_policy = row[6] or []
            if isinstance(warn_policy, str):
                try:
                    warn_policy = json.loads(warn_policy)
                except json.JSONDecodeError:
                    warn_policy = []
            if not warn_policy:
                warn_policy = DEFAULT_WARN_POLICY
            return GuildSettings(
                guild_id=row[0],
                prefix=row[1],
                moderation_enabled=row[2],
                analytics_enabled=row[3],
                sentiment_enabled=row[4],
                warn_decay_days=row[5],
                warn_policy=warn_policy,
            )


async def upsert_guild_settings(db: Database, settings: GuildSettings) -> GuildSettings:
    upsert_query = """
    INSERT INTO guild_settings (
        guild_id,
        prefix,
        moderation_enabled,
        analytics_enabled,
        sentiment_enabled,
        warn_decay_days,
        warn_policy,
        updated_at
    )
    VALUES (
        %(guild_id)s,
        %(prefix)s,
        %(moderation_enabled)s,
        %(analytics_enabled)s,
        %(sentiment_enabled)s,
        %(warn_decay_days)s,
        %(warn_policy)s::jsonb,
        NOW()
    )
    ON CONFLICT (guild_id)
    DO UPDATE SET
        prefix = EXCLUDED.prefix,
        moderation_enabled = EXCLUDED.moderation_enabled,
        analytics_enabled = EXCLUDED.analytics_enabled,
        sentiment_enabled = EXCLUDED.sentiment_enabled,
        warn_decay_days = EXCLUDED.warn_decay_days,
        warn_policy = EXCLUDED.warn_policy,
        updated_at = NOW();
    """
    select_query = """
    SELECT guild_id,
           prefix,
           moderation_enabled,
           analytics_enabled,
           sentiment_enabled,
           warn_decay_days,
           warn_policy
    FROM guild_settings WHERE guild_id = %(guild_id)s;
    """
    async with db.pool.connection() as conn:
        async with conn.cursor() as cur:
            await cur.execute(
                upsert_query,
                {
                    "guild_id": settings.guild_id,
                    "prefix": settings.prefix,
                    "moderation_enabled": settings.moderation_enabled,
                    "analytics_enabled": settings.analytics_enabled,
                    "sentiment_enabled": settings.sentiment_enabled,
                    "warn_decay_days": settings.warn_decay_days,
                    "warn_policy": json.dumps([item.dict() for item in settings.warn_policy]),
                },
            )
            await cur.execute(select_query, {"guild_id": settings.guild_id})
            row = await cur.fetchone()
            warn_policy = row[6] or []
            if isinstance(warn_policy, str):
                try:
                    warn_policy = json.loads(warn_policy)
                except json.JSONDecodeError:
                    warn_policy = []
            if not warn_policy:
                warn_policy = DEFAULT_WARN_POLICY
            return GuildSettings(
                guild_id=row[0],
                prefix=row[1],
                moderation_enabled=row[2],
                analytics_enabled=row[3],
                sentiment_enabled=row[4],
                warn_decay_days=row[5],
                warn_policy=warn_policy,
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


async def insert_guild_warn(
    db: Database,
    guild_id: str,
    user_id: str,
    moderator_id: str,
    reason: Optional[str],
    expires_at: Optional[datetime],
) -> None:
    query = """
    INSERT INTO guild_warns (guild_id, user_id, moderator_id, reason, expires_at)
    VALUES (%(guild_id)s, %(user_id)s, %(moderator_id)s, %(reason)s, %(expires_at)s);
    """
    async with db.pool.connection() as conn:
        async with conn.cursor() as cur:
            await cur.execute(
                query,
                {
                    "guild_id": guild_id,
                    "user_id": user_id,
                    "moderator_id": moderator_id,
                    "reason": reason,
                    "expires_at": expires_at,
                },
            )


async def fetch_active_warns(db: Database, guild_id: str, user_id: str, now: datetime) -> list[dict[str, Any]]:
    query = """
    SELECT id, moderator_id, reason, created_at, expires_at
    FROM guild_warns
    WHERE guild_id = %(guild_id)s
      AND user_id = %(user_id)s
      AND (expires_at IS NULL OR expires_at > %(now)s)
    ORDER BY created_at DESC;
    """
    async with db.pool.connection() as conn:
        async with conn.cursor() as cur:
            await cur.execute(query, {"guild_id": guild_id, "user_id": user_id, "now": now})
            rows = await cur.fetchall()
            return [
                {
                    "id": row[0],
                    "moderator_id": row[1],
                    "reason": row[2],
                    "created_at": row[3],
                    "expires_at": row[4],
                }
                for row in rows
            ]


async def clear_warns(db: Database, guild_id: str, user_id: str) -> int:
    query = "DELETE FROM guild_warns WHERE guild_id = %(guild_id)s AND user_id = %(user_id)s;"
    async with db.pool.connection() as conn:
        async with conn.cursor() as cur:
            await cur.execute(query, {"guild_id": guild_id, "user_id": user_id})
            return cur.rowcount or 0


async def record_ban_action(
    db: Database,
    guild_id: str,
    user_id: str,
    moderator_id: str,
    moderator_name: Optional[str],
    reason: Optional[str],
) -> None:
    query = """
    INSERT INTO guild_ban_records (guild_id, user_id, moderator_id, moderator_name, reason)
    VALUES (%(guild_id)s, %(user_id)s, %(moderator_id)s, %(moderator_name)s, %(reason)s);
    """
    async with db.pool.connection() as conn:
        async with conn.cursor() as cur:
            await cur.execute(
                query,
                {
                    "guild_id": guild_id,
                    "user_id": user_id,
                    "moderator_id": moderator_id,
                    "moderator_name": moderator_name,
                    "reason": reason,
                },
            )


async def fetch_latest_ban_record(db: Database, guild_id: str, user_id: str) -> Optional[dict[str, Any]]:
    query = """
    SELECT moderator_id, moderator_name, reason, created_at
    FROM guild_ban_records
    WHERE guild_id = %(guild_id)s AND user_id = %(user_id)s
    ORDER BY created_at DESC
    LIMIT 1;
    """
    async with db.pool.connection() as conn:
        async with conn.cursor() as cur:
            await cur.execute(query, {"guild_id": guild_id, "user_id": user_id})
            row = await cur.fetchone()
            if not row:
                return None
            return {
                "moderator_id": row[0],
                "moderator_name": row[1],
                "reason": row[2],
                "created_at": row[3],
            }


async def create_appeal(
    db: Database,
    appeal_id: uuid.UUID,
    guild_id: str,
    user_id: str,
    user_name: Optional[str],
    user_avatar: Optional[str],
    moderator_id: Optional[str],
    moderator_name: Optional[str],
    ban_reason: Optional[str],
    appeal_text: str,
) -> dict[str, Any]:
    query = """
    INSERT INTO guild_appeals (
        id,
        guild_id,
        user_id,
        user_name,
        user_avatar,
        moderator_id,
        moderator_name,
        ban_reason,
        appeal_text
    )
    VALUES (
        %(id)s::uuid,
        %(guild_id)s,
        %(user_id)s,
        %(user_name)s,
        %(user_avatar)s,
        %(moderator_id)s,
        %(moderator_name)s,
        %(ban_reason)s,
        %(appeal_text)s
    )
    RETURNING id, status, created_at;
    """
    async with db.pool.connection() as conn:
        async with conn.cursor() as cur:
            await cur.execute(
                query,
                {
                    "id": appeal_id,
                    "guild_id": guild_id,
                    "user_id": user_id,
                    "user_name": user_name,
                    "user_avatar": user_avatar,
                    "moderator_id": moderator_id,
                    "moderator_name": moderator_name,
                    "ban_reason": ban_reason,
                    "appeal_text": appeal_text,
                },
            )
            row = await cur.fetchone()
            return {"id": str(row[0]), "status": row[1], "created_at": row[2]}


async def fetch_guild_appeals(db: Database, guild_id: str, limit: int = 200) -> list[dict[str, Any]]:
    query = """
    SELECT id,
           user_id,
           user_name,
           user_avatar,
           moderator_id,
           moderator_name,
           ban_reason,
           appeal_text,
           status,
           summary,
           resolved_by,
           resolved_at,
           created_at,
           updated_at
    FROM guild_appeals
    WHERE guild_id = %(guild_id)s
    ORDER BY created_at DESC
    LIMIT %(limit)s;
    """
    async with db.pool.connection() as conn:
        async with conn.cursor() as cur:
            await cur.execute(query, {"guild_id": guild_id, "limit": limit})
            rows = await cur.fetchall()
            return [
                {
                    "id": str(row[0]),
                    "user_id": row[1],
                    "user_name": row[2],
                    "user_avatar": row[3],
                    "moderator_id": row[4],
                    "moderator_name": row[5],
                    "ban_reason": row[6],
                    "appeal_text": row[7],
                    "status": row[8],
                    "summary": row[9],
                    "resolved_by": row[10],
                    "resolved_at": row[11],
                    "created_at": row[12],
                    "updated_at": row[13],
                }
                for row in rows
            ]


async def fetch_appeal(db: Database, appeal_id: str) -> Optional[dict[str, Any]]:
    query = """
    SELECT id,
           guild_id,
           user_id,
           user_name,
           user_avatar,
           moderator_id,
           moderator_name,
           ban_reason,
           appeal_text,
           status,
           summary,
           resolved_by,
           resolved_at,
           created_at,
           updated_at
    FROM guild_appeals
    WHERE id = %(appeal_id)s::uuid;
    """
    async with db.pool.connection() as conn:
        async with conn.cursor() as cur:
            await cur.execute(query, {"appeal_id": appeal_id})
            row = await cur.fetchone()
            if not row:
                return None
            return {
                "id": str(row[0]),
                "guild_id": row[1],
                "user_id": row[2],
                "user_name": row[3],
                "user_avatar": row[4],
                "moderator_id": row[5],
                "moderator_name": row[6],
                "ban_reason": row[7],
                "appeal_text": row[8],
                "status": row[9],
                "summary": row[10],
                "resolved_by": row[11],
                "resolved_at": row[12],
                "created_at": row[13],
                "updated_at": row[14],
            }


async def set_appeal_status(
    db: Database,
    appeal_id: str,
    status: str,
    resolved_by: Optional[str],
) -> None:
    query = """
    UPDATE guild_appeals
    SET status = %(status)s,
        resolved_by = %(resolved_by)s,
        resolved_at = NOW(),
        updated_at = NOW()
    WHERE id = %(appeal_id)s::uuid;
    """
    async with db.pool.connection() as conn:
        async with conn.cursor() as cur:
            await cur.execute(
                query,
                {"appeal_id": appeal_id, "status": status, "resolved_by": resolved_by},
            )


async def set_appeal_summary(db: Database, appeal_id: str, summary: str) -> None:
    query = """
    UPDATE guild_appeals
    SET summary = %(summary)s,
        updated_at = NOW()
    WHERE id = %(appeal_id)s::uuid;
    """
    async with db.pool.connection() as conn:
        async with conn.cursor() as cur:
            await cur.execute(query, {"appeal_id": appeal_id, "summary": summary})


async def is_appeal_blocked(db: Database, guild_id: str, user_id: str) -> bool:
    query = "SELECT 1 FROM guild_appeal_blocks WHERE guild_id = %(guild_id)s AND user_id = %(user_id)s;"
    async with db.pool.connection() as conn:
        async with conn.cursor() as cur:
            await cur.execute(query, {"guild_id": guild_id, "user_id": user_id})
            return (await cur.fetchone()) is not None


async def block_appeals(db: Database, guild_id: str, user_id: str, reason: Optional[str]) -> None:
    query = """
    INSERT INTO guild_appeal_blocks (guild_id, user_id, reason)
    VALUES (%(guild_id)s, %(user_id)s, %(reason)s)
    ON CONFLICT (guild_id, user_id) DO UPDATE
      SET reason = EXCLUDED.reason,
          created_at = NOW();
    """
    async with db.pool.connection() as conn:
        async with conn.cursor() as cur:
            await cur.execute(query, {"guild_id": guild_id, "user_id": user_id, "reason": reason})

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


async def record_sentiment_score(db: Database, guild_id: str, day: datetime, sentiment: str, score: float) -> None:
    query = """
    INSERT INTO analytics_sentiment (day, guild_id, sentiment, score)
    VALUES (%(day)s, %(guild_id)s, %(sentiment)s, %(score)s)
    ON CONFLICT (day, guild_id) DO UPDATE
      SET sentiment = EXCLUDED.sentiment,
          score = EXCLUDED.score;
    """
    async with db.pool.connection() as conn:
        async with conn.cursor() as cur:
            await cur.execute(
                query,
                {"day": day.date(), "guild_id": guild_id, "sentiment": sentiment, "score": score},
            )


async def upsert_user(db: Database, user_id: str, username: str, avatar: Optional[str]) -> None:
    query = """
    INSERT INTO users (user_id, username, avatar, updated_at)
    VALUES (%(user_id)s, %(username)s, %(avatar)s, NOW())
    ON CONFLICT (user_id) DO UPDATE
      SET username = EXCLUDED.username,
          avatar = EXCLUDED.avatar,
          updated_at = NOW();
    """
    async with db.pool.connection() as conn:
        async with conn.cursor() as cur:
            await cur.execute(query, {"user_id": user_id, "username": username, "avatar": avatar})


async def fetch_user_profile(db: Database, user_id: str) -> Optional[dict[str, Any]]:
    query = "SELECT username, avatar FROM users WHERE user_id = %(user_id)s;"
    async with db.pool.connection() as conn:
        async with conn.cursor() as cur:
            await cur.execute(query, {"user_id": user_id})
            row = await cur.fetchone()
            if not row:
                return None
            return {"username": row[0], "avatar": row[1]}


async def ensure_subscription_row(db: Database, user_id: str) -> None:
    query = """
    INSERT INTO subscriptions (user_id)
    VALUES (%(user_id)s)
    ON CONFLICT (user_id) DO NOTHING;
    """
    async with db.pool.connection() as conn:
        async with conn.cursor() as cur:
            await cur.execute(query, {"user_id": user_id})


async def set_subscription_plan(db: Database, user_id: str, plan: str, status: str = "active") -> None:
    query = """
    INSERT INTO subscriptions (user_id, plan, status, updated_at)
    VALUES (%(user_id)s, %(plan)s, %(status)s, NOW())
    ON CONFLICT (user_id) DO UPDATE
      SET plan = EXCLUDED.plan,
          status = EXCLUDED.status,
          updated_at = NOW();
    """
    async with db.pool.connection() as conn:
        async with conn.cursor() as cur:
            await cur.execute(query, {"user_id": user_id, "plan": plan, "status": status})


async def fetch_subscription_plan(db: Database, user_id: str) -> str:
    await ensure_subscription_row(db, user_id)
    query = "SELECT plan FROM subscriptions WHERE user_id = %(user_id)s;"
    async with db.pool.connection() as conn:
        async with conn.cursor() as cur:
            await cur.execute(query, {"user_id": user_id})
            row = await cur.fetchone()
            return row[0] if row else "free"


async def fetch_subscription(db: Database, user_id: str) -> dict[str, Any]:
    await ensure_subscription_row(db, user_id)
    query = """
    SELECT plan, status, stripe_subscription_id, stripe_price_id, current_period_end, cancel_at_period_end
    FROM subscriptions
    WHERE user_id = %(user_id)s;
    """
    async with db.pool.connection() as conn:
        async with conn.cursor() as cur:
            await cur.execute(query, {"user_id": user_id})
            row = await cur.fetchone()
            if not row:
                return {
                    "plan": "free",
                    "status": "active",
                    "stripe_subscription_id": None,
                    "stripe_price_id": None,
                    "current_period_end": None,
                    "cancel_at_period_end": False,
                }
            return {
                "plan": row[0],
                "status": row[1],
                "stripe_subscription_id": row[2],
                "stripe_price_id": row[3],
                "current_period_end": row[4],
                "cancel_at_period_end": row[5],
            }


async def upsert_stripe_subscription(
    db: Database,
    user_id: str,
    *,
    plan: str,
    status: str,
    stripe_subscription_id: Optional[str],
    stripe_price_id: Optional[str],
    current_period_end: Optional[datetime],
    cancel_at_period_end: bool,
) -> None:
    await ensure_subscription_row(db, user_id)
    query = """
    INSERT INTO subscriptions (
        user_id, plan, status, stripe_subscription_id, stripe_price_id, current_period_end, cancel_at_period_end, updated_at
    )
    VALUES (
        %(user_id)s, %(plan)s, %(status)s, %(stripe_subscription_id)s, %(stripe_price_id)s, %(current_period_end)s, %(cancel_at_period_end)s, NOW()
    )
    ON CONFLICT (user_id) DO UPDATE
      SET plan = EXCLUDED.plan,
          status = EXCLUDED.status,
          stripe_subscription_id = EXCLUDED.stripe_subscription_id,
          stripe_price_id = EXCLUDED.stripe_price_id,
          current_period_end = EXCLUDED.current_period_end,
          cancel_at_period_end = EXCLUDED.cancel_at_period_end,
          updated_at = NOW();
    """
    async with db.pool.connection() as conn:
        async with conn.cursor() as cur:
            await cur.execute(
                query,
                {
                    "user_id": user_id,
                    "plan": plan,
                    "status": status,
                    "stripe_subscription_id": stripe_subscription_id,
                    "stripe_price_id": stripe_price_id,
                    "current_period_end": current_period_end,
                    "cancel_at_period_end": cancel_at_period_end,
                },
            )


async def create_session(db: Database, user_id: str, ttl_days: int = 7) -> tuple[str, datetime]:
    session_id = str(uuid.uuid4())
    expires_at = datetime.now(timezone.utc) + timedelta(days=ttl_days)
    query = """
    INSERT INTO sessions (session_id, user_id, expires_at)
    VALUES (%(session_id)s::uuid, %(user_id)s, %(expires_at)s);
    """
    async with db.pool.connection() as conn:
        async with conn.cursor() as cur:
            await cur.execute(query, {"session_id": session_id, "user_id": user_id, "expires_at": expires_at})
    return session_id, expires_at


async def delete_session(db: Database, session_id: str) -> None:
    query = "DELETE FROM sessions WHERE session_id = %(session_id)s::uuid;"
    async with db.pool.connection() as conn:
        async with conn.cursor() as cur:
            await cur.execute(query, {"session_id": session_id})


async def fetch_session_user_id(db: Database, session_id: str) -> Optional[str]:
    query = """
    SELECT user_id
    FROM sessions
    WHERE session_id = %(session_id)s::uuid AND expires_at > NOW();
    """
    async with db.pool.connection() as conn:
        async with conn.cursor() as cur:
            await cur.execute(query, {"session_id": session_id})
            row = await cur.fetchone()
            return row[0] if row else None


async def upsert_user_guilds(db: Database, user_id: str, guilds: list[dict[str, Any]]) -> None:
    query = """
    INSERT INTO user_guilds (user_id, guild_id, guild_name, guild_icon, is_owner, permissions, updated_at)
    VALUES (%(user_id)s, %(guild_id)s, %(guild_name)s, %(guild_icon)s, %(is_owner)s, %(permissions)s, NOW())
    ON CONFLICT (user_id, guild_id) DO UPDATE
      SET guild_name = EXCLUDED.guild_name,
          guild_icon = EXCLUDED.guild_icon,
          is_owner = EXCLUDED.is_owner,
          permissions = EXCLUDED.permissions,
          updated_at = NOW();
    """
    async with db.pool.connection() as conn:
        async with conn.cursor() as cur:
            for guild in guilds:
                await cur.execute(
                    query,
                    {
                        "user_id": user_id,
                        "guild_id": str(guild.get("id")),
                        "guild_name": guild.get("name"),
                        "guild_icon": guild.get("icon"),
                        "is_owner": bool(guild.get("owner")),
                        "permissions": int(guild.get("permissions", 0) or 0),
                    },
                )


async def user_can_access_guild(db: Database, user_id: str, guild_id: str) -> bool:
    query = "SELECT 1 FROM user_guilds WHERE user_id = %(user_id)s AND guild_id = %(guild_id)s;"
    async with db.pool.connection() as conn:
        async with conn.cursor() as cur:
            await cur.execute(query, {"user_id": user_id, "guild_id": guild_id})
            return (await cur.fetchone()) is not None


async def fetch_user_guild_access(db: Database, user_id: str, guild_id: str) -> Optional[dict[str, Any]]:
    query = """
    SELECT is_owner, permissions
    FROM user_guilds
    WHERE user_id = %(user_id)s AND guild_id = %(guild_id)s;
    """
    async with db.pool.connection() as conn:
        async with conn.cursor() as cur:
            await cur.execute(query, {"user_id": user_id, "guild_id": guild_id})
            row = await cur.fetchone()
            if not row:
                return None
            return {"is_owner": bool(row[0]), "permissions": int(row[1] or 0)}


def user_has_manage_guild_permissions(is_owner: bool, permissions: int) -> bool:
    # Discord permission bits: https://discord.com/developers/docs/topics/permissions
    ADMINISTRATOR = 0x00000008
    MANAGE_GUILD = 0x00000020
    return bool(is_owner) or bool(permissions & ADMINISTRATOR) or bool(permissions & MANAGE_GUILD)


async def connect_guild_to_user(db: Database, guild_id: str, billing_user_id: str) -> None:
    query = """
    INSERT INTO guild_accounts (guild_id, billing_user_id, connected_at)
    VALUES (%(guild_id)s, %(billing_user_id)s, NOW())
    ON CONFLICT (guild_id) DO UPDATE
      SET billing_user_id = EXCLUDED.billing_user_id,
          connected_at = NOW();
    """
    async with db.pool.connection() as conn:
        async with conn.cursor() as cur:
            await cur.execute(query, {"guild_id": guild_id, "billing_user_id": billing_user_id})


async def delete_guild_connection(db: Database, guild_id: str, billing_user_id: str) -> None:
    query = """
    DELETE FROM guild_accounts
    WHERE guild_id = %(guild_id)s AND billing_user_id = %(billing_user_id)s;
    """
    async with db.pool.connection() as conn:
        async with conn.cursor() as cur:
            await cur.execute(query, {"guild_id": guild_id, "billing_user_id": billing_user_id})


async def fetch_guild_billing_user_id(db: Database, guild_id: str) -> Optional[str]:
    query = "SELECT billing_user_id FROM guild_accounts WHERE guild_id = %(guild_id)s;"
    async with db.pool.connection() as conn:
        async with conn.cursor() as cur:
            await cur.execute(query, {"guild_id": guild_id})
            row = await cur.fetchone()
            return row[0] if row else None


async def fetch_guild_plan(db: Database, guild_id: str) -> str:
    billing_user_id = await fetch_guild_billing_user_id(db, guild_id)
    if not billing_user_id:
        return "free"
    return await fetch_subscription_plan(db, billing_user_id)


async def fetch_user_connected_guild_ids(db: Database, user_id: str) -> set[str]:
    query = "SELECT guild_id FROM guild_accounts WHERE billing_user_id = %(user_id)s;"
    async with db.pool.connection() as conn:
        async with conn.cursor() as cur:
            await cur.execute(query, {"user_id": user_id})
            rows = await cur.fetchall()
            return {row[0] for row in rows}


async def insert_sentiment_sample(db: Database, guild_id: str, day: datetime, content: str) -> None:
    query = """
    INSERT INTO sentiment_daily_samples (guild_id, day, content)
    VALUES (%(guild_id)s, %(day)s, %(content)s);
    """
    async with db.pool.connection() as conn:
        async with conn.cursor() as cur:
            await cur.execute(query, {"guild_id": guild_id, "day": day.date(), "content": content})


async def fetch_sentiment_samples(db: Database, guild_id: str, day: datetime, limit: int = 200) -> list[str]:
    query = """
    SELECT content
    FROM sentiment_daily_samples
    WHERE guild_id = %(guild_id)s AND day = %(day)s
    ORDER BY id DESC
    LIMIT %(limit)s;
    """
    async with db.pool.connection() as conn:
        async with conn.cursor() as cur:
            await cur.execute(
                query, {"guild_id": guild_id, "day": day.date(), "limit": limit}
            )
            rows = await cur.fetchall()
            return [row[0] for row in rows]


async def upsert_sentiment_report(
    db: Database, guild_id: str, day: datetime, model: str, report: dict[str, Any]
) -> None:
    query = """
    INSERT INTO sentiment_daily_reports (guild_id, day, model, report, created_at, updated_at)
    VALUES (%(guild_id)s, %(day)s, %(model)s, %(report)s::jsonb, NOW(), NOW())
    ON CONFLICT (guild_id, day) DO UPDATE
      SET model = EXCLUDED.model,
          report = EXCLUDED.report,
          updated_at = NOW();
    """
    async with db.pool.connection() as conn:
        async with conn.cursor() as cur:
            await cur.execute(
                query,
                {"guild_id": guild_id, "day": day.date(), "model": model, "report": json.dumps(report)},
            )


async def fetch_sentiment_report(db: Database, guild_id: str, day: datetime) -> Optional[dict[str, Any]]:
    query = """
    SELECT report
    FROM sentiment_daily_reports
    WHERE guild_id = %(guild_id)s AND day = %(day)s;
    """
    async with db.pool.connection() as conn:
        async with conn.cursor() as cur:
            await cur.execute(query, {"guild_id": guild_id, "day": day.date()})
            row = await cur.fetchone()
            return row[0] if row else None


async def fetch_user_guilds(db: Database, user_id: str, limit: int = 200) -> list[dict[str, Any]]:
    query = """
    SELECT guild_id, guild_name, guild_icon, is_owner, permissions
    FROM user_guilds
    WHERE user_id = %(user_id)s
    ORDER BY updated_at DESC
    LIMIT %(limit)s;
    """
    async with db.pool.connection() as conn:
        async with conn.cursor() as cur:
            await cur.execute(query, {"user_id": user_id, "limit": limit})
            rows = await cur.fetchall()
            return [
                {
                    "guild_id": row[0],
                    "name": row[1],
                    "icon": row[2],
                    "is_owner": row[3],
                    "permissions": int(row[4] or 0),
                }
                for row in rows
            ]


async def fetch_message_counts(
    db: Database, guild_id: str, from_ts: datetime, to_ts: datetime, limit: int = 2000
) -> list[dict[str, Any]]:
    query = """
    SELECT time_bucket, count
    FROM analytics_message_counts
    WHERE guild_id = %(guild_id)s
      AND time_bucket >= %(from_ts)s
      AND time_bucket <= %(to_ts)s
    ORDER BY time_bucket ASC
    LIMIT %(limit)s;
    """
    async with db.pool.connection() as conn:
        async with conn.cursor() as cur:
            await cur.execute(
                query,
                {"guild_id": guild_id, "from_ts": from_ts, "to_ts": to_ts, "limit": limit},
            )
            rows = await cur.fetchall()
            return [{"time_bucket": row[0].isoformat(), "count": int(row[1])} for row in rows]


async def fetch_sentiment_daily(
    db: Database, guild_id: str, from_day: datetime, to_day: datetime, limit: int = 366
) -> list[dict[str, Any]]:
    query = """
    SELECT day, sentiment, score
    FROM analytics_sentiment
    WHERE guild_id = %(guild_id)s
      AND day >= %(from_day)s
      AND day <= %(to_day)s
    ORDER BY day ASC
    LIMIT %(limit)s;
    """
    async with db.pool.connection() as conn:
        async with conn.cursor() as cur:
            await cur.execute(
                query,
                {"guild_id": guild_id, "from_day": from_day.date(), "to_day": to_day.date(), "limit": limit},
            )
            rows = await cur.fetchall()
            return [
                {"day": row[0].isoformat(), "sentiment": row[1], "score": row[2]}
                for row in rows
            ]


async def fetch_moderation_logs(db: Database, guild_id: str, limit: int = 200) -> list[dict[str, Any]]:
    query = """
    SELECT id, message_id, channel_id, author_id, action, reason, created_at
    FROM moderation_logs
    WHERE guild_id = %(guild_id)s
    ORDER BY id DESC
    LIMIT %(limit)s;
    """
    async with db.pool.connection() as conn:
        async with conn.cursor() as cur:
            await cur.execute(query, {"guild_id": guild_id, "limit": limit})
            rows = await cur.fetchall()
            return [
                {
                    "id": int(row[0]),
                    "message_id": row[1],
                    "channel_id": row[2],
                    "author_id": row[3],
                    "action": row[4],
                    "reason": row[5],
                    "created_at": row[6].isoformat(),
                }
                for row in rows
            ]


async def fetch_recent_moderation_count(db: Database, guild_id: str, since: datetime) -> int:
    query = """
    SELECT COUNT(*)
    FROM moderation_logs
    WHERE guild_id = %(guild_id)s
      AND created_at >= %(since)s;
    """
    async with db.pool.connection() as conn:
        async with conn.cursor() as cur:
            await cur.execute(query, {"guild_id": guild_id, "since": since})
            row = await cur.fetchone()
            return int(row[0] or 0)


async def fetch_message_count_sum(db: Database, guild_id: str, from_ts: datetime, to_ts: datetime) -> int:
    query = """
    SELECT COALESCE(SUM(count), 0)
    FROM analytics_message_counts
    WHERE guild_id = %(guild_id)s
      AND time_bucket >= %(from_ts)s
      AND time_bucket <= %(to_ts)s;
    """
    async with db.pool.connection() as conn:
        async with conn.cursor() as cur:
            await cur.execute(query, {"guild_id": guild_id, "from_ts": from_ts, "to_ts": to_ts})
            row = await cur.fetchone()
            return int(row[0] or 0)


async def fetch_latest_sentiment(db: Database, guild_id: str) -> Optional[dict[str, Any]]:
    query = """
    SELECT day, sentiment, score
    FROM analytics_sentiment
    WHERE guild_id = %(guild_id)s
    ORDER BY day DESC
    LIMIT 1;
    """
    async with db.pool.connection() as conn:
        async with conn.cursor() as cur:
            await cur.execute(query, {"guild_id": guild_id})
            row = await cur.fetchone()
            if not row:
                return None
            return {"day": row[0].isoformat(), "sentiment": row[1], "score": row[2]}


# Convenience helper for tests to wait for DB readiness.
async def wait_for_db(db: Database, timeout: float = 10.0) -> None:
    async def _ping() -> None:
        async with db.pool.connection() as conn:
            async with conn.cursor() as cur:
                await cur.execute("SELECT 1;")
                await cur.fetchone()

    await asyncio.wait_for(_ping(), timeout=timeout)
