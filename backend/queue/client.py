import asyncio
import json
import logging
from typing import Any, Awaitable, Callable, Dict, Iterable, Optional

from redis.asyncio import Redis
from redis.exceptions import ResponseError

from backend.common.models import QueueMessage


class QueuePublisher:
    """Publish normalized events to a Redis Stream."""

    def __init__(self, redis_url: str, stream_name: str, max_length: int) -> None:
        self._stream_name = stream_name
        self._max_length = max_length
        self._redis = Redis.from_url(redis_url)

    async def publish(self, payload: Dict[str, Any]) -> Optional[str]:
        serialized = json.dumps(payload, separators=(",", ":"), ensure_ascii=True)
        return await self._redis.xadd(
            name=self._stream_name,
            fields={"data": serialized},
            maxlen=self._max_length,
            approximate=True,
        )

    async def close(self) -> None:
        await self._redis.aclose()


class QueueConsumer:
    """Consume messages from a Redis Stream with consumer groups."""

    def __init__(
        self,
        redis_url: str,
        stream_name: str,
        group_name: str,
        consumer_name: str,
    ) -> None:
        self._stream_name = stream_name
        self._group_name = group_name
        self._consumer_name = consumer_name
        self._redis = Redis.from_url(redis_url)

    async def ensure_group(self) -> None:
        try:
            await self._redis.xgroup_create(
                name=self._stream_name,
                groupname=self._group_name,
                id="$",
                mkstream=True,
            )
            logging.info("Created consumer group '%s' on stream '%s'", self._group_name, self._stream_name)
        except ResponseError as exc:
            if "BUSYGROUP" in str(exc):
                logging.debug("Consumer group '%s' already exists", self._group_name)
            else:
                raise

    async def read_forever(
        self,
        handler: Callable[[QueueMessage], Awaitable[None]],
        block_ms: int = 5000,
        count: int = 10,
        stop_after: Optional[int] = None,
        stop_event: Optional[asyncio.Event] = None,
    ) -> None:
        await self.ensure_group()

        processed = 0
        while True:
            if stop_event and stop_event.is_set():
                break

            entries = await self._redis.xreadgroup(
                groupname=self._group_name,
                consumername=self._consumer_name,
                streams={self._stream_name: ">"},
                count=count,
                block=block_ms,
            )

            if not entries:
                continue

            for _, messages in entries:
                for message_id, fields in messages:
                    payload_raw = fields.get("data")
                    try:
                        payload_dict = _decode_payload(payload_raw)
                        message = QueueMessage.model_validate(payload_dict)
                    except Exception as exc:  # noqa: BLE001
                        logging.warning("Failed to parse message %s: %s", message_id, exc)
                        continue

                    try:
                        await handler(message)
                        await self._redis.xack(self._stream_name, self._group_name, message_id)
                    except Exception as exc:  # noqa: BLE001
                        logging.exception("Handler failed for message %s: %s", message_id, exc)

                    processed += 1
                    if stop_after is not None and processed >= stop_after:
                        return

    async def pending(self, count: int = 20) -> Iterable[str]:
        """List pending message IDs (debug helper)."""

        response = await self._redis.xpending_range(
            name=self._stream_name,
            groupname=self._group_name,
            min="-",
            max="+",
            count=count,
        )
        return [entry["message_id"] for entry in response]

    async def close(self) -> None:
        await self._redis.aclose()


def _decode_payload(value: Any) -> Dict[str, Any]:
    if value is None:
        raise ValueError("empty payload")
    if isinstance(value, (bytes, bytearray)):
        value = value.decode("utf-8")
    if isinstance(value, str):
        return json.loads(value)
    if isinstance(value, dict):
        return value
    raise TypeError(f"Unsupported payload type: {type(value)}")
