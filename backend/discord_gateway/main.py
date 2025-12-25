import asyncio
import logging
from datetime import datetime, timedelta, timezone
from typing import Any, Dict, Optional

import hikari

from .config import GatewayConfig
from .app_commands import build_application_commands, register_application_commands
from .publisher import QueuePublisher
from backend.database.db import (
    Database,
    create_pool,
    fetch_guild_plan,
    fetch_latest_sentiment,
    fetch_message_count_sum,
    fetch_moderation_logs,
    init_db,
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
        if not isinstance(interaction, hikari.CommandInteraction):
            return

        name = interaction.command_name
        logging.info("Slash command %s guild=%s", name, getattr(interaction, "guild_id", None))

        async def respond(text: str) -> None:
            await interaction.create_initial_response(
                hikari.ResponseType.MESSAGE_CREATE,
                text,
                flags=hikari.MessageFlag.EPHEMERAL,
            )

        if name == "ping":
            await respond("Pong.")
            return

        if name == "help":
            await respond(
                "\n".join(
                    [
                        "Commands:",
                        "/ping",
                        "/help",
                        "/dashboard",
                        "/stats",
                        "/sentiment",
                        "/modlogs (Plus/Premium)",
                    ]
                )
            )
            return

        if name == "dashboard":
            base = (config.frontend_base_url or "http://localhost:3000").rstrip("/")
            await respond(f"Dashboard: {base}")
            return

        guild_id = str(getattr(interaction, "guild_id", "") or "")
        if guild_id == "":
            await respond("This command can only be used in a server.")
            return

        if not db:
            await respond("Database not configured for this bot; set DATABASE_URL for DB-backed commands.")
            return

        if name == "stats":
            now = datetime.now(timezone.utc)
            last_hour = await fetch_message_count_sum(db, guild_id, now - timedelta(hours=1), now)
            last_day = await fetch_message_count_sum(db, guild_id, now - timedelta(hours=24), now)
            await respond(f"Message stats:\n- last hour: {last_hour}\n- last 24h: {last_day}")
            return

        if name == "sentiment":
            latest = await fetch_latest_sentiment(db, guild_id)
            if not latest:
                await respond("No sentiment data yet.")
                return
            score = latest.get("score")
            score_str = f"{score:.3f}" if isinstance(score, (int, float)) else "n/a"
            await respond(f"Latest sentiment ({latest['day']}): {latest['sentiment']} (score {score_str})")
            return

        if name == "modlogs":
            plan = await fetch_guild_plan(db, guild_id)
            if plan not in {"plus", "premium"}:
                await respond("Moderation log history is a paid feature.")
                return

            rows = await fetch_moderation_logs(db, guild_id, limit=5)
            if not rows:
                await respond("No moderation events yet.")
                return

            lines = ["Latest moderation events:"]
            for row in rows:
                when = row["created_at"]
                action = row.get("action") or "event"
                reason = row.get("reason") or ""
                lines.append(f"- {when}: {action} {reason}".strip())
            await respond("\n".join(lines))
            return

        await respond("Unknown command. Try /help.")

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
