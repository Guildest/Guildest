import asyncio
import logging
import uuid
from datetime import datetime, timedelta, timezone
from typing import Any, Dict, Optional

import hikari

from .config import GatewayConfig
from .app_commands import build_application_commands, register_application_commands
from .publisher import QueuePublisher
from backend.common.discord_rest import (
    ban_member,
    create_dm_channel,
    send_channel_message,
    timeout_member,
    unban_member,
)
from backend.database.db import (
    Database,
    create_pool,
    create_appeal,
    fetch_active_warns,
    fetch_guild_plan,
    fetch_guild_settings,
    fetch_latest_ban_record,
    fetch_latest_sentiment,
    fetch_message_count_sum,
    fetch_moderation_logs,
    insert_guild_warn,
    init_db,
    is_appeal_blocked,
    clear_warns,
    log_moderation_action,
    record_ban_action,
)


async def build_bot(config: GatewayConfig) -> hikari.GatewayBot:
    """Create and wire the Hikari gateway bot."""

    intents = hikari.Intents.GUILD_MESSAGES | hikari.Intents.MESSAGE_CONTENT
    bot = hikari.GatewayBot(token=config.discord_token, intents=intents)

    publisher = QueuePublisher(
        redis_url=config.redis_url,
        stream_name=config.queue_stream,
        max_length=config.queue_max_length,
    )

    db: Optional[Database] = None
    if config.database_url:
        db = await create_pool(config.database_url)
        await init_db(db)
        logging.info("Discord gateway connected to database for slash commands")

    def _option_value(interaction: hikari.CommandInteraction, name: str) -> Optional[Any]:
        if not interaction.options:
            return None
        for option in interaction.options:
            if option.name == name:
                return option.value
        return None

    def _get_user_option(
        interaction: hikari.CommandInteraction, name: str
    ) -> tuple[Optional[str], Optional[hikari.User], Optional[hikari.Member]]:
        value = _option_value(interaction, name)
        if value is None:
            return None, None, None
        user_id = str(value)
        resolved = interaction.resolved
        user = resolved.users.get(value) if resolved and resolved.users else None
        member = resolved.members.get(value) if resolved and resolved.members else None
        return user_id, user, member

    def _format_user(user_id: str, user: Optional[hikari.User]) -> str:
        if user:
            return f"{user.username}#{user.discriminator} (<@{user_id}>)"
        return f"<@{user_id}>"

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

    async def _log_action(
        *,
        guild_id: str,
        action: str,
        target_id: Optional[str],
        reason: Optional[str] = None,
        actor_id: Optional[str] = None,
        actor_type: Optional[str] = None,
        source: Optional[str] = None,
        channel_id: Optional[str] = None,
        metadata: Optional[dict[str, Any]] = None,
    ) -> None:
        if not db:
            return
        await log_moderation_action(
            db,
            guild_id=guild_id,
            action=action,
            reason=reason,
            channel_id=channel_id,
            author_id=target_id,
            actor_id=actor_id,
            actor_type=actor_type,
            target_id=target_id,
            bot_id=config.discord_application_id,
            source=source,
            metadata=metadata,
        )

    async def _send_ban_dm(
        *,
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
            logging.warning("Failed to DM ban appeal to user %s: %s", user_id, exc)

    @bot.listen(hikari.StartingEvent)
    async def on_starting(_: hikari.StartingEvent) -> None:
        logging.info("Discord gateway starting up")
        if config.discord_application_id:
            try:
                commands = build_application_commands()
                result = await register_application_commands(
                    bot_token=config.discord_token,
                    application_id=config.discord_application_id,
                    commands=commands,
                    guild_id=config.commands_guild_id,
                )
                logging.info("Registered %s app commands (%s)", result.count, result.scope)
            except Exception as exc:  # noqa: BLE001
                logging.exception("Failed to register application commands: %s", exc)
        else:
            logging.info("DISCORD_APPLICATION_ID not set; skipping slash command registration")

    @bot.listen(hikari.StoppingEvent)
    async def on_stopping(_: hikari.StoppingEvent) -> None:
        logging.info("Discord gateway shutting down; closing Redis connection")
        await publisher.close()
        if db:
            await db.close()
            logging.info("Discord gateway database connection closed")

    @bot.listen(hikari.GuildMessageCreateEvent)
    async def on_message(event: hikari.GuildMessageCreateEvent) -> None:
        """Handle incoming guild messages and push to the queue."""

        if event.is_bot or event.content is None:
            return

        payload: Dict[str, Any] = {
            "event": "MESSAGE_CREATE",
            "message_id": str(event.message_id),
            "guild_id": str(event.guild_id),
            "channel_id": str(event.channel_id),
            "author_id": str(event.author_id),
            "content": event.content,
            "timestamp": (event.message.created_at or event.timestamp).isoformat(),
            "metadata": {
                "is_webhook": event.is_webhook,
                "mentions_self": event.message.mentions_self,
            },
        }

        try:
            entry_id = await publisher.publish(payload)
            logging.debug("Enqueued message %s to %s (%s)", event.message_id, config.queue_stream, entry_id)
        except Exception as exc:  # noqa: BLE001
            logging.exception("Failed to publish message %s: %s", event.message_id, exc)

    @bot.listen(hikari.InteractionCreateEvent)
    async def on_interaction(event: hikari.InteractionCreateEvent) -> None:
        interaction = event.interaction
        async def respond_embed(
            *,
            title: str,
            description: str,
            color: int = 0x6366F1,
            ephemeral: bool = True,
        ) -> None:
            embed = hikari.Embed(title=title, description=description, color=color)
            await interaction.create_initial_response(
                hikari.ResponseType.MESSAGE_CREATE,
                embed=embed,
                flags=hikari.MessageFlag.EPHEMERAL if ephemeral else None,
            )

        if isinstance(interaction, hikari.CommandInteraction):
            name = interaction.command_name
            logging.info("Slash command %s guild=%s", name, getattr(interaction, "guild_id", None))

            def has_any_permission(perms: list[hikari.Permissions]) -> bool:
                member = interaction.member
                if not member:
                    return False
                if member.permissions & hikari.Permissions.ADMINISTRATOR:
                    return True
                return any(member.permissions & perm for perm in perms)

            if name == "ping":
                await respond_embed(title="Pong", description="Bot is alive.", color=0x22C55E)
                return

            if name == "help":
                await respond_embed(
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

            if name == "dashboard":
                base = (config.frontend_base_url or "http://localhost:3000").rstrip("/")
                await respond_embed(
                    title="Dashboard",
                    description=f"{base}\nConnect this server in the dashboard to enable paid features.",
                    color=0x38BDF8,
                )
                return

            guild_id = str(getattr(interaction, "guild_id", "") or "")
            if guild_id == "":
                await respond_embed(
                    title="Server only",
                    description="This command can only be used in a server.",
                    color=0xF97316,
                )
                return

            if not db:
                await respond_embed(
                    title="Database unavailable",
                    description="Database not configured for this bot; set DATABASE_URL for DB-backed commands.",
                    color=0xF97316,
                )
                return

            if name == "stats":
                now = datetime.now(timezone.utc)
                last_hour = await fetch_message_count_sum(db, guild_id, now - timedelta(hours=1), now)
                last_day = await fetch_message_count_sum(db, guild_id, now - timedelta(hours=24), now)
                await respond_embed(
                    title="Message stats",
                    description=f"Last hour: {last_hour}\nLast 24h: {last_day}",
                    color=0x22C55E,
                )
                return

            if name == "sentiment":
                latest = await fetch_latest_sentiment(db, guild_id)
                if not latest:
                    await respond_embed(title="Sentiment", description="No sentiment data yet.", color=0xF97316)
                    return
                score = latest.get("score")
                score_str = f"{score:.3f}" if isinstance(score, (int, float)) else "n/a"
                await respond_embed(
                    title="Latest sentiment",
                    description=f"{latest['day']}: {latest['sentiment']} (score {score_str})",
                    color=0x38BDF8,
                )
                return

            if name == "modlogs":
                plan = await fetch_guild_plan(db, guild_id)
                if plan not in {"plus", "premium"}:
                    await respond_embed(
                        title="Moderation logs",
                        description="Moderation log history is a paid feature.",
                        color=0xF97316,
                    )
                    return

                rows = await fetch_moderation_logs(db, guild_id, limit=5)
                if not rows:
                    await respond_embed(title="Moderation logs", description="No moderation events yet.")
                    return

                lines = []
                for row in rows:
                    when = row["created_at"]
                    action = row.get("action") or "event"
                    reason = row.get("reason") or ""
                    lines.append(f"- {when}: {action} {reason}".strip())
                await respond_embed(
                    title="Latest moderation events",
                    description="\n".join(lines),
                    color=0x38BDF8,
                )
                return

            if name in {"warn", "warns", "warn-clear"}:
                if not has_any_permission([hikari.Permissions.MODERATE_MEMBERS, hikari.Permissions.MANAGE_MESSAGES]):
                    await respond_embed(
                        title="Missing permissions",
                        description="You need Moderate Members or Manage Messages to use warnings.",
                        color=0xEF4444,
                    )
                    return

                user_id, user, _member = _get_user_option(interaction, "user")
                if not user_id:
                    await respond_embed(title="Missing user", description="Select a user to continue.", color=0xEF4444)
                    return

                now = datetime.now(timezone.utc)

                if name == "warns":
                    warns = await fetch_active_warns(db, guild_id, user_id, now)
                    if not warns:
                        await respond_embed(
                            title="Warnings",
                            description=f"{_format_user(user_id, user)} has no active warnings.",
                            color=0x22C55E,
                        )
                        return
                    lines = []
                    for warn in warns[:5]:
                        reason = warn.get("reason") or "No reason provided"
                        lines.append(f"- {warn['created_at']}: {reason}")
                    await respond_embed(
                        title="Warnings",
                        description=f"{_format_user(user_id, user)} has {len(warns)} warning(s).\n" + "\n".join(lines),
                        color=0xF59E0B,
                    )
                    return

                if name == "warn-clear":
                    cleared = await clear_warns(db, guild_id, user_id)
                    await _log_action(
                        guild_id=guild_id,
                        action="warn_clear",
                        target_id=user_id,
                        actor_id=str(interaction.user.id),
                        actor_type="human",
                        source="command",
                        channel_id=str(interaction.channel_id) if interaction.channel_id else None,
                        metadata={"cleared": cleared},
                    )
                    await respond_embed(
                        title="Warnings cleared",
                        description=f"Cleared {cleared} warning(s) for {_format_user(user_id, user)}.",
                        color=0x22C55E,
                    )
                    return

                reason = _option_value(interaction, "reason")
                settings = await fetch_guild_settings(db, guild_id)
                expires_at = None
                if settings.warn_decay_days and settings.warn_decay_days > 0:
                    expires_at = now + timedelta(days=settings.warn_decay_days)
                await insert_guild_warn(
                    db,
                    guild_id=guild_id,
                    user_id=user_id,
                    moderator_id=str(interaction.user.id),
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
                        await timeout_member(
                            bot_token=config.discord_token,
                            guild_id=guild_id,
                            user_id=user_id,
                            communication_disabled_until=until.isoformat(),
                            reason=f"Warn threshold reached ({warn_count})",
                        )
                        await _log_action(
                            guild_id=guild_id,
                            action="timeout",
                            target_id=user_id,
                            actor_id=config.discord_application_id,
                            actor_type="bot" if config.discord_application_id else "system",
                            source="automod",
                            channel_id=str(interaction.channel_id) if interaction.channel_id else None,
                            reason="Warn threshold reached",
                            metadata={"warn_count": warn_count, "duration_hours": duration_hours},
                        )
                        action_note = f"Auto-timeout applied ({duration_hours}h)."
                    elif action["action"] == "ban":
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
                            moderator_id=str(interaction.user.id),
                            moderator_name=interaction.user.username,
                            reason="Warn threshold reached",
                        )
                        await _send_ban_dm(user_id=user_id, guild_id=guild_id, reason="Warn threshold reached")
                        await _log_action(
                            guild_id=guild_id,
                            action="ban",
                            target_id=user_id,
                            actor_id=config.discord_application_id,
                            actor_type="bot" if config.discord_application_id else "system",
                            source="automod",
                            channel_id=str(interaction.channel_id) if interaction.channel_id else None,
                            reason="Warn threshold reached",
                            metadata={"warn_count": warn_count},
                        )
                        action_note = "Auto-ban applied."

                await _log_action(
                    guild_id=guild_id,
                    action="warn",
                    target_id=user_id,
                    actor_id=str(interaction.user.id),
                    actor_type="human",
                    source="command",
                    channel_id=str(interaction.channel_id) if interaction.channel_id else None,
                    reason=reason,
                    metadata={"warn_count": warn_count},
                )

                description = f"{_format_user(user_id, user)} now has {warn_count} warning(s)."
                if reason:
                    description += f"\nReason: {reason}"
                if action_note:
                    description += f"\n{action_note}"
                await respond_embed(title="Warning issued", description=description, color=0xF59E0B)
                return

            if name == "timeout":
                if not has_any_permission([hikari.Permissions.MODERATE_MEMBERS]):
                    await respond_embed(
                        title="Missing permissions",
                        description="You need Moderate Members to timeout users.",
                        color=0xEF4444,
                    )
                    return

                user_id, user, _member = _get_user_option(interaction, "user")
                minutes = _option_value(interaction, "minutes")
                reason = _option_value(interaction, "reason")
                if not user_id or minutes is None:
                    await respond_embed(title="Missing input", description="User and minutes are required.")
                    return
                try:
                    minutes_value = int(minutes)
                except (TypeError, ValueError):
                    await respond_embed(title="Invalid duration", description="Minutes must be a number.")
                    return
                if minutes_value < 1 or minutes_value > 40320:
                    await respond_embed(title="Invalid duration", description="Timeout must be 1-40320 minutes.")
                    return
                until = datetime.now(timezone.utc) + timedelta(minutes=minutes_value)
                await timeout_member(
                    bot_token=config.discord_token,
                    guild_id=guild_id,
                    user_id=user_id,
                    communication_disabled_until=until.isoformat(),
                    reason=reason,
                )
                await _log_action(
                    guild_id=guild_id,
                    action="timeout",
                    target_id=user_id,
                    actor_id=str(interaction.user.id),
                    actor_type="human",
                    source="command",
                    channel_id=str(interaction.channel_id) if interaction.channel_id else None,
                    reason=reason,
                    metadata={"duration_minutes": minutes_value},
                )
                description = f"Timed out {_format_user(user_id, user)} for {minutes_value} minutes."
                if reason:
                    description += f"\nReason: {reason}"
                await respond_embed(title="Timeout applied", description=description, color=0xF97316)
                return

            if name == "ban":
                if not has_any_permission([hikari.Permissions.BAN_MEMBERS]):
                    await respond_embed(
                        title="Missing permissions",
                        description="You need Ban Members to ban users.",
                        color=0xEF4444,
                    )
                    return

                user_id, user, _member = _get_user_option(interaction, "user")
                reason = _option_value(interaction, "reason")
                if not user_id:
                    await respond_embed(title="Missing user", description="Select a user to ban.", color=0xEF4444)
                    return
                await ban_member(
                    bot_token=config.discord_token,
                    guild_id=guild_id,
                    user_id=user_id,
                    reason=reason,
                )
                await _log_action(
                    guild_id=guild_id,
                    action="ban",
                    target_id=user_id,
                    actor_id=str(interaction.user.id),
                    actor_type="human",
                    source="command",
                    channel_id=str(interaction.channel_id) if interaction.channel_id else None,
                    reason=reason,
                )
                await record_ban_action(
                    db,
                    guild_id=guild_id,
                    user_id=user_id,
                    moderator_id=str(interaction.user.id),
                    moderator_name=interaction.user.username,
                    reason=reason,
                )
                await _send_ban_dm(user_id=user_id, guild_id=guild_id, reason=reason)
                description = f"Banned {_format_user(user_id, user)}."
                if reason:
                    description += f"\nReason: {reason}"
                await respond_embed(title="User banned", description=description, color=0xEF4444)
                return

            if name == "unban":
                if not has_any_permission([hikari.Permissions.BAN_MEMBERS]):
                    await respond_embed(
                        title="Missing permissions",
                        description="You need Ban Members to unban users.",
                        color=0xEF4444,
                    )
                    return

                raw_user_id = _option_value(interaction, "user_id")
                if not raw_user_id:
                    await respond_embed(title="Missing user id", description="Provide a user ID to unban.")
                    return
                user_id = "".join(ch for ch in str(raw_user_id) if ch.isdigit())
                if not user_id:
                    await respond_embed(title="Invalid user id", description="Provide a valid user ID.")
                    return
                await unban_member(
                    bot_token=config.discord_token,
                    guild_id=guild_id,
                    user_id=user_id,
                )
                await _log_action(
                    guild_id=guild_id,
                    action="unban",
                    target_id=user_id,
                    actor_id=str(interaction.user.id),
                    actor_type="human",
                    source="command",
                    channel_id=str(interaction.channel_id) if interaction.channel_id else None,
                )
                await respond_embed(title="User unbanned", description=f"Unbanned <@{user_id}>.", color=0x22C55E)
                return

            await respond_embed(title="Unknown command", description="Try /help for a list of commands.")
            return

        if isinstance(interaction, hikari.ComponentInteraction):
            if not db:
                await respond_embed(
                    title="Appeals unavailable",
                    description="Database not configured for appeals.",
                    color=0xF97316,
                )
                return
            custom_id = interaction.custom_id or ""
            if custom_id.startswith("appeal:"):
                guild_id = custom_id.split(":", 1)[1]
                user_id = str(interaction.user.id)
                if await is_appeal_blocked(db, guild_id, user_id):
                    await respond_embed(
                        title="Appeals disabled",
                        description="You are not allowed to submit appeals for this guild.",
                        color=0xEF4444,
                    )
                    return
                row = hikari.impl.ModalActionRowBuilder().add_text_input(
                    "appeal_text",
                    "Why should this ban be lifted?",
                    style=hikari.TextInputStyle.PARAGRAPH,
                    min_length=10,
                    max_length=1000,
                    required=True,
                )
                await interaction.create_modal_response(
                    "Ban appeal",
                    custom_id=f"appeal_modal:{guild_id}",
                    components=[row],
                )
                return

        if isinstance(interaction, hikari.ModalInteraction):
            if not db:
                await respond_embed(
                    title="Appeals unavailable",
                    description="Database not configured for appeals.",
                    color=0xF97316,
                )
                return
            custom_id = interaction.custom_id or ""
            if custom_id.startswith("appeal_modal:"):
                guild_id = custom_id.split(":", 1)[1]
                user_id = str(interaction.user.id)
                if await is_appeal_blocked(db, guild_id, user_id):
                    await respond_embed(
                        title="Appeals disabled",
                        description="You are not allowed to submit appeals for this guild.",
                        color=0xEF4444,
                    )
                    return
                appeal_text = None
                for row in interaction.components:
                    for component in row.components:
                        if getattr(component, "custom_id", "") == "appeal_text":
                            appeal_text = component.value
                            break
                    if appeal_text:
                        break
                if not appeal_text:
                    await respond_embed(title="Appeal missing", description="Please provide appeal text.")
                    return

                ban_record = await fetch_latest_ban_record(db, guild_id, user_id)
                appeal_id = uuid.uuid4()
                await create_appeal(
                    db,
                    appeal_id=appeal_id,
                    guild_id=guild_id,
                    user_id=user_id,
                    user_name=interaction.user.username,
                    user_avatar=interaction.user.avatar_hash or None,
                    moderator_id=ban_record["moderator_id"] if ban_record else None,
                    moderator_name=ban_record["moderator_name"] if ban_record else None,
                    ban_reason=ban_record["reason"] if ban_record else None,
                    appeal_text=appeal_text,
                )
                await respond_embed(
                    title="Appeal submitted",
                    description="Your appeal has been submitted for review.",
                    color=0x22C55E,
                )
                return

    return bot


def main() -> None:
    config = GatewayConfig.from_env()

    logging.basicConfig(
        level=config.log_level,
        format="%(asctime)s [%(levelname)s] %(name)s: %(message)s",
    )

    loop = asyncio.new_event_loop()
    asyncio.set_event_loop(loop)

    bot = loop.run_until_complete(build_bot(config))

    try:
        bot.run(
            status=hikari.Status.ONLINE,
            activity=hikari.Activity(name="Guildest", type=hikari.ActivityType.LISTENING),
        )
    finally:
        loop.stop()
        loop.close()


if __name__ == "__main__":
    main()
