import asyncio
import os
import uuid

import pytest

from backend.queue.client import QueueConsumer, QueuePublisher

pytestmark = pytest.mark.asyncio


@pytest.mark.timeout(10)
async def test_queue_publish_and_consume() -> None:
    redis_url = os.getenv("REDIS_URL", "redis://localhost:6379/0")
    stream_name = f"test:guildest:{uuid.uuid4()}"
    group_name = f"test-group-{uuid.uuid4()}"

    publisher = QueuePublisher(redis_url=redis_url, stream_name=stream_name, max_length=100)
    consumer = QueueConsumer(
        redis_url=redis_url,
        stream_name=stream_name,
        group_name=group_name,
        consumer_name="consumer-1",
    )

    received_ids = []

    async def handler(message) -> None:
        received_ids.append(message.message_id)

    consumer_task = asyncio.create_task(
        consumer.read_forever(handler, block_ms=500, count=10, stop_after=1),
    )

    payload = {
        "event": "MESSAGE_CREATE",
        "message_id": "mid-1",
        "guild_id": "gid-1",
        "channel_id": "cid-1",
        "author_id": "aid-1",
        "content": "hello",
        "timestamp": "2025-01-01T00:00:00Z",
        "metadata": {},
    }
    await publisher.publish(payload)

    await asyncio.wait_for(consumer_task, timeout=5)

    assert received_ids == ["mid-1"]

    await publisher.close()
    await consumer.close()
