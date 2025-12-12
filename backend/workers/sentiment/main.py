import json
import logging
from dataclasses import dataclass
from datetime import datetime, timedelta, timezone
from typing import Optional

from backend.common.config import AppConfig
from backend.common.models import QueueMessage
from backend.common.openrouter import chat_completion
from backend.database.db import (
    Database,
    fetch_guild_plan,
    fetch_guild_settings,
    fetch_sentiment_samples,
    insert_sentiment_sample,
    record_sentiment_score,
    upsert_sentiment_report,
)
from backend.workers.consumer import run_worker


POSITIVE_WORDS = {
    "love",
    "loved",
    "great",
    "awesome",
    "amazing",
    "nice",
    "good",
    "gg",
    "fun",
    "hype",
    "pog",
    "thanks",
    "thank",
    "congrats",
}

NEGATIVE_WORDS = {
    "hate",
    "hated",
    "bad",
    "awful",
    "terrible",
    "toxic",
    "sad",
    "angry",
    "mad",
    "annoying",
    "boring",
    "ugh",
}


def _safe_parse_ts(ts: str) -> datetime:
    try:
        dt = datetime.fromisoformat(ts)
        return dt if dt.tzinfo else dt.replace(tzinfo=timezone.utc)
    except ValueError:
        return datetime.now(timezone.utc)


def _sentiment_score(text: str) -> float:
    tokens = [t.strip(".,!?;:()[]{}<>\"'").lower() for t in text.split()]
    if not tokens:
        return 0.0
    pos = sum(1 for t in tokens if t in POSITIVE_WORDS)
    neg = sum(1 for t in tokens if t in NEGATIVE_WORDS)
    score = (pos - neg) / max(1, min(len(tokens), 30))
    return max(-1.0, min(1.0, score))


def _score_to_label(score: float) -> str:
    if score >= 0.05:
        return "positive"
    if score <= -0.05:
        return "negative"
    return "neutral"


def _sanitize_sample(text: str, max_len: int = 500) -> str:
    text = " ".join(text.strip().split())
    return text[:max_len]


@dataclass
class DayStats:
    count: int = 0
    score_sum: float = 0.0
    last_persist_at: Optional[datetime] = None
    last_report_at: Optional[datetime] = None


_plan_cache: dict[str, tuple[str, datetime]] = {}
_settings_cache: dict[str, tuple[bool, datetime]] = {}
_stats: dict[tuple[str, str], DayStats] = {}


async def _guild_plan(db: Database, guild_id: str) -> str:
    cached = _plan_cache.get(guild_id)
    now = datetime.now(timezone.utc)
    if cached and cached[1] > now:
        return cached[0]
    plan = await fetch_guild_plan(db, guild_id)
    _plan_cache[guild_id] = (plan, now + timedelta(minutes=5))
    return plan


async def _sentiment_enabled(db: Database, guild_id: str) -> bool:
    cached = _settings_cache.get(guild_id)
    now = datetime.now(timezone.utc)
    if cached and cached[1] > now:
        return cached[0]
    settings = await fetch_guild_settings(db, guild_id)
    enabled = bool(settings.sentiment_enabled)
    _settings_cache[guild_id] = (enabled, now + timedelta(minutes=2))
    return enabled


async def _maybe_generate_report(
    *,
    config: AppConfig,
    db: Database,
    guild_id: str,
    day: datetime,
    stats: DayStats,
) -> None:
    if not config.openrouter_api_key:
        return

    plan = await _guild_plan(db, guild_id)
    if plan != "pro":
        return

    now = datetime.now(timezone.utc)
    if stats.last_report_at and (now - stats.last_report_at) < timedelta(hours=6):
        return
    if stats.count < 40:
        return

    samples = await fetch_sentiment_samples(db, guild_id, day, limit=200)
    if len(samples) < 20:
        return

    prompt = {
        "role": "system",
        "content": (
            "You are an analytics agent for a Discord server. "
            "Given message samples from a single day, produce a STRICT JSON object with:\n"
            "{"
            "\"overview\":string,"
            "\"mood\": {\"label\": \"positive|neutral|negative\", \"score\": number},"
            "\"topics\": [{\"name\": string, \"evidence\": [string]}],"
            "\"notable_games\": [{\"name\": string, \"why\": string}],"
            "\"recommended_events\": [{\"title\": string, \"when\": string, \"why\": string, \"format\": string}],"
            "\"moderation_risks\": [{\"risk\": string, \"why\": string}],"
            "\"action_items\": [string]"
            "}"
            "\nDo not include markdown; output only JSON."
        ),
    }
    user = {
        "role": "user",
        "content": json.dumps(
            {
                "guild_id": guild_id,
                "day": day.date().isoformat(),
                "samples": samples[:200],
            },
            ensure_ascii=True,
        ),
    }

    raw = await chat_completion(
        api_key=config.openrouter_api_key,
        model=config.openrouter_model,
        messages=[prompt, user],
        temperature=0.2,
        max_tokens=900,
        title="Guildest Sentiment Agent",
    )

    try:
        report = json.loads(raw)
    except Exception:
        report = {"overview": raw.strip()[:4000]}

    await upsert_sentiment_report(db, guild_id=guild_id, day=day, model=config.openrouter_model, report=report)
    stats.last_report_at = now


async def handle_message(message: QueueMessage, config: AppConfig, db: Optional[Database]) -> None:
    if not db:
        logging.info("[sentiment] observed message %s in guild %s", message.message_id, message.guild_id)
        return

    if not await _sentiment_enabled(db, message.guild_id):
        return

    ts = _safe_parse_ts(message.timestamp)
    day = ts.astimezone(timezone.utc).replace(hour=0, minute=0, second=0, microsecond=0)
    key = (message.guild_id, day.date().isoformat())
    stats = _stats.setdefault(key, DayStats())

    score = _sentiment_score(message.content or "")
    stats.count += 1
    stats.score_sum += score

    # Persist an inexpensive daily score for everyone.
    now = datetime.now(timezone.utc)
    if not stats.last_persist_at or (now - stats.last_persist_at) > timedelta(minutes=5):
        avg = stats.score_sum / max(1, stats.count)
        await record_sentiment_score(db, message.guild_id, day, sentiment=_score_to_label(avg), score=float(avg))
        stats.last_persist_at = now

    # For pro servers, sample messages to support daily report generation.
    plan = await _guild_plan(db, message.guild_id)
    if plan == "pro" and message.content:
        if abs(hash(message.message_id)) % 25 == 0:
            await insert_sentiment_sample(db, message.guild_id, day, _sanitize_sample(message.content))

    await _maybe_generate_report(config=config, db=db, guild_id=message.guild_id, day=day, stats=stats)


def main() -> None:
    run_worker("sentiment", handle_message)


if __name__ == "__main__":
    main()
