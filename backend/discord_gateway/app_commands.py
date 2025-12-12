from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Optional

import httpx


DISCORD_API_BASE = "https://discord.com/api/v10"


@dataclass(frozen=True)
class CommandRegistrationResult:
    scope: str
    count: int


def build_application_commands() -> list[dict[str, Any]]:
    # Plain REST payloads (Discord application command objects).
    return [
        {"name": "ping", "description": "Check if the bot is alive"},
        {"name": "help", "description": "Show available commands"},
        {"name": "dashboard", "description": "Get the web dashboard link"},
        {"name": "stats", "description": "Message stats for this server (DB-backed)"},
        {"name": "sentiment", "description": "Latest sentiment snapshot (DB-backed)"},
        {"name": "modlogs", "description": "Recent moderation events (Pro, DB-backed)"},
    ]


async def register_application_commands(
    *,
    bot_token: str,
    application_id: str,
    commands: list[dict[str, Any]],
    guild_id: Optional[str] = None,
) -> CommandRegistrationResult:
    headers = {
        "Authorization": f"Bot {bot_token}",
        "Content-Type": "application/json",
    }

    if guild_id:
        url = f"{DISCORD_API_BASE}/applications/{application_id}/guilds/{guild_id}/commands"
        scope = f"guild:{guild_id}"
    else:
        url = f"{DISCORD_API_BASE}/applications/{application_id}/commands"
        scope = "global"

    async with httpx.AsyncClient(timeout=20) as client:
        response = await client.put(url, headers=headers, json=commands)
        response.raise_for_status()
        data = response.json()

    return CommandRegistrationResult(scope=scope, count=len(data) if isinstance(data, list) else 0)

