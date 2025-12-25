from __future__ import annotations

from typing import Any, Optional

import httpx


DISCORD_API_BASE = "https://discord.com/api/v10"


async def bot_in_guild(*, bot_token: str, guild_id: str, timeout_seconds: float = 15) -> bool:
    headers = {"Authorization": f"Bot {bot_token}"}
    async with httpx.AsyncClient(timeout=timeout_seconds) as client:
        response = await client.get(f"{DISCORD_API_BASE}/guilds/{guild_id}", headers=headers)
        if response.status_code in (403, 404):
            return False
        response.raise_for_status()
        return True


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
