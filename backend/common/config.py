import os
from dataclasses import dataclass, field
from typing import Optional


def _env_int(name: str, default: int) -> int:
    raw = os.getenv(name)
    if raw is None or raw.strip() == "":
        return default
    try:
        return int(raw)
    except ValueError:
        raise ValueError(f"{name} must be an integer, got '{raw}'") from None


@dataclass
class RedisConfig:
    url: str = "redis://localhost:6379/0"


@dataclass
class QueueConfig:
    stream: str = "guildest:events"
    max_length: int = 5000
    group_name: str = "guildest-workers"
    consumer_name: str = "worker-1"


@dataclass
class AppConfig:
    """Shared application configuration loaded from environment."""

    log_level: str = "INFO"
    api_base: Optional[str] = None
    database_url: Optional[str] = None
    discord_token: Optional[str] = None
    discord_client_id: Optional[str] = None
    discord_client_secret: Optional[str] = None
    discord_oauth_redirect_uri: Optional[str] = None
    frontend_base_url: Optional[str] = None
    session_secret: Optional[str] = None
    openrouter_api_key: Optional[str] = None
    openrouter_model: str = "deepseek/deepseek-v3.2"
    dev_admin_token: Optional[str] = None
    stripe_secret_key: Optional[str] = None
    stripe_webhook_secret: Optional[str] = None
    stripe_plus_price_id: Optional[str] = None
    stripe_premium_price_id: Optional[str] = None
    redis: RedisConfig = field(default_factory=RedisConfig)
    queue: QueueConfig = field(default_factory=QueueConfig)


def load_app_config() -> AppConfig:
    """Load configuration from environment variables."""

    redis_url = os.getenv("REDIS_URL", "redis://localhost:6379/0").strip()
    queue_stream = os.getenv("QUEUE_STREAM", "guildest:events").strip()
    queue_max = _env_int("QUEUE_MAX_LENGTH", 5000)

    group_name = os.getenv("QUEUE_GROUP", "guildest-workers").strip()
    consumer_name = os.getenv("QUEUE_CONSUMER", "worker-1").strip()

    return AppConfig(
        log_level=os.getenv("LOG_LEVEL", "INFO").strip().upper(),
        api_base=os.getenv("API_BASE"),
        database_url=os.getenv("DATABASE_URL"),
        discord_token=os.getenv("DISCORD_TOKEN"),
        discord_client_id=os.getenv("DISCORD_CLIENT_ID"),
        discord_client_secret=os.getenv("DISCORD_CLIENT_SECRET"),
        discord_oauth_redirect_uri=os.getenv("DISCORD_OAUTH_REDIRECT_URI"),
        frontend_base_url=os.getenv("FRONTEND_BASE_URL"),
        session_secret=os.getenv("SESSION_SECRET"),
        openrouter_api_key=os.getenv("OPENROUTER_API_KEY"),
        openrouter_model=os.getenv("OPENROUTER_MODEL", "deepseek/deepseek-v3.2").strip(),
        dev_admin_token=os.getenv("DEV_ADMIN_TOKEN"),
        stripe_secret_key=os.getenv("STRIPE_SECRET_KEY"),
        stripe_webhook_secret=os.getenv("STRIPE_WEBHOOK_SECRET"),
        stripe_plus_price_id=os.getenv("STRIPE_PLUS_PRICE_ID"),
        stripe_premium_price_id=os.getenv("STRIPE_PREMIUM_PRICE_ID"),
        redis=RedisConfig(url=redis_url),
        queue=QueueConfig(
            stream=queue_stream,
            max_length=queue_max,
            group_name=group_name,
            consumer_name=consumer_name,
        ),
    )
