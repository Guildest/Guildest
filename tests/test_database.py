import os
import uuid

import pytest

from backend.common.models import GuildSettings
from backend.database.db import create_pool, fetch_guild_settings, init_db, upsert_guild_settings, wait_for_db

pytestmark = pytest.mark.asyncio


@pytest.fixture(scope="module")
async def db():
    database_url = os.getenv("DATABASE_URL", "postgresql://postgres:postgres@localhost:5432/postgres")
    database = await create_pool(database_url)
    await wait_for_db(database)
    await init_db(database)
    yield database
    await database.close()


async def test_guild_settings_roundtrip(db) -> None:
    guild_id = f"test-{uuid.uuid4()}"

    settings = await fetch_guild_settings(db, guild_id)
    assert settings.guild_id == guild_id
    assert settings.prefix == "!"
    assert settings.moderation_enabled is True

    updated = await upsert_guild_settings(
        db,
        GuildSettings(
            guild_id=guild_id,
            prefix="?",
            moderation_enabled=False,
            analytics_enabled=True,
            sentiment_enabled=False,
        ),
    )

    assert updated.prefix == "?"
    assert updated.moderation_enabled is False
    assert updated.analytics_enabled is True
    assert updated.sentiment_enabled is False
