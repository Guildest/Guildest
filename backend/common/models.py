from typing import Any, Dict

from pydantic import BaseModel, Field


class QueueMessage(BaseModel):
    """Normalized queue message schema."""

    event: str
    message_id: str
    guild_id: str
    channel_id: str
    author_id: str
    content: str
    timestamp: str
    metadata: Dict[str, Any] = Field(default_factory=dict)


class GuildSettings(BaseModel):
    guild_id: str
    prefix: str = "!"
    moderation_enabled: bool = True
    analytics_enabled: bool = True
    sentiment_enabled: bool = True
