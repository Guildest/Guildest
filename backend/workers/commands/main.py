import asyncio
import logging
from datetime import datetime, timedelta, timezone
from typing import Any, Optional

import hikari

from backend.common.config import AppConfig
from backend.common.discord_rest import (
    ban_member,
    create_dm_channel,
    edit_interaction_response,
    send_interaction_followup,
    send_channel_message,
    timeout_member,
    unban_member,
)
from backend.common.models import QueueMessage
from backend.database.db import (
    Database,
    clear_warns,
    fetch_active_warns,
    fetch_guild_plan,
    fetch_guild_settings,
    fetch_latest_sentiment,
    fetch_message_count_sum,
    fetch_moderation_logs,
    insert_guild_warn,
    log_moderation_action,
    record_ban_action,
)
from backend.workers.consumer import run_worker


def _option_value(options: list[dict[str, Any]], name: str) -> Optional[Any]:
    for option in options:
        if option.get("name") == name:
            return option.get("value")
        nested = option.get("options") or []
        nested_value = _option_value(nested, name)
        if nested_value is not None:
            return nested_value
    return None


def _resolved_user(resolved_users: dict[str, Any], user_id: str) -> Optional[dict[str, Any]]:
    if not resolved_users:
        return None
    return resolved_users.get(str(user_id))


def _get_user_option(
    options: list[dict[str, Any]], resolved_users: dict[str, Any], name: str
) -> tuple[Optional[str], Optional[dict[str, Any]]]:
    value = _option_value(options, name)
    if value is None:
        return None, None
    user_id = str(value)
    return user_id, _resolved_user(resolved_users, user_id)


def _format_user(user_id: str, user: Optional[dict[str, Any]]) -> str:
    if user:
        username = user.get("username") or user.get("global_name")
        discriminator = str(user.get("discriminator") or "")
        if username:
            if discriminator and discriminator != "0":
                return f"{username}#{discriminator} (<@{user_id}>)"
            return f"{username} (<@{user_id}>)"
    return f"<@{user_id}>"


def _permissions_from_metadata(metadata: dict[str, Any]) -> hikari.Permissions:
    raw = metadata.get("member_permissions")
    try:
        value = int(raw)
    except (TypeError, ValueError):
        return hikari.Permissions.NONE
    return hikari.Permissions(value)


def _has_any_permission(perms: hikari.Permissions, required: list[hikari.Permissions]) -> bool:
    if perms & hikari.Permissions.ADMINISTRATOR:
        return True
    return any(bool(perms & perm) for perm in required)


def _select_warn_action(policy: list[dict[str, Any]], warn_count: int) -> Optional[dict[str, Any]]:
    if not policy:
        return None
    normalized: list[dict[str, Any]] = []
    for item in policy:
        if hasattr(item, "dict"):
            data = item.dict()
        else:
            data = dict(item)
        try:
            threshold = int(data.get("threshold", 0))
        except (TypeError, ValueError):
            continue
        action = str(data.get("action") or "").lower()
        duration = data.get("duration_hours")
        try:
            duration_hours = int(duration) if duration is not None else None
        except (TypeError, ValueError):
            duration_hours = None
        if threshold <= 0 or action not in {"timeout", "ban"}:
            continue
        normalized.append({"threshold": threshold, "action": action, "duration_hours": duration_hours})
    normalized.sort(key=lambda item: item["threshold"])
    selected = None
    for item in normalized:
        if warn_count >= item["threshold"]:
            selected = item
    return selected


async def _respond_embed(
    *,
    application_id: str,
    interaction_token: str,
    title: str,
    description: str,
    color: int = 0x6366F1,
) -> None:
    embed = {"title": title, "description": description, "color": color}
    try:
        await asyncio.wait_for(
            edit_interaction_response(
                application_id=application_id,
                interaction_token=interaction_token,
                content="",
                embeds=[embed],
                allowed_mentions={"parse": []},
            ),
            timeout=12,
        )
        return
    except Exception as exc:  # noqa: BLE001
        logging.warning("[commands] edit response failed, sending follow-up: %s", exc)

    try:
        await asyncio.wait_for(
            send_interaction_followup(
                application_id=application_id,
                interaction_token=interaction_token,
                content="",
                embeds=[embed],
                allowed_mentions={"parse": []},
                ephemeral=True,
            ),
            timeout=12,
        )
    except Exception as exc:  # noqa: BLE001
        logging.error("[commands] follow-up response failed: %s", exc)


async def _send_ban_dm(
    *,
    config: AppConfig,
    user_id: str,
    guild_id: str,
    reason: Optional[str],
) -> None:
    if not config.discord_token:
        return
    description = "You have been banned from this server."
    if reason:
        description += f"\nReason: {reason}"
    description += "\nIf you believe this was a mistake, you can submit an appeal below."
    embed = {
        "title": "Ban Notice",
        "description": description,
        "color": 0xEF4444,
    }
    components = [
        {
            "type": 1,
            "components": [
                {
                    "type": 2,
                    "style": 1,
                    "label": "Appeal this ban",
                    "custom_id": f"appeal:{guild_id}",
                }
            ],
        }
    ]
    try:
        channel_id = await create_dm_channel(bot_token=config.discord_token, user_id=user_id)
        await send_channel_message(
            bot_token=config.discord_token,
            channel_id=channel_id,
            content="",
            embeds=[embed],
            components=components,
            allowed_mentions={"parse": []},
        )
    except Exception as exc:  # noqa: BLE001
        logging.warning("[commands] failed to DM ban appeal to user %s: %s", user_id, exc)


async def handle_message(message: QueueMessage, config: AppConfig, db: Optional[Database]) -> None:
    if message.event != "COMMAND_INTERACTION":
        return

    metadata = message.metadata or {}
    command_name = metadata.get("command_name") or message.content or ""
    options = metadata.get("options") or []
    resolved_users = metadata.get("resolved_users") or {}
    user = metadata.get("user") or {}

    application_id = metadata.get("application_id") or config.discord_client_id
    interaction_token = metadata.get("interaction_token") or metadata.get("token")
    if not application_id or not interaction_token:
        logging.warning("[commands] missing interaction credentials for message %s", message.message_id)
        return

    author_id = user.get("id") or message.author_id
    author_name = user.get("username") or user.get("global_name")
    guild_id = message.guild_id or ""
    channel_id = message.channel_id or ""
    perms = _permissions_from_metadata(metadata)
    bot_id = metadata.get("application_id") or config.discord_client_id

    logging.info("[commands] handling /%s (message %s)", command_name, message.message_id)

    if command_name == "ping":
        await _respond_embed(
            application_id=application_id,
            interaction_token=interaction_token,
            title="Pong",
            description="Bot is alive.",
            color=0x22C55E,
        )
        return

    if command_name == "help":
        await _respond_embed(
            application_id=application_id,
            interaction_token=interaction_token,
            title="Commands",
            description="\n".join(
                [
                    "/ping",
                    "/help",
                    "/dashboard",
                    "/stats",
                    "/sentiment",
                    "/modlogs (Plus/Premium)",
                    "/warn, /warns, /warn-clear",
                    "/timeout, /ban, /unban",
                ]
            ),
        )
        return

    if command_name == "dashboard":
        base = (config.frontend_base_url or "http://localhost:3000").rstrip("/")
        await _respond_embed(
            application_id=application_id,
            interaction_token=interaction_token,
            title="Dashboard",
            description=f"{base}\nConnect this server in the dashboard to enable paid features.",
            color=0x38BDF8,
        )
        return

    if not guild_id:
        await _respond_embed(
            application_id=application_id,
            interaction_token=interaction_token,
            title="Server only",
            description="This command can only be used in a server.",
            color=0xF97316,
        )
        return

    if not db:
        await _respond_embed(
            application_id=application_id,
            interaction_token=interaction_token,
            title="Database unavailable",
            description="Database not configured for this bot; set DATABASE_URL for DB-backed commands.",
            color=0xF97316,
        )
        return

    if command_name == "stats":
        now = datetime.now(timezone.utc)
        last_hour = await fetch_message_count_sum(db, guild_id, now - timedelta(hours=1), now)
        last_day = await fetch_message_count_sum(db, guild_id, now - timedelta(hours=24), now)
        await _respond_embed(
            application_id=application_id,
            interaction_token=interaction_token,
            title="Message stats",
            description=f"Last hour: {last_hour}\nLast 24h: {last_day}",
            color=0x22C55E,
        )
        return

    if command_name == "sentiment":
        latest = await fetch_latest_sentiment(db, guild_id)
        if not latest:
            await _respond_embed(
                application_id=application_id,
                interaction_token=interaction_token,
                title="Sentiment",
                description="No sentiment data yet.",
                color=0xF97316,
            )
            return
        score = latest.get("score")
        score_str = f"{score:.3f}" if isinstance(score, (int, float)) else "n/a"
        await _respond_embed(
            application_id=application_id,
            interaction_token=interaction_token,
            title="Latest sentiment",
            description=f"{latest['day']}: {latest['sentiment']} (score {score_str})",
            color=0x38BDF8,
        )
        return

    if command_name == "modlogs":
        plan = await fetch_guild_plan(db, guild_id)
        if plan not in {"plus", "premium"}:
            await _respond_embed(
                application_id=application_id,
                interaction_token=interaction_token,
                title="Moderation logs",
                description="Moderation log history is a paid feature.",
                color=0xF97316,
            )
            return
        rows = await fetch_moderation_logs(db, guild_id, limit=5)
        if not rows:
            await _respond_embed(
                application_id=application_id,
                interaction_token=interaction_token,
                title="Moderation logs",
                description="No moderation events yet.",
            )
            return
        lines = []
        for row in rows:
            when = row["created_at"]
            action = row.get("action") or "event"
            reason = row.get("reason") or ""
            lines.append(f"- {when}: {action} {reason}".strip())
        await _respond_embed(
            application_id=application_id,
            interaction_token=interaction_token,
            title="Latest moderation events",
            description="\n".join(lines),
            color=0x38BDF8,
        )
        return

    if command_name in {"warn", "warns", "warn-clear"}:
        if not _has_any_permission(
            perms,
            [hikari.Permissions.MODERATE_MEMBERS, hikari.Permissions.MANAGE_MESSAGES],
        ):
            await _respond_embed(
                application_id=application_id,
                interaction_token=interaction_token,
                title="Missing permissions",
                description="You need Moderate Members or Manage Messages to use warnings.",
                color=0xEF4444,
            )
            return

        user_id, user_data = _get_user_option(options, resolved_users, "user")
        if not user_id:
            await _respond_embed(
                application_id=application_id,
                interaction_token=interaction_token,
                title="Missing user",
                description="Select a user to continue.",
                color=0xEF4444,
            )
            return

        now = datetime.now(timezone.utc)

        if command_name == "warns":
            warns = await fetch_active_warns(db, guild_id, user_id, now)
            if not warns:
                await _respond_embed(
                    application_id=application_id,
                    interaction_token=interaction_token,
                    title="Warnings",
                    description=f"{_format_user(user_id, user_data)} has no active warnings.",
                    color=0x22C55E,
                )
                return
            lines = []
            for warn in warns[:5]:
                reason = warn.get("reason") or "No reason provided"
                lines.append(f"- {warn['created_at']}: {reason}")
            await _respond_embed(
                application_id=application_id,
                interaction_token=interaction_token,
                title="Warnings",
                description=f"{_format_user(user_id, user_data)} has {len(warns)} warning(s).\n"
                + "\n".join(lines),
                color=0xF59E0B,
            )
            return

        if command_name == "warn-clear":
            cleared = await clear_warns(db, guild_id, user_id)
            await log_moderation_action(
                db,
                guild_id=guild_id,
                action="warn_clear",
                reason=None,
                channel_id=channel_id or None,
                author_id=user_id,
                actor_id=author_id,
                actor_type="human",
                target_id=user_id,
                bot_id=bot_id,
                source="command",
                metadata={"cleared": cleared},
            )
            await _respond_embed(
                application_id=application_id,
                interaction_token=interaction_token,
                title="Warnings cleared",
                description=f"Cleared {cleared} warning(s) for {_format_user(user_id, user_data)}.",
                color=0x22C55E,
            )
            return

        reason = _option_value(options, "reason")
        settings = await fetch_guild_settings(db, guild_id)
        expires_at = None
        if settings.warn_decay_days and settings.warn_decay_days > 0:
            expires_at = now + timedelta(days=settings.warn_decay_days)
        await insert_guild_warn(
            db,
            guild_id=guild_id,
            user_id=user_id,
            moderator_id=author_id,
            reason=reason,
            expires_at=expires_at,
        )
        warns = await fetch_active_warns(db, guild_id, user_id, now)
        warn_count = len(warns)
        action = _select_warn_action(settings.warn_policy, warn_count)
        action_note = None
        if action:
            if action["action"] == "timeout":
                duration_hours = action["duration_hours"] or 24
                until = now + timedelta(hours=duration_hours)
                if config.discord_token:
                    await timeout_member(
                        bot_token=config.discord_token,
                        guild_id=guild_id,
                        user_id=user_id,
                        communication_disabled_until=until.isoformat(),
                        reason=f"Warn threshold reached ({warn_count})",
                    )
                    await log_moderation_action(
                        db,
                        guild_id=guild_id,
                        action="timeout",
                        reason="Warn threshold reached",
                        channel_id=channel_id or None,
                        author_id=user_id,
                        actor_id=bot_id,
                        actor_type="bot" if bot_id else "system",
                        target_id=user_id,
                        bot_id=bot_id,
                        source="automod",
                        metadata={"warn_count": warn_count, "duration_hours": duration_hours},
                    )
                    action_note = f"Auto-timeout applied ({duration_hours}h)."
                else:
                    action_note = "Auto-timeout skipped (bot token missing)."
            elif action["action"] == "ban":
                if config.discord_token:
                    await ban_member(
                        bot_token=config.discord_token,
                        guild_id=guild_id,
                        user_id=user_id,
                        reason=f"Warn threshold reached ({warn_count})",
                    )
                    await record_ban_action(
                        db,
                        guild_id=guild_id,
                        user_id=user_id,
                        moderator_id=author_id,
                        moderator_name=author_name,
                        reason="Warn threshold reached",
                    )
                    await _send_ban_dm(
                        config=config,
                        user_id=user_id,
                        guild_id=guild_id,
                        reason="Warn threshold reached",
                    )
                    await log_moderation_action(
                        db,
                        guild_id=guild_id,
                        action="ban",
                        reason="Warn threshold reached",
                        channel_id=channel_id or None,
                        author_id=user_id,
                        actor_id=bot_id,
                        actor_type="bot" if bot_id else "system",
                        target_id=user_id,
                        bot_id=bot_id,
                        source="automod",
                        metadata={"warn_count": warn_count},
                    )
                    action_note = "Auto-ban applied."
                else:
                    action_note = "Auto-ban skipped (bot token missing)."

        await log_moderation_action(
            db,
            guild_id=guild_id,
            action="warn",
            reason=reason,
            channel_id=channel_id or None,
            author_id=user_id,
            actor_id=author_id,
            actor_type="human",
            target_id=user_id,
            bot_id=bot_id,
            source="command",
            metadata={"warn_count": warn_count},
        )

        description = f"{_format_user(user_id, user_data)} now has {warn_count} warning(s)."
        if reason:
            description += f"\nReason: {reason}"
        if action_note:
            description += f"\n{action_note}"
        await _respond_embed(
            application_id=application_id,
            interaction_token=interaction_token,
            title="Warning issued",
            description=description,
            color=0xF59E0B,
        )
        return

    if command_name == "timeout":
        if not _has_any_permission(perms, [hikari.Permissions.MODERATE_MEMBERS]):
            await _respond_embed(
                application_id=application_id,
                interaction_token=interaction_token,
                title="Missing permissions",
                description="You need Moderate Members to timeout users.",
                color=0xEF4444,
            )
            return
        if not config.discord_token:
            await _respond_embed(
                application_id=application_id,
                interaction_token=interaction_token,
                title="Bot token missing",
                description="Discord token not configured for moderation actions.",
                color=0xEF4444,
            )
            return
        user_id, user_data = _get_user_option(options, resolved_users, "user")
        minutes = _option_value(options, "minutes")
        reason = _option_value(options, "reason")
        if not user_id or minutes is None:
            await _respond_embed(
                application_id=application_id,
                interaction_token=interaction_token,
                title="Missing input",
                description="User and minutes are required.",
            )
            return
        try:
            minutes_value = int(minutes)
        except (TypeError, ValueError):
            await _respond_embed(
                application_id=application_id,
                interaction_token=interaction_token,
                title="Invalid duration",
                description="Minutes must be a number.",
            )
            return
        if minutes_value < 1 or minutes_value > 40320:
            await _respond_embed(
                application_id=application_id,
                interaction_token=interaction_token,
                title="Invalid duration",
                description="Timeout must be 1-40320 minutes.",
            )
            return
        until = datetime.now(timezone.utc) + timedelta(minutes=minutes_value)
        await timeout_member(
            bot_token=config.discord_token,
            guild_id=guild_id,
            user_id=user_id,
            communication_disabled_until=until.isoformat(),
            reason=reason,
        )
        await log_moderation_action(
            db,
            guild_id=guild_id,
            action="timeout",
            reason=reason,
            channel_id=channel_id or None,
            author_id=user_id,
            actor_id=author_id,
            actor_type="human",
            target_id=user_id,
            bot_id=bot_id,
            source="command",
            metadata={"duration_minutes": minutes_value},
        )
        description = f"Timed out {_format_user(user_id, user_data)} for {minutes_value} minutes."
        if reason:
            description += f"\nReason: {reason}"
        await _respond_embed(
            application_id=application_id,
            interaction_token=interaction_token,
            title="Timeout applied",
            description=description,
            color=0xF97316,
        )
        return

    if command_name == "ban":
        if not _has_any_permission(perms, [hikari.Permissions.BAN_MEMBERS]):
            await _respond_embed(
                application_id=application_id,
                interaction_token=interaction_token,
                title="Missing permissions",
                description="You need Ban Members to ban users.",
                color=0xEF4444,
            )
            return
        if not config.discord_token:
            await _respond_embed(
                application_id=application_id,
                interaction_token=interaction_token,
                title="Bot token missing",
                description="Discord token not configured for moderation actions.",
                color=0xEF4444,
            )
            return
        user_id, user_data = _get_user_option(options, resolved_users, "user")
        reason = _option_value(options, "reason")
        if not user_id:
            await _respond_embed(
                application_id=application_id,
                interaction_token=interaction_token,
                title="Missing user",
                description="Select a user to ban.",
                color=0xEF4444,
            )
            return
        await ban_member(
            bot_token=config.discord_token,
            guild_id=guild_id,
            user_id=user_id,
            reason=reason,
        )
        await log_moderation_action(
            db,
            guild_id=guild_id,
            action="ban",
            reason=reason,
            channel_id=channel_id or None,
            author_id=user_id,
            actor_id=author_id,
            actor_type="human",
            target_id=user_id,
            bot_id=bot_id,
            source="command",
        )
        await record_ban_action(
            db,
            guild_id=guild_id,
            user_id=user_id,
            moderator_id=author_id,
            moderator_name=author_name,
            reason=reason,
        )
        await _send_ban_dm(config=config, user_id=user_id, guild_id=guild_id, reason=reason)
        description = f"Banned {_format_user(user_id, user_data)}."
        if reason:
            description += f"\nReason: {reason}"
        await _respond_embed(
            application_id=application_id,
            interaction_token=interaction_token,
            title="User banned",
            description=description,
            color=0xEF4444,
        )
        return

    if command_name == "unban":
        if not _has_any_permission(perms, [hikari.Permissions.BAN_MEMBERS]):
            await _respond_embed(
                application_id=application_id,
                interaction_token=interaction_token,
                title="Missing permissions",
                description="You need Ban Members to unban users.",
                color=0xEF4444,
            )
            return
        if not config.discord_token:
            await _respond_embed(
                application_id=application_id,
                interaction_token=interaction_token,
                title="Bot token missing",
                description="Discord token not configured for moderation actions.",
                color=0xEF4444,
            )
            return
        raw_user_id = _option_value(options, "user_id")
        if not raw_user_id:
            await _respond_embed(
                application_id=application_id,
                interaction_token=interaction_token,
                title="Missing user id",
                description="Provide a user ID to unban.",
            )
            return
        user_id = "".join(ch for ch in str(raw_user_id) if ch.isdigit())
        if not user_id:
            await _respond_embed(
                application_id=application_id,
                interaction_token=interaction_token,
                title="Invalid user id",
                description="Provide a valid user ID.",
            )
            return
        await unban_member(
            bot_token=config.discord_token,
            guild_id=guild_id,
            user_id=user_id,
        )
        await log_moderation_action(
            db,
            guild_id=guild_id,
            action="unban",
            reason=None,
            channel_id=channel_id or None,
            author_id=user_id,
            actor_id=author_id,
            actor_type="human",
            target_id=user_id,
            bot_id=bot_id,
            source="command",
        )
        await _respond_embed(
            application_id=application_id,
            interaction_token=interaction_token,
            title="User unbanned",
            description=f"Unbanned <@{user_id}>.",
            color=0x22C55E,
        )
        return

    await _respond_embed(
        application_id=application_id,
        interaction_token=interaction_token,
        title="Unknown command",
        description="Try /help for a list of commands.",
    )


def main() -> None:
    run_worker("commands", handle_message)


if __name__ == "__main__":
    main()
