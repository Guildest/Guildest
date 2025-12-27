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


async def fetch_user(*, bot_token: str, user_id: str, timeout_seconds: float = 15) -> dict[str, Any]:
    headers = {"Authorization": f"Bot {bot_token}"}
    async with httpx.AsyncClient(timeout=timeout_seconds) as client:
        response = await client.get(f"{DISCORD_API_BASE}/users/{user_id}", headers=headers)
        response.raise_for_status()
        return response.json()


async def ban_member(
    *,
    bot_token: str,
    guild_id: str,
    user_id: str,
    reason: Optional[str] = None,
    delete_message_days: Optional[int] = None,
    timeout_seconds: float = 15,
) -> None:
    headers = {"Authorization": f"Bot {bot_token}"}
    if reason:
        headers["X-Audit-Log-Reason"] = reason
    payload: dict[str, Any] = {}
    if delete_message_days is not None:
        payload["delete_message_days"] = delete_message_days
    async with httpx.AsyncClient(timeout=timeout_seconds) as client:
        response = await client.put(
            f"{DISCORD_API_BASE}/guilds/{guild_id}/bans/{user_id}",
            headers=headers,
            json=payload if payload else None,
        )
        response.raise_for_status()


async def unban_member(
    *,
    bot_token: str,
    guild_id: str,
    user_id: str,
    reason: Optional[str] = None,
    timeout_seconds: float = 15,
) -> None:
    headers = {"Authorization": f"Bot {bot_token}"}
    if reason:
        headers["X-Audit-Log-Reason"] = reason
    async with httpx.AsyncClient(timeout=timeout_seconds) as client:
        response = await client.delete(
            f"{DISCORD_API_BASE}/guilds/{guild_id}/bans/{user_id}",
            headers=headers,
        )
        response.raise_for_status()


async def timeout_member(
    *,
    bot_token: str,
    guild_id: str,
    user_id: str,
    communication_disabled_until: str,
    reason: Optional[str] = None,
    timeout_seconds: float = 15,
) -> None:
    headers = {
        "Authorization": f"Bot {bot_token}",
        "Content-Type": "application/json",
    }
    if reason:
        headers["X-Audit-Log-Reason"] = reason
    payload = {"communication_disabled_until": communication_disabled_until}
    async with httpx.AsyncClient(timeout=timeout_seconds) as client:
        response = await client.patch(
            f"{DISCORD_API_BASE}/guilds/{guild_id}/members/{user_id}",
            headers=headers,
            json=payload,
        )
        response.raise_for_status()


async def create_dm_channel(
    *,
    bot_token: str,
    user_id: str,
    timeout_seconds: float = 15,
) -> str:
    headers = {
        "Authorization": f"Bot {bot_token}",
        "Content-Type": "application/json",
    }
    payload = {"recipient_id": user_id}
    async with httpx.AsyncClient(timeout=timeout_seconds) as client:
        response = await client.post(f"{DISCORD_API_BASE}/users/@me/channels", headers=headers, json=payload)
        response.raise_for_status()
        data = response.json()
        return str(data["id"])


async def send_channel_message(
    *,
    bot_token: str,
    channel_id: str,
    content: str,
    allowed_mentions: Optional[dict[str, Any]] = None,
    embeds: Optional[list[dict[str, Any]]] = None,
    components: Optional[list[dict[str, Any]]] = None,
    timeout_seconds: float = 15,
) -> None:
    headers = {
        "Authorization": f"Bot {bot_token}",
        "Content-Type": "application/json",
    }
    payload: dict[str, Any] = {"content": content}
    if allowed_mentions is not None:
        payload["allowed_mentions"] = allowed_mentions
    if embeds is not None:
        payload["embeds"] = embeds
    if components is not None:
        payload["components"] = components

    async with httpx.AsyncClient(timeout=timeout_seconds) as client:
        response = await client.post(f"{DISCORD_API_BASE}/channels/{channel_id}/messages", headers=headers, json=payload)
        response.raise_for_status()


async def edit_interaction_response(
    *,
    application_id: str,
    interaction_token: str,
    content: Optional[str] = None,
    allowed_mentions: Optional[dict[str, Any]] = None,
    embeds: Optional[list[dict[str, Any]]] = None,
    components: Optional[list[dict[str, Any]]] = None,
    timeout_seconds: float = 15,
) -> None:
    headers = {"Content-Type": "application/json"}
    payload: dict[str, Any] = {}
    if content is not None:
        payload["content"] = content
    if allowed_mentions is not None:
        payload["allowed_mentions"] = allowed_mentions
    if embeds is not None:
        payload["embeds"] = embeds
    if components is not None:
        payload["components"] = components

    async with httpx.AsyncClient(timeout=timeout_seconds) as client:
        response = await client.patch(
            f"{DISCORD_API_BASE}/webhooks/{application_id}/{interaction_token}/messages/@original",
            headers=headers,
            json=payload,
        )
        response.raise_for_status()


async def send_interaction_followup(
    *,
    application_id: str,
    interaction_token: str,
    content: Optional[str] = None,
    allowed_mentions: Optional[dict[str, Any]] = None,
    embeds: Optional[list[dict[str, Any]]] = None,
    components: Optional[list[dict[str, Any]]] = None,
    ephemeral: bool = False,
    timeout_seconds: float = 15,
) -> None:
    headers = {"Content-Type": "application/json"}
    payload: dict[str, Any] = {}
    if content is not None:
        payload["content"] = content
    if allowed_mentions is not None:
        payload["allowed_mentions"] = allowed_mentions
    if embeds is not None:
        payload["embeds"] = embeds
    if components is not None:
        payload["components"] = components
    if ephemeral:
        payload["flags"] = 1 << 6

    async with httpx.AsyncClient(timeout=timeout_seconds) as client:
        response = await client.post(
            f"{DISCORD_API_BASE}/webhooks/{application_id}/{interaction_token}",
            headers=headers,
            json=payload,
        )
        response.raise_for_status()
