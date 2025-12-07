import json
from typing import Any, Dict, Optional

from redis.asyncio import Redis


class QueuePublisher:
    """Publish normalized gateway events to a Redis Stream."""

    def __init__(
        self,
        redis_url: str,
        stream_name: str,
        max_length: int,
    ) -> None:
        self._stream_name = stream_name
        self._max_length = max_length
        self._redis = Redis.from_url(redis_url)

    async def publish(self, payload: Dict[str, Any]) -> Optional[str]:
        """Push an event payload onto the stream; returns entry ID."""

        # Redis Streams store field-value pairs; we serialize payload once.
        serialized = json.dumps(payload, separators=(",", ":"), ensure_ascii=True)
        return await self._redis.xadd(
            name=self._stream_name,
            fields={"data": serialized},
            maxlen=self._max_length,
            approximate=True,
        )

    async def close(self) -> None:
        await self._redis.aclose()
