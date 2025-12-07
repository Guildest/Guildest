import asyncio
import logging
from typing import Any, Dict

import hikari

from .config import GatewayConfig
from .publisher import QueuePublisher


async def build_bot(config: GatewayConfig) -> hikari.GatewayBot:
    """Create and wire the Hikari gateway bot."""

    intents = hikari.Intents.GUILD_MESSAGES | hikari.Intents.MESSAGE_CONTENT
    bot = hikari.GatewayBot(token=config.discord_token, intents=intents)

    publisher = QueuePublisher(
        redis_url=config.redis_url,
        stream_name=config.queue_stream,
        max_length=config.queue_max_length,
    )

    @bot.listen(hikari.StartingEvent)
    async def on_starting(_: hikari.StartingEvent) -> None:
        logging.info("Discord gateway starting up")

    @bot.listen(hikari.StoppingEvent)
    async def on_stopping(_: hikari.StoppingEvent) -> None:
        logging.info("Discord gateway shutting down; closing Redis connection")
        await publisher.close()

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
