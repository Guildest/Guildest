from typing import Any, Dict, List, Optional

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


class WarnPolicyItem(BaseModel):
    threshold: int = Field(default=3, ge=1)
    action: str = Field(default="timeout")
    duration_hours: Optional[int] = Field(default=24, ge=1)


class GuildSettings(BaseModel):
    guild_id: str
    prefix: str = "!"
    moderation_enabled: bool = True
    analytics_enabled: bool = True
    sentiment_enabled: bool = True
    warn_decay_days: int = Field(default=90, ge=0)
    warn_policy: List[WarnPolicyItem] = Field(default_factory=list)
