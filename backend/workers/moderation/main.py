import logging
import re
from datetime import datetime, timedelta, timezone
from typing import Optional

from backend.common.config import AppConfig
from backend.common.models import QueueMessage
from backend.database.db import Database, fetch_guild_plan, fetch_guild_settings, log_moderation_action
from backend.workers.consumer import run_worker


INVITE_RE = re.compile(r"(discord\.gg/|discord\.com/invite/)", re.IGNORECASE)
URL_RE = re.compile(r"https?://", re.IGNORECASE)

_plan_cache: dict[str, tuple[str, datetime]] = {}
_settings_cache: dict[str, tuple[bool, datetime]] = {}


async def _guild_plan(db: Database, guild_id: str) -> str:
    cached = _plan_cache.get(guild_id)
    now = datetime.now(timezone.utc)
    if cached and cached[1] > now:
        return cached[0]
    plan = await fetch_guild_plan(db, guild_id)
    _plan_cache[guild_id] = (plan, now + timedelta(minutes=5))
    return plan


async def _moderation_enabled(db: Database, guild_id: str) -> bool:
    cached = _settings_cache.get(guild_id)
    now = datetime.now(timezone.utc)
    if cached and cached[1] > now:
        return cached[0]
    settings = await fetch_guild_settings(db, guild_id)
    enabled = bool(settings.moderation_enabled)
    _settings_cache[guild_id] = (enabled, now + timedelta(minutes=2))
    return enabled


def _decide_action(message: QueueMessage) -> tuple[str, str]:
    content = (message.content or "").strip()
    if content == "":
        return "reviewed", "empty"

    if INVITE_RE.search(content):
        return "flagged", "invite link"

    if len(URL_RE.findall(content)) >= 3:
        return "flagged", "excessive links"

    if "@everyone" in content or "@here" in content:
        return "flagged", "mass mention"

    if len(content) > 1500:
        return "flagged", "very long message"

    return "reviewed", "heuristic checks"


async def handle_message(message: QueueMessage, config: AppConfig, db: Optional[Database]) -> None:
    if not db:
        return

    if not await _moderation_enabled(db, message.guild_id):
        return

    action, reason = _decide_action(message)
    logging.info("[moderation] %s message %s in guild %s (%s)", action, message.message_id, message.guild_id, reason)

    plan = await _guild_plan(db, message.guild_id)
    if plan in {"plus", "premium"}:
        bot_id = config.discord_client_id
        actor_type = "bot" if bot_id else "system"
        await log_moderation_action(
            db,
            guild_id=message.guild_id,
            action=action,
            reason=reason,
            message_id=message.message_id,
            channel_id=message.channel_id,
            author_id=message.author_id,
            actor_id=bot_id,
            actor_type=actor_type,
            target_id=message.author_id,
            bot_id=bot_id,
            source="automod",
            metadata={"message_content": message.content} if message.content else None,
        )
        logging.debug("[moderation] logged moderation event for message %s", message.message_id)


def main() -> None:
    run_worker("moderation", handle_message)


if __name__ == "__main__":
    main()
