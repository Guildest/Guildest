import os
from dataclasses import dataclass
from typing import Optional


@dataclass
class GatewayConfig:
    """Configuration for the Discord gateway service."""

    discord_token: str
    discord_application_id: Optional[str] = None
    commands_guild_id: Optional[str] = None
    redis_url: str = "redis://localhost:6379/0"
    queue_stream: str = "guildest:events"
    queue_max_length: int = 5000
    log_level: str = "INFO"
    database_url: Optional[str] = None
    frontend_base_url: str = "http://localhost:3000"

    @classmethod
    def from_env(cls) -> "GatewayConfig":
        """Load configuration from environment variables."""

        token = os.getenv("DISCORD_TOKEN")
        if not token:
            raise ValueError("DISCORD_TOKEN is required for the Discord gateway")

        def _int_env(name: str, default: int) -> int:
            raw = os.getenv(name)
            if raw is None or raw.strip() == "":
                return default
            try:
                return int(raw)
            except ValueError:
                raise ValueError(f"{name} must be an integer, got '{raw}'") from None

        return cls(
            discord_token=token.strip(),
            discord_application_id=os.getenv("DISCORD_APPLICATION_ID"),
            commands_guild_id=os.getenv("DISCORD_COMMANDS_GUILD_ID"),
            redis_url=os.getenv("REDIS_URL", "redis://localhost:6379/0").strip(),
            queue_stream=os.getenv("QUEUE_STREAM", "guildest:events").strip(),
            queue_max_length=_int_env("QUEUE_MAX_LENGTH", 5000),
            log_level=os.getenv("LOG_LEVEL", "INFO").strip().upper(),
            database_url=os.getenv("DATABASE_URL"),
            frontend_base_url=os.getenv("FRONTEND_BASE_URL", "http://localhost:3000").strip(),
        )
