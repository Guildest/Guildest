from __future__ import annotations

from typing import Any, Optional

import httpx


DISCORD_API_BASE = "https://discord.com/api/v10"


async def send_channel_message(
    *,
    bot_token: str,
    channel_id: str,
    content: str,
    allowed_mentions: Optional[dict[str, Any]] = None,
    timeout_seconds: float = 15,
) -> None:
    headers = {
        "Authorization": f"Bot {bot_token}",
        "Content-Type": "application/json",
    }
    payload: dict[str, Any] = {"content": content}
    if allowed_mentions is not None:
        payload["allowed_mentions"] = allowed_mentions

    async with httpx.AsyncClient(timeout=timeout_seconds) as client:
        response = await client.post(f"{DISCORD_API_BASE}/channels/{channel_id}/messages", headers=headers, json=payload)
        response.raise_for_status()

