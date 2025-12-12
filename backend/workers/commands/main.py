import logging
from datetime import datetime, timedelta, timezone
from typing import Optional

from backend.common.config import AppConfig
from backend.common.discord_rest import send_channel_message
from backend.common.models import QueueMessage
from backend.database.db import (
    Database,
    fetch_guild_plan,
    fetch_guild_settings,
    fetch_latest_sentiment,
    fetch_message_count_sum,
    fetch_moderation_logs,
)
from backend.workers.consumer import run_worker


_prefix_cache: dict[str, tuple[str, datetime]] = {}


async def _get_prefix(db: Database, guild_id: str) -> str:
    cached = _prefix_cache.get(guild_id)
    now = datetime.now(timezone.utc)
    if cached and cached[1] > now:
        return cached[0]

    settings = await fetch_guild_settings(db, guild_id)
    prefix = settings.prefix or "!"
    _prefix_cache[guild_id] = (prefix, now + timedelta(minutes=2))
    return prefix


def _parse_command(content: str, prefix: str) -> Optional[tuple[str, list[str]]]:
    text = (content or "").strip()
    if not text.startswith(prefix):
        return None

    rest = text[len(prefix) :].strip()
    if rest == "":
        return None

    parts = rest.split()
    cmd = parts[0].lower()
    args = parts[1:]
    return cmd, args


async def _reply(config: AppConfig, message: QueueMessage, content: str) -> None:
    if not config.discord_token:
        logging.warning("[commands] DISCORD_TOKEN not set; cannot reply")
        return

    await send_channel_message(
        bot_token=config.discord_token,
        channel_id=message.channel_id,
        content=content,
        allowed_mentions={"parse": []},
    )


async def handle_message(message: QueueMessage, config: AppConfig, db: Optional[Database]) -> None:
    if message.event != "MESSAGE_CREATE":
        return
    if not db:
        return

    prefix = await _get_prefix(db, message.guild_id)
    parsed = _parse_command(message.content, prefix)
    if not parsed:
        return

    cmd, _args = parsed
    logging.info("[commands] guild=%s cmd=%s", message.guild_id, cmd)

    if cmd in {"help", "h"}:
        await _reply(
            config,
            message,
            "\n".join(
                [
                    f"Commands (prefix `{prefix}`):",
                    f"- `{prefix}help`",
                    f"- `{prefix}ping`",
                    f"- `{prefix}dashboard`",
                    f"- `{prefix}stats`",
                    f"- `{prefix}sentiment`",
                    f"- `{prefix}modlogs` (Pro)",
                ]
            ),
        )
        return

    if cmd == "ping":
        await _reply(config, message, "Pong.")
        return

    if cmd == "dashboard":
        base = (config.frontend_base_url or "http://localhost:3000").rstrip("/")
        await _reply(config, message, f"Dashboard: {base}\nConnect this server in the dashboard to enable Pro features.")
        return

    if cmd == "stats":
        now = datetime.now(timezone.utc)
        last_hour = await fetch_message_count_sum(db, message.guild_id, now - timedelta(hours=1), now)
        last_day = await fetch_message_count_sum(db, message.guild_id, now - timedelta(hours=24), now)
        await _reply(config, message, f"Message stats:\n- last hour: {last_hour}\n- last 24h: {last_day}")
        return

    if cmd == "sentiment":
        latest = await fetch_latest_sentiment(db, message.guild_id)
        if not latest:
            await _reply(config, message, "No sentiment data yet.")
            return
        score = latest.get("score")
        score_str = f"{score:.3f}" if isinstance(score, (int, float)) else "n/a"
        await _reply(
            config,
            message,
            f"Latest sentiment ({latest['day']}): {latest['sentiment']} (score {score_str})",
        )
        return

    if cmd in {"modlogs", "modlog"}:
        plan = await fetch_guild_plan(db, message.guild_id)
        if plan != "pro":
            await _reply(config, message, "Moderation log history is a Pro feature.")
            return
        rows = await fetch_moderation_logs(db, message.guild_id, limit=5)
        if not rows:
            await _reply(config, message, "No moderation events yet.")
            return
        lines = ["Latest moderation events:"]
        for row in rows:
            when = row["created_at"]
            action = row.get("action") or "event"
            reason = row.get("reason") or ""
            lines.append(f"- {when}: {action} {reason}".strip())
        await _reply(config, message, "\n".join(lines))
        return

    await _reply(config, message, f"Unknown command `{cmd}`. Try `{prefix}help`.")


def main() -> None:
    run_worker("commands", handle_message)


if __name__ == "__main__":
    main()

