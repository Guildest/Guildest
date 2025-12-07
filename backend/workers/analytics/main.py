import logging
from collections import Counter
from typing import Dict, Optional

from backend.common.config import AppConfig
from backend.common.models import QueueMessage
from backend.database.db import Database, bump_message_count
from backend.workers.consumer import run_worker


message_buckets: Dict[str, Counter] = {}


async def handle_message(message: QueueMessage, _: AppConfig, db: Optional[Database]) -> None:
    # In-memory aggregation plus database counter when available.
    counter = message_buckets.setdefault(message.guild_id, Counter())
    counter["messages"] += 1
    logging.info("[analytics] guild=%s total_messages=%s", message.guild_id, counter["messages"])

    if db:
        await bump_message_count(db, message)
        logging.debug("[analytics] persisted message count for guild=%s", message.guild_id)


def main() -> None:
    run_worker("analytics", handle_message)


if __name__ == "__main__":
    main()
