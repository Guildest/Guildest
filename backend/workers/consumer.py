import asyncio
import logging
from typing import Awaitable, Callable, Optional

from backend.common.config import AppConfig, load_app_config
from backend.common.logging import configure_logging
from backend.common.models import QueueMessage
from backend.database.db import Database, create_pool, init_db
from backend.queue.client import QueueConsumer

MessageHandler = Callable[[QueueMessage, AppConfig, Optional[Database]], Awaitable[None]]


async def _consume(worker_name: str, handler: MessageHandler, config: AppConfig) -> None:
    consumer = QueueConsumer(
        redis_url=config.redis.url,
        stream_name=config.queue.stream,
        group_name=config.queue.group_name,
        consumer_name=config.queue.consumer_name or worker_name,
    )

    db: Optional[Database] = None
    if config.database_url:
        db = await create_pool(config.database_url)
        await init_db(db)
        logging.info("[%s] connected to database", worker_name)
    else:
        logging.warning("[%s] DATABASE_URL not set; running without persistence", worker_name)

    logging.info(
        "[%s] consuming stream=%s group=%s consumer=%s",
        worker_name,
        config.queue.stream,
        config.queue.group_name,
        config.queue.consumer_name or worker_name,
    )

    try:
        await consumer.read_forever(lambda message: handler(message, config, db))
    finally:
        if db:
            await db.close()
            logging.info("[%s] database connection closed", worker_name)


def run_worker(worker_name: str, handler: MessageHandler) -> None:
    config = load_app_config()
    configure_logging(config.log_level)
    try:
        asyncio.run(_consume(worker_name, handler, config))
    except KeyboardInterrupt:
        logging.info("[%s] shutting down on interrupt", worker_name)
