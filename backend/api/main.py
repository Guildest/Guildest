import logging
from contextlib import asynccontextmanager
from typing import Any, Dict

import uvicorn
from fastapi import FastAPI, HTTPException, Request
from pydantic import BaseModel

from backend.common.config import load_app_config
from backend.common.logging import configure_logging
from backend.common.models import GuildSettings
from backend.database.db import Database, create_pool, fetch_guild_settings, init_db, upsert_guild_settings


class WebhookPayload(BaseModel):
    id: str
    type: str
    data: Dict[str, Any] = {}


config = load_app_config()
configure_logging(config.log_level)


@asynccontextmanager
async def lifespan(_: FastAPI):
    if not config.database_url:
        raise RuntimeError("DATABASE_URL is required for the API service")

    db = await create_pool(config.database_url)
    await init_db(db)
    app.state.db = db
    logging.info("API started with database connection")
    try:
        yield
    finally:
        await db.close()
        logging.info("API database connection closed")


app = FastAPI(title="Guildest API", version="0.1.0", lifespan=lifespan)


@app.get("/health")
async def health() -> Dict[str, str]:
    return {"status": "ok"}


@app.get("/guilds/{guild_id}/settings", response_model=GuildSettings)
async def get_settings(guild_id: str, request: Request) -> GuildSettings:
    db: Database = request.app.state.db
    return await fetch_guild_settings(db, guild_id)


@app.patch("/guilds/{guild_id}/settings", response_model=GuildSettings)
async def update_settings(guild_id: str, body: GuildSettings, request: Request) -> GuildSettings:
    if guild_id != body.guild_id:
        raise HTTPException(status_code=400, detail="guild_id mismatch")

    db: Database = request.app.state.db
    return await upsert_guild_settings(db, body)


@app.post("/webhooks/discord")
async def webhook_discord(payload: WebhookPayload) -> Dict[str, Any]:
    logging.info("Received Discord webhook %s of type %s", payload.id, payload.type)
    return {"received": True}


@app.post("/subscriptions/stripe")
async def webhook_stripe(payload: WebhookPayload) -> Dict[str, Any]:
    logging.info("Received Stripe event %s of type %s", payload.id, payload.type)
    return {"received": True}


def run(host: str = "0.0.0.0", port: int = 8000) -> None:
    uvicorn.run("backend.api.main:app", host=host, port=port, reload=False, log_level=config.log_level.lower())


if __name__ == "__main__":
    run()
