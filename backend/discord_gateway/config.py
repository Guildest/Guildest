import os
from dataclasses import dataclass


@dataclass
class GatewayConfig:
    """Configuration for the Discord gateway service."""

    discord_token: str
    redis_url: str = "redis://localhost:6379/0"
    queue_stream: str = "guildest:events"
    queue_max_length: int = 5000
    log_level: str = "INFO"

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
            redis_url=os.getenv("REDIS_URL", "redis://localhost:6379/0").strip(),
            queue_stream=os.getenv("QUEUE_STREAM", "guildest:events").strip(),
            queue_max_length=_int_env("QUEUE_MAX_LENGTH", 5000),
            log_level=os.getenv("LOG_LEVEL", "INFO").strip().upper(),
        )
