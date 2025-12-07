import logging
from typing import Optional

from backend.common.config import AppConfig
from backend.common.models import QueueMessage
from backend.database.db import Database, log_moderation_event
from backend.workers.consumer import run_worker


async def handle_message(message: QueueMessage, _: AppConfig, db: Optional[Database]) -> None:
    # Placeholder moderation logic; replace with LLM safety checks.
    action = "reviewed"
    reason = "placeholder check"
    logging.info("[moderation] %s message %s in guild %s", action, message.message_id, message.guild_id)

    if db:
        await log_moderation_event(db, message, action=action, reason=reason)
        logging.debug("[moderation] logged moderation event for message %s", message.message_id)


def main() -> None:
    run_worker("moderation", handle_message)


if __name__ == "__main__":
    main()
