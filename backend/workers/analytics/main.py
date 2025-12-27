import asyncio
import json
import logging
from dataclasses import dataclass
from datetime import date, datetime, timedelta, timezone
from typing import Any, Dict, Optional, Tuple

import redis.asyncio as redis

from backend.common.config import AppConfig, load_app_config
from backend.common.logging import configure_logging
from backend.database.db import (
    Database,
    GUILD_CHANNEL_ID,
    create_pool,
    fetch_guild_settings,
    init_db,
    upsert_command_bucket_deltas,
    upsert_daily_summary_deltas,
    upsert_message_bucket_deltas,
    upsert_message_count_deltas,
    upsert_voice_bucket_deltas,
)


BUCKET_SIZES = (60, 3600, 86400)
FLUSH_INTERVAL_SECONDS = 2.0
MAX_PENDING = 2000
MAX_BUFFER_KEYS = 5000
HLL_TTL_SECONDS = 60 * 60 * 24 * 45
ACTIVE_SET_TTL_SECONDS = 60 * 60 * 24 * 45
VOICE_SESSION_TTL_SECONDS = 60 * 60 * 12
VOICE_CONCURRENT_TTL_SECONDS = 60 * 60 * 12

BucketKey = Tuple[datetime, int, str, Optional[str]]
CommandKey = Tuple[datetime, int, str, str]
DailyKey = Tuple[str, date]
MinuteKey = Tuple[datetime, str]


@dataclass
class MsgDelta:
    message_count: int = 0
    delete_count: int = 0
    attachment_count: int = 0
    total_content_len: int = 0


@dataclass
class VoiceDelta:
    join_count: int = 0
    leave_count: int = 0
    total_seconds: int = 0
    peak_concurrent: int = 0


@dataclass
class DailyDelta:
    messages: int = 0
    voice_minutes: int = 0


class Aggregator:
    def __init__(self) -> None:
        self.message: Dict[BucketKey, MsgDelta] = {}
        self.voice: Dict[BucketKey, VoiceDelta] = {}
        self.command: Dict[CommandKey, int] = {}
        self.daily: Dict[DailyKey, DailyDelta] = {}
        self.minute_counts: Dict[MinuteKey, int] = {}

    def add_message(self, key: BucketKey, content_len: int, has_attachment: bool) -> None:
        delta = self.message.setdefault(key, MsgDelta())
        delta.message_count += 1
        delta.total_content_len += int(content_len)
        if has_attachment:
            delta.attachment_count += 1

    def add_delete(self, key: BucketKey) -> None:
        delta = self.message.setdefault(key, MsgDelta())
        delta.delete_count += 1

    def add_voice(self, key: BucketKey, seconds: int, join: bool, leave: bool, peak: int) -> None:
        delta = self.voice.setdefault(key, VoiceDelta())
        if join:
            delta.join_count += 1
        if leave:
            delta.leave_count += 1
        delta.total_seconds += max(0, int(seconds))
        if peak > delta.peak_concurrent:
            delta.peak_concurrent = peak

    def add_command(self, key: CommandKey) -> None:
        self.command[key] = self.command.get(key, 0) + 1

    def add_daily_message(self, guild_id: str, day: date) -> None:
        delta = self.daily.setdefault((guild_id, day), DailyDelta())
        delta.messages += 1

    def add_daily_voice_minutes(self, guild_id: str, day: date, minutes: int) -> None:
        if minutes <= 0:
            return
        delta = self.daily.setdefault((guild_id, day), DailyDelta())
        delta.voice_minutes += minutes

    def add_minute_count(self, bucket_start: datetime, guild_id: str) -> None:
        key = (bucket_start, guild_id)
        self.minute_counts[key] = self.minute_counts.get(key, 0) + 1

    def drain(self) -> tuple[
        Dict[BucketKey, MsgDelta],
        Dict[BucketKey, VoiceDelta],
        Dict[CommandKey, int],
        Dict[DailyKey, DailyDelta],
        Dict[MinuteKey, int],
    ]:
        message = self.message
        voice = self.voice
        command = self.command
        daily = self.daily
        minute_counts = self.minute_counts
        self.message = {}
        self.voice = {}
        self.command = {}
        self.daily = {}
        self.minute_counts = {}
        return message, voice, command, daily, minute_counts

    def merge(
        self,
        message: Dict[BucketKey, MsgDelta],
        voice: Dict[BucketKey, VoiceDelta],
        command: Dict[CommandKey, int],
        daily: Dict[DailyKey, DailyDelta],
        minute_counts: Dict[MinuteKey, int],
    ) -> None:
        for key, delta in message.items():
            current = self.message.setdefault(key, MsgDelta())
            current.message_count += delta.message_count
            current.delete_count += delta.delete_count
            current.attachment_count += delta.attachment_count
            current.total_content_len += delta.total_content_len

        for key, delta in voice.items():
            current = self.voice.setdefault(key, VoiceDelta())
            current.join_count += delta.join_count
            current.leave_count += delta.leave_count
            current.total_seconds += delta.total_seconds
            if delta.peak_concurrent > current.peak_concurrent:
                current.peak_concurrent = delta.peak_concurrent

        for key, count in command.items():
            self.command[key] = self.command.get(key, 0) + count

        for key, delta in daily.items():
            current = self.daily.setdefault(key, DailyDelta())
            current.messages += delta.messages
            current.voice_minutes += delta.voice_minutes

        for key, count in minute_counts.items():
            self.minute_counts[key] = self.minute_counts.get(key, 0) + count

    def size(self) -> int:
        return len(self.message) + len(self.voice) + len(self.command) + len(self.daily) + len(self.minute_counts)


class RedisBuffers:
    def __init__(self) -> None:
        self.hll_updates: Dict[str, set[str]] = {}
        self.active_channels: Dict[str, set[str]] = {}
        self.daily_keys: set[DailyKey] = set()

    def add_user(self, guild_id: str, day: date, user_id: str) -> None:
        key = _hll_key(guild_id, day)
        self.hll_updates.setdefault(key, set()).add(user_id)
        self.daily_keys.add((guild_id, day))

    def add_active_channel(self, guild_id: str, day: date, channel_id: str) -> None:
        key = _active_channel_key(guild_id, day)
        self.active_channels.setdefault(key, set()).add(channel_id)
        self.daily_keys.add((guild_id, day))

    def clear(self) -> None:
        self.hll_updates = {}
        self.active_channels = {}
        self.daily_keys = set()


_settings_cache: dict[str, tuple[bool, datetime]] = {}


def _safe_parse_ts(value: Optional[str]) -> datetime:
    if not value:
        return datetime.now(timezone.utc)
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
        return parsed if parsed.tzinfo else parsed.replace(tzinfo=timezone.utc)
    except ValueError:
        return datetime.now(timezone.utc)


def _floor_ts(ts: datetime, bucket_size: int) -> datetime:
    epoch = int(ts.timestamp())
    floored = epoch - (epoch % bucket_size)
    return datetime.fromtimestamp(floored, tz=timezone.utc)


def _bucket_keys(ts: datetime, guild_id: str, channel_id: Optional[str]) -> list[BucketKey]:
    keys: list[BucketKey] = []
    channel_key = channel_id or GUILD_CHANNEL_ID
    for size in BUCKET_SIZES:
        bucket_start = _floor_ts(ts, size)
        keys.append((bucket_start, size, guild_id, channel_key))
    return keys


def _hll_key(guild_id: str, day: date) -> str:
    return f"hll:dau:{guild_id}:{day.isoformat()}"


def _active_channel_key(guild_id: str, day: date) -> str:
    return f"set:active_channels:{guild_id}:{day.isoformat()}"


async def _analytics_enabled(db: Database, guild_id: str) -> bool:
    cached = _settings_cache.get(guild_id)
    now = datetime.now(timezone.utc)
    if cached and cached[1] > now:
        return cached[0]
    settings = await fetch_guild_settings(db, guild_id)
    enabled = bool(settings.analytics_enabled)
    _settings_cache[guild_id] = (enabled, now + timedelta(minutes=2))
    return enabled


async def _apply_redis_buffers(rds: redis.Redis, buffers: RedisBuffers) -> None:
    for key, values in buffers.hll_updates.items():
        if values:
            await rds.pfadd(key, *values)
            await rds.expire(key, HLL_TTL_SECONDS)
    for key, values in buffers.active_channels.items():
        if values:
            await rds.sadd(key, *values)
            await rds.expire(key, ACTIVE_SET_TTL_SECONDS)


def _day_range(day: date, window: int) -> list[date]:
    return [day - timedelta(days=offset) for offset in range(window)]


async def _daily_counts(
    rds: redis.Redis,
    daily_keys: set[DailyKey],
    daily_deltas: Dict[DailyKey, DailyDelta],
) -> Dict[DailyKey, Dict[str, Optional[int]]]:
    results: Dict[DailyKey, Dict[str, Optional[int]]] = {}
    combined = set(daily_keys) | set(daily_deltas.keys())
    for guild_id, day in combined:
        hll_key = _hll_key(guild_id, day)
        active_key = _active_channel_key(guild_id, day)
        dau = await rds.pfcount(hll_key)
        active_channels = await rds.scard(active_key)

        wau_keys = [_hll_key(guild_id, d) for d in _day_range(day, 7)]
        mau_keys = [_hll_key(guild_id, d) for d in _day_range(day, 30)]
        wau = await rds.pfcount(*wau_keys) if wau_keys else None
        mau = await rds.pfcount(*mau_keys) if mau_keys else None

        results[(guild_id, day)] = {
            "dau_est": int(dau) if dau is not None else None,
            "wau_est": int(wau) if wau is not None else None,
            "mau_est": int(mau) if mau is not None else None,
            "active_channels": int(active_channels) if active_channels is not None else 0,
        }
    return results


async def _update_concurrency(
    rds: redis.Redis,
    guild_id: str,
    channel_id: Optional[str],
    delta: int,
) -> tuple[int, int]:
    guild_key = f"voice:concurrent:{guild_id}:guild"
    if delta > 0:
        guild_count = int(await rds.incr(guild_key))
    else:
        guild_count = int(await rds.decr(guild_key))
    await rds.expire(guild_key, VOICE_CONCURRENT_TTL_SECONDS)

    channel_count = guild_count
    if channel_id:
        channel_key = f"voice:concurrent:{guild_id}:channel:{channel_id}"
        if delta > 0:
            channel_count = int(await rds.incr(channel_key))
        else:
            channel_count = int(await rds.decr(channel_key))
        await rds.expire(channel_key, VOICE_CONCURRENT_TTL_SECONDS)
    return guild_count, channel_count


async def _process_event(
    payload: Dict[str, Any],
    *,
    agg: Aggregator,
    buffers: RedisBuffers,
    rds: redis.Redis,
    db: Database,
) -> None:
    event_type = payload.get("event") or payload.get("type")
    if not event_type:
        return

    guild_id = str(payload.get("guild_id") or "").strip()
    if not guild_id:
        return

    if not await _analytics_enabled(db, guild_id):
        return

    metadata = payload.get("metadata") or {}
    flags = payload.get("flags") or metadata.get("flags") or {}
    if flags and flags.get("premium_analytics") is False:
        return

    ts = _safe_parse_ts(payload.get("timestamp") or payload.get("ts"))
    day = ts.date()
    channel_id = str(payload.get("channel_id") or "").strip() or None

    if event_type == "MESSAGE_CREATE":
        if bool(payload.get("is_bot") or metadata.get("is_bot")):
            return
        author_id = str(payload.get("author_id") or "").strip()
        content_len = payload.get("content_len")
        if content_len is None:
            content_len = len(payload.get("content") or "")
        try:
            content_len_value = int(content_len)
        except (TypeError, ValueError):
            content_len_value = len(payload.get("content") or "")
        has_attachments = payload.get("has_attachments")
        if has_attachments is None:
            attachments = metadata.get("attachments")
            if isinstance(attachments, (list, tuple)):
                has_attachments = len(attachments) > 0
            elif isinstance(attachments, int):
                has_attachments = attachments > 0
            else:
                has_attachments = bool(metadata.get("has_attachments"))

        for key in _bucket_keys(ts, guild_id, None):
            agg.add_message(key, content_len_value, bool(has_attachments))
        if channel_id:
            for key in _bucket_keys(ts, guild_id, channel_id):
                agg.add_message(key, content_len_value, bool(has_attachments))

        agg.add_daily_message(guild_id, day)
        if author_id:
            buffers.add_user(guild_id, day, author_id)
        if channel_id:
            buffers.add_active_channel(guild_id, day, channel_id)

        minute_bucket = _floor_ts(ts, 60)
        agg.add_minute_count(minute_bucket, guild_id)

    elif event_type == "MESSAGE_DELETE":
        for key in _bucket_keys(ts, guild_id, None):
            agg.add_delete(key)
        if channel_id:
            for key in _bucket_keys(ts, guild_id, channel_id):
                agg.add_delete(key)

    elif event_type in {"COMMAND_INTERACTION", "INTERACTION_CREATE"}:
        command_name = metadata.get("command_name") or payload.get("command_name")
        if not command_name:
            return
        for size in BUCKET_SIZES:
            bucket_start = _floor_ts(ts, size)
            agg.add_command((bucket_start, size, guild_id, str(command_name)))

    elif event_type == "VOICE_STATE_UPDATE":
        user_id = str(payload.get("user_id") or payload.get("author_id") or "").strip()
        if not user_id:
            return

        before = payload.get("before") or {}
        after = payload.get("after") or {}
        before_channel = str(before.get("channel_id") or "").strip() or None
        after_channel = str(after.get("channel_id") or "").strip() or None

        session_key = f"voice:active:{guild_id}:{user_id}"

        if before_channel is None and after_channel is not None:
            await rds.hset(session_key, mapping={"channel_id": after_channel, "joined_ts": ts.isoformat()})
            await rds.expire(session_key, VOICE_SESSION_TTL_SECONDS)
            guild_count, channel_count = await _update_concurrency(rds, guild_id, after_channel, 1)

            for key in _bucket_keys(ts, guild_id, None):
                agg.add_voice(key, 0, join=True, leave=False, peak=guild_count)
            for key in _bucket_keys(ts, guild_id, after_channel):
                agg.add_voice(key, 0, join=True, leave=False, peak=channel_count)
            return

        if before_channel is not None and after_channel is None:
            session = await rds.hgetall(session_key)
            joined_ts = session.get(b"joined_ts") if session else None
            channel = session.get(b"channel_id") if session else None
            channel_str = channel.decode() if channel else before_channel
            seconds = 0
            if joined_ts:
                joined = _safe_parse_ts(joined_ts.decode())
                seconds = int((ts - joined).total_seconds())
            await rds.delete(session_key)
            guild_count, channel_count = await _update_concurrency(rds, guild_id, channel_str, -1)

            for key in _bucket_keys(ts, guild_id, None):
                agg.add_voice(key, seconds, join=False, leave=True, peak=guild_count)
            if channel_str:
                for key in _bucket_keys(ts, guild_id, channel_str):
                    agg.add_voice(key, seconds, join=False, leave=True, peak=channel_count)
            agg.add_daily_voice_minutes(guild_id, day, int(seconds // 60))
            return

        if before_channel and after_channel and before_channel != after_channel:
            session = await rds.hgetall(session_key)
            joined_ts = session.get(b"joined_ts") if session else None
            channel = session.get(b"channel_id") if session else None
            channel_str = channel.decode() if channel else before_channel
            seconds = 0
            if joined_ts:
                joined = _safe_parse_ts(joined_ts.decode())
                seconds = int((ts - joined).total_seconds())

            guild_count, channel_count = await _update_concurrency(rds, guild_id, channel_str, -1)
            for key in _bucket_keys(ts, guild_id, None):
                agg.add_voice(key, seconds, join=False, leave=True, peak=guild_count)
            if channel_str:
                for key in _bucket_keys(ts, guild_id, channel_str):
                    agg.add_voice(key, seconds, join=False, leave=True, peak=channel_count)
            agg.add_daily_voice_minutes(guild_id, day, int(seconds // 60))

            await rds.hset(session_key, mapping={"channel_id": after_channel, "joined_ts": ts.isoformat()})
            await rds.expire(session_key, VOICE_SESSION_TTL_SECONDS)
            guild_count, channel_count = await _update_concurrency(rds, guild_id, after_channel, 1)
            for key in _bucket_keys(ts, guild_id, None):
                agg.add_voice(key, 0, join=True, leave=False, peak=guild_count)
            for key in _bucket_keys(ts, guild_id, after_channel):
                agg.add_voice(key, 0, join=True, leave=False, peak=channel_count)


async def _flush(
    *,
    agg: Aggregator,
    buffers: RedisBuffers,
    rds: redis.Redis,
    db: Database,
    pending_ids: list[str],
    stream: str,
    group: str,
) -> None:
    if agg.size() == 0 and not pending_ids:
        return

    message, voice, command, daily, minute_counts = agg.drain()
    try:
        await _apply_redis_buffers(rds, buffers)
        daily_counts = await _daily_counts(rds, buffers.daily_keys, daily)

        message_rows = [
            {
                "bucket_start": key[0],
                "bucket_size": key[1],
                "guild_id": key[2],
                "channel_id": key[3],
                "message_count": delta.message_count,
                "delete_count": delta.delete_count,
                "attachment_count": delta.attachment_count,
                "total_content_len": delta.total_content_len,
                "unique_speakers_est": None,
            }
            for key, delta in message.items()
        ]
        voice_rows = [
            {
                "bucket_start": key[0],
                "bucket_size": key[1],
                "guild_id": key[2],
                "channel_id": key[3],
                "join_count": delta.join_count,
                "leave_count": delta.leave_count,
                "total_seconds": delta.total_seconds,
                "peak_concurrent": delta.peak_concurrent,
                "unique_listeners_est": None,
            }
            for key, delta in voice.items()
        ]
        command_rows = [
            {
                "bucket_start": key[0],
                "bucket_size": key[1],
                "guild_id": key[2],
                "command_name": key[3],
                "use_count": count,
                "error_count": 0,
            }
            for key, count in command.items()
        ]
        daily_rows = []
        for key, delta in daily.items():
            guild_id, day = key
            counts = daily_counts.get(key, {})
            daily_rows.append(
                {
                    "day": day,
                    "guild_id": guild_id,
                    "messages": delta.messages,
                    "voice_minutes": delta.voice_minutes,
                    "active_channels": counts.get("active_channels", 0) or 0,
                    "dau_est": counts.get("dau_est"),
                    "wau_est": counts.get("wau_est"),
                    "mau_est": counts.get("mau_est"),
                }
            )

        minute_rows = [
            {"time_bucket": key[0], "guild_id": key[1], "count": count}
            for key, count in minute_counts.items()
        ]

        await upsert_message_bucket_deltas(db, message_rows)
        await upsert_voice_bucket_deltas(db, voice_rows)
        await upsert_command_bucket_deltas(db, command_rows)
        await upsert_daily_summary_deltas(db, daily_rows)
        await upsert_message_count_deltas(db, minute_rows)

        if pending_ids:
            await rds.xack(stream, group, *pending_ids)
            pending_ids.clear()

        buffers.clear()
    except Exception:  # noqa: BLE001
        logging.exception("[analytics] flush failed; will retry")
        agg.merge(message, voice, command, daily, minute_counts)


async def _consume(config: AppConfig) -> None:
    if not config.database_url:
        raise RuntimeError("DATABASE_URL is required for analytics worker")

    rds = redis.from_url(config.redis.url, decode_responses=False)
    stream = config.queue.stream
    group = config.queue.group_name
    consumer = config.queue.consumer_name or "analytics-1"

    try:
        await rds.xgroup_create(stream, group, id="$", mkstream=True)
    except Exception:
        pass

    db = await create_pool(config.database_url)
    await init_db(db)
    logging.info("[analytics] connected to database")

    agg = Aggregator()
    buffers = RedisBuffers()
    pending_ids: list[str] = []
    last_flush = datetime.now(timezone.utc)

    try:
        while True:
            entries = await rds.xreadgroup(
                groupname=group,
                consumername=consumer,
                streams={stream: ">"},
                count=200,
                block=int(FLUSH_INTERVAL_SECONDS * 1000),
            )

            if entries:
                for _, messages in entries:
                    for message_id, fields in messages:
                        raw = fields.get(b"data") if isinstance(fields, dict) else None
                        if raw is None:
                            raw = fields.get("data") if isinstance(fields, dict) else None
                        if raw is None:
                            await rds.xack(stream, group, message_id)
                            continue
                        if isinstance(raw, (bytes, bytearray)):
                            raw = raw.decode("utf-8")
                        try:
                            payload = json.loads(raw)
                        except json.JSONDecodeError:
                            await rds.xack(stream, group, message_id)
                            continue

                        try:
                            await _process_event(payload, agg=agg, buffers=buffers, rds=rds, db=db)
                        except Exception:  # noqa: BLE001
                            logging.exception("[analytics] failed to process event")
                        pending_ids.append(
                            message_id if isinstance(message_id, str) else message_id.decode("utf-8")
                        )

            now = datetime.now(timezone.utc)
            if (
                (now - last_flush).total_seconds() >= FLUSH_INTERVAL_SECONDS
                or agg.size() >= MAX_BUFFER_KEYS
                or len(pending_ids) >= MAX_PENDING
            ):
                await _flush(
                    agg=agg,
                    buffers=buffers,
                    rds=rds,
                    db=db,
                    pending_ids=pending_ids,
                    stream=stream,
                    group=group,
                )
                last_flush = now
    finally:
        await rds.aclose()
        await db.close()
        logging.info("[analytics] shutdown complete")


def main() -> None:
    config = load_app_config()
    configure_logging(config.log_level)
    try:
        asyncio.run(_consume(config))
    except KeyboardInterrupt:
        logging.info("[analytics] shutting down on interrupt")


if __name__ == "__main__":
    main()
