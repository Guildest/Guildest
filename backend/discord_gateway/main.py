import asyncio
import logging
import uuid
from datetime import datetime, timezone
from typing import Any, Dict, Optional

import hikari

from .config import GatewayConfig
from .app_commands import build_application_commands, register_application_commands
from .publisher import QueuePublisher
from backend.database.db import (
    Database,
    create_pool,
    create_appeal,
    fetch_latest_ban_record,
    init_db,
    is_appeal_blocked,
)


async def build_bot(config: GatewayConfig) -> hikari.GatewayBot:
    """Create and wire the Hikari gateway bot."""

    intents = (
        hikari.Intents.GUILD_MESSAGES
        | hikari.Intents.MESSAGE_CONTENT
        | hikari.Intents.GUILD_VOICE_STATES
    )
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
        logging.info("Discord gateway connected to database for appeals")

    def _serialize_user(user: hikari.User) -> dict[str, Any]:
        return {
            "id": str(user.id),
            "username": user.username,
            "discriminator": user.discriminator,
            "avatar": str(user.avatar_hash) if user.avatar_hash else None,
            "global_name": getattr(user, "global_name", None),
        }

    def _serialize_options(options: Optional[list[hikari.CommandInteractionOption]]) -> list[dict[str, Any]]:
        if not options:
            return []
        payload: list[dict[str, Any]] = []
        for option in options:
            data: dict[str, Any] = {"name": option.name, "type": int(option.type)}
            value = option.value
            if option.type in {
                hikari.OptionType.USER,
                hikari.OptionType.ROLE,
                hikari.OptionType.CHANNEL,
                hikari.OptionType.MENTIONABLE,
            }:
                data["value"] = str(value) if value is not None else None
            elif isinstance(value, (str, int, float, bool)) or value is None:
                data["value"] = value
            else:
                data["value"] = str(value)
            if option.options:
                data["options"] = _serialize_options(option.options)
            payload.append(data)
        return payload

    def _build_command_payload(interaction: hikari.CommandInteraction) -> Dict[str, Any]:
        resolved_users: dict[str, Any] = {}
        if interaction.resolved and interaction.resolved.users:
            for snowflake, user in interaction.resolved.users.items():
                resolved_users[str(snowflake)] = _serialize_user(user)
        author = _serialize_user(interaction.user)
        member_permissions = int(interaction.member.permissions) if interaction.member else 0
        timestamp = (interaction.created_at or datetime.now(timezone.utc)).isoformat()
        return {
            "event": "COMMAND_INTERACTION",
            "message_id": str(interaction.id),
            "guild_id": str(interaction.guild_id) if interaction.guild_id else "",
            "channel_id": str(interaction.channel_id) if interaction.channel_id else "",
            "author_id": str(interaction.user.id),
            "content": interaction.command_name,
            "timestamp": timestamp,
            "metadata": {
                "interaction_id": str(interaction.id),
                "interaction_token": interaction.token,
                "application_id": str(interaction.application_id) if interaction.application_id else None,
                "command_name": interaction.command_name,
                "options": _serialize_options(interaction.options),
                "member_permissions": member_permissions,
                "user": author,
                "resolved_users": resolved_users,
            },
        }

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

        if event.is_bot:
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
                "mentions_self": bool(getattr(event.message, "mentions_self", False)),
            },
        }

        try:
            entry_id = await publisher.publish(payload)
            logging.debug("Enqueued message %s to %s (%s)", event.message_id, config.queue_stream, entry_id)
        except Exception as exc:  # noqa: BLE001
            logging.exception("Failed to publish message %s: %s", event.message_id, exc)

    @bot.listen(hikari.VoiceStateUpdateEvent)
    async def on_voice_state(event: hikari.VoiceStateUpdateEvent) -> None:
        state = event.state
        old_state = event.old_state
        guild_id = None
        if state and state.guild_id:
            guild_id = str(state.guild_id)
        elif old_state and old_state.guild_id:
            guild_id = str(old_state.guild_id)
        if not guild_id:
            return

        before_channel = str(old_state.channel_id) if old_state and old_state.channel_id else None
        after_channel = str(state.channel_id) if state and state.channel_id else None
        channel_id = after_channel or before_channel or ""
        user_id = str(state.user_id) if state else (str(old_state.user_id) if old_state else "")

        payload: Dict[str, Any] = {
            "event": "VOICE_STATE_UPDATE",
            "message_id": str(uuid.uuid4()),
            "guild_id": guild_id,
            "channel_id": channel_id,
            "author_id": user_id,
            "content": "",
            "timestamp": datetime.now(timezone.utc).isoformat(),
            "user_id": user_id,
            "metadata": {
                "before": {"channel_id": before_channel},
                "after": {"channel_id": after_channel},
            },
        }

        try:
            entry_id = await publisher.publish(payload)
            logging.debug("Enqueued voice update %s to %s (%s)", user_id, config.queue_stream, entry_id)
        except Exception as exc:  # noqa: BLE001
            logging.exception("Failed to publish voice update for %s: %s", user_id, exc)

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

        if not isinstance(interaction, hikari.CommandInteraction):
            guild_id = str(getattr(interaction, "guild_id", "") or "")
            channel_id = str(getattr(interaction, "channel_id", "") or "")
            author_id = str(interaction.user.id)
            metadata: dict[str, Any] = {
                "interaction_type": str(getattr(interaction, "type", "")),
            }
            if isinstance(interaction, hikari.ComponentInteraction):
                metadata["custom_id"] = interaction.custom_id
            if isinstance(interaction, hikari.ModalInteraction):
                metadata["custom_id"] = interaction.custom_id
            payload = {
                "event": "INTERACTION_CREATE",
                "message_id": str(interaction.id),
                "guild_id": guild_id,
                "channel_id": channel_id,
                "author_id": author_id,
                "content": "",
                "timestamp": datetime.now(timezone.utc).isoformat(),
                "metadata": metadata,
            }
            try:
                await publisher.publish(payload)
            except Exception as exc:  # noqa: BLE001
                logging.exception("Failed to publish interaction %s: %s", interaction.id, exc)

        if isinstance(interaction, hikari.CommandInteraction):
            logging.info("Slash command %s guild=%s", interaction.command_name, interaction.guild_id)
            try:
                await interaction.create_initial_response(
                    hikari.ResponseType.DEFERRED_MESSAGE_CREATE,
                    flags=hikari.MessageFlag.EPHEMERAL,
                )
            except Exception as exc:  # noqa: BLE001
                logging.exception("Failed to defer command interaction %s: %s", interaction.id, exc)
                return

            payload = _build_command_payload(interaction)
            try:
                entry_id = await publisher.publish(payload)
                logging.debug("Enqueued command %s to %s (%s)", interaction.id, config.queue_stream, entry_id)
            except Exception as exc:  # noqa: BLE001
                logging.exception("Failed to publish command %s: %s", interaction.id, exc)
                try:
                    await interaction.edit_initial_response(
                        embed=hikari.Embed(
                            title="Command failed",
                            description="Unable to reach the commands worker. Try again shortly.",
                            color=0xEF4444,
                        )
                    )
                except Exception as edit_exc:  # noqa: BLE001
                    logging.exception("Failed to edit command response: %s", edit_exc)
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
