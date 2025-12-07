import logging
from datetime import datetime, timezone
from typing import Optional

from backend.common.config import AppConfig
from backend.common.models import QueueMessage
from backend.database.db import Database, record_sentiment
from backend.workers.consumer import run_worker


async def handle_message(message: QueueMessage, _: AppConfig, db: Optional[Database]) -> None:
    # Placeholder sentiment logic; replace with summarization once analytics data exists.
    logging.info("[sentiment] observed message %s in guild %s", message.message_id, message.guild_id)

    if db:
        # Demo: mark sentiment as neutral for the day to validate DB writes.
        today = datetime.now(timezone.utc)
        await record_sentiment(db, message.guild_id, today, sentiment="neutral")
        logging.debug("[sentiment] recorded neutral sentiment for guild=%s day=%s", message.guild_id, today.date())


def main() -> None:
    run_worker("sentiment", handle_message)


if __name__ == "__main__":
    main()
