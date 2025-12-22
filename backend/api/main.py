import logging
from contextlib import asynccontextmanager
from datetime import datetime, timedelta, timezone
from typing import Any, Dict, Optional
from urllib.parse import urlencode

import stripe
import uvicorn
from fastapi import FastAPI, HTTPException, Request
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel
from starlette.concurrency import run_in_threadpool
from starlette.responses import RedirectResponse

from backend.api.auth import (
    OAUTH_STATE_COOKIE_NAME,
    SESSION_COOKIE_NAME,
    make_oauth_state_cookie,
    make_session_cookie,
    new_oauth_state,
    parse_oauth_state_cookie,
    parse_session_cookie,
)
from backend.api.discord_oauth import build_authorize_url, exchange_code_for_token, fetch_me, fetch_my_guilds
from backend.common.config import load_app_config
from backend.common.logging import configure_logging
from backend.common.models import GuildSettings
from backend.database.db import (
    Database,
    connect_guild_to_user,
    create_pool,
    create_session,
    delete_session,
    fetch_user_guild_access,
    fetch_subscription,
    fetch_guild_plan,
    fetch_guild_settings,
    fetch_message_counts,
    fetch_moderation_logs,
    fetch_sentiment_daily,
    fetch_sentiment_report,
    fetch_session_user_id,
    fetch_subscription_plan,
    fetch_user_id_by_stripe_customer_id,
    fetch_user_connected_guild_ids,
    fetch_user_guilds,
    fetch_user_stripe_customer_id,
    init_db,
    set_user_stripe_customer_id,
    set_subscription_plan,
    upsert_stripe_subscription,
    upsert_guild_settings,
    upsert_user,
    upsert_user_guilds,
    user_can_access_guild,
    user_has_manage_guild_permissions,
 )


class WebhookPayload(BaseModel):
    id: str
    type: str
    data: Dict[str, Any] = {}


class DevSetPlanBody(BaseModel):
    plan: str


class BillingCheckoutBody(BaseModel):
    plan: str = "pro"


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

frontend_origin = (config.frontend_base_url or "http://localhost:3000").rstrip("/")
app.add_middleware(
    CORSMiddleware,
    allow_origins=[frontend_origin],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)


def _require_session_secret() -> str:
    if not config.session_secret:
        raise HTTPException(status_code=500, detail="SESSION_SECRET is not configured")
    return config.session_secret


def _require_stripe_secret_key() -> str:
    if not config.stripe_secret_key:
        raise HTTPException(status_code=404, detail="stripe billing not enabled")
    return config.stripe_secret_key


def _require_stripe_webhook_secret() -> str:
    if not config.stripe_webhook_secret:
        raise HTTPException(status_code=404, detail="stripe webhooks not enabled")
    return config.stripe_webhook_secret


def _get_stripe_price_id(plan: str) -> str:
    """Get the Stripe price ID for the given plan tier."""
    if plan == "plus":
        if not config.stripe_plus_price_id:
            raise HTTPException(status_code=500, detail="STRIPE_PLUS_PRICE_ID is not configured")
        return config.stripe_plus_price_id
    elif plan == "premium":
        if not config.stripe_premium_price_id:
            raise HTTPException(status_code=500, detail="STRIPE_PREMIUM_PRICE_ID is not configured")
        return config.stripe_premium_price_id
    else:
        raise HTTPException(status_code=400, detail=f"Invalid plan: {plan}")


def _frontend_base_url() -> str:
    return (config.frontend_base_url or "http://localhost:3000").rstrip("/")


async def _require_user_id(request: Request) -> str:
    secret = _require_session_secret()

    token: Optional[str] = None
    authz = request.headers.get("authorization")
    if authz and authz.lower().startswith("bearer "):
        token = authz.split(" ", 1)[1].strip()
    if not token:
        token = request.cookies.get(SESSION_COOKIE_NAME)
    if not token:
        raise HTTPException(status_code=401, detail="not authenticated")

    parsed = parse_session_cookie(secret, token)
    if not parsed:
        raise HTTPException(status_code=401, detail="invalid session")

    db: Database = request.app.state.db
    user_id = await fetch_session_user_id(db, parsed.session_id)
    if not user_id:
        raise HTTPException(status_code=401, detail="session expired")
    return user_id


async def _require_guild_access(request: Request, guild_id: str) -> str:
    user_id = await _require_user_id(request)
    db: Database = request.app.state.db
    if not await user_can_access_guild(db, user_id, guild_id):
        raise HTTPException(status_code=403, detail="no access to guild")
    return user_id


async def _require_guild_manage(request: Request, guild_id: str) -> str:
    user_id = await _require_guild_access(request, guild_id)
    db: Database = request.app.state.db
    access = await fetch_user_guild_access(db, user_id, guild_id)
    if not access or not user_has_manage_guild_permissions(access["is_owner"], access["permissions"]):
        raise HTTPException(status_code=403, detail="requires Manage Guild permissions")
    return user_id


@app.get("/health")
async def health() -> Dict[str, str]:
    return {"status": "ok"}


@app.get("/auth/discord/login")
async def discord_login(request: Request, redirect: str = "/dashboard") -> RedirectResponse:
    if not (config.discord_client_id and config.discord_oauth_redirect_uri):
        raise HTTPException(status_code=500, detail="Discord OAuth is not configured")

    if not redirect.startswith("/") or redirect.startswith("//"):
        redirect = "/dashboard"

    secret = _require_session_secret()
    state = new_oauth_state()
    oauth_cookie = make_oauth_state_cookie(secret, state=state, redirect_path=redirect)
    url = build_authorize_url(config.discord_client_id, config.discord_oauth_redirect_uri, state=state)

    response = RedirectResponse(url=url, status_code=302)
    response.set_cookie(
        key=OAUTH_STATE_COOKIE_NAME,
        value=oauth_cookie,
        httponly=True,
        secure=False,
        samesite="lax",
        max_age=600,
    )
    return response


@app.get("/auth/discord/callback")
async def discord_callback(request: Request, code: str, state: str) -> RedirectResponse:
    if not (config.discord_client_id and config.discord_client_secret and config.discord_oauth_redirect_uri):
        raise HTTPException(status_code=500, detail="Discord OAuth is not configured")

    secret = _require_session_secret()
    state_cookie = request.cookies.get(OAUTH_STATE_COOKIE_NAME)
    if not state_cookie:
        raise HTTPException(status_code=400, detail="missing oauth state cookie")

    parsed = parse_oauth_state_cookie(secret, state_cookie)
    if not parsed:
        raise HTTPException(status_code=400, detail="invalid oauth state cookie")
    expected_state, redirect_path = parsed
    if state != expected_state:
        raise HTTPException(status_code=400, detail="oauth state mismatch")

    token = await exchange_code_for_token(
        config.discord_client_id,
        config.discord_client_secret,
        config.discord_oauth_redirect_uri,
        code,
    )
    access_token = token.get("access_token")
    if not isinstance(access_token, str) or access_token.strip() == "":
        raise HTTPException(status_code=400, detail="missing access_token from discord")

    me = await fetch_me(access_token)
    guilds = await fetch_my_guilds(access_token)

    user_id = str(me.get("id"))
    username = me.get("global_name") or me.get("username") or "unknown"
    avatar = me.get("avatar")

    db: Database = request.app.state.db
    await upsert_user(db, user_id=user_id, username=str(username), avatar=str(avatar) if avatar else None)
    await upsert_user_guilds(db, user_id=user_id, guilds=guilds)

    plan = await fetch_subscription_plan(db, user_id)
    logging.info("Discord login user=%s plan=%s guilds=%s", user_id, plan, len(guilds))

    session_id, _expires_at = await create_session(db, user_id=user_id, ttl_days=7)
    session_token = make_session_cookie(secret, session_id=session_id, ttl_seconds=7 * 24 * 60 * 60)

    qs = urlencode({"token": session_token, "redirect": redirect_path})
    response = RedirectResponse(url=f"{frontend_origin}/auth/callback?{qs}", status_code=302)
    response.delete_cookie(OAUTH_STATE_COOKIE_NAME)
    return response


@app.post("/auth/logout")
async def logout(request: Request) -> Dict[str, bool]:
    secret = _require_session_secret()

    token: Optional[str] = None
    authz = request.headers.get("authorization")
    if authz and authz.lower().startswith("bearer "):
        token = authz.split(" ", 1)[1].strip()
    if not token:
        token = request.cookies.get(SESSION_COOKIE_NAME)

    if token:
        parsed = parse_session_cookie(secret, token)
        if parsed:
            db: Database = request.app.state.db
            await delete_session(db, parsed.session_id)
    return {"ok": True}


@app.get("/me")
async def me(request: Request) -> Dict[str, Any]:
    user_id = await _require_user_id(request)
    db: Database = request.app.state.db
    plan = await fetch_subscription_plan(db, user_id)
    guilds = await fetch_user_guilds(db, user_id)
    connected = await fetch_user_connected_guild_ids(db, user_id)
    return {
        "user_id": user_id,
        "plan": plan,
        "guilds": [{**g, "connected": g["guild_id"] in connected} for g in guilds],
    }


@app.get("/billing/subscription")
async def billing_subscription(request: Request) -> Dict[str, Any]:
    user_id = await _require_user_id(request)
    db: Database = request.app.state.db

    sub = await fetch_subscription(db, user_id)
    customer_id = await fetch_user_stripe_customer_id(db, user_id)
    return {
        "user_id": user_id,
        "stripe_enabled": bool(config.stripe_secret_key),
        "stripe_customer_id": customer_id,
        "plan": sub["plan"],
        "status": sub["status"],
        "cancel_at_period_end": bool(sub["cancel_at_period_end"]),
        "current_period_end": sub["current_period_end"].isoformat() if sub["current_period_end"] else None,
    }


@app.post("/billing/checkout")
async def billing_checkout(request: Request, body: BillingCheckoutBody) -> Dict[str, Any]:
    user_id = await _require_user_id(request)
    secret_key = _require_stripe_secret_key()

    plan = body.plan.strip().lower()
    if plan not in {"plus", "premium"}:
        raise HTTPException(status_code=400, detail="plan must be 'plus' or 'premium'")

    price_id = _get_stripe_price_id(plan)

    stripe.api_key = secret_key

    db: Database = request.app.state.db
    customer_id = await fetch_user_stripe_customer_id(db, user_id)
    if not customer_id:
        customer = await run_in_threadpool(stripe.Customer.create, metadata={"user_id": user_id})
        customer_id = str(customer["id"])
        await set_user_stripe_customer_id(db, user_id, customer_id)

    base = _frontend_base_url()
    session = await run_in_threadpool(
        stripe.checkout.Session.create,
        mode="subscription",
        customer=customer_id,
        line_items=[{"price": price_id, "quantity": 1}],
        allow_promotion_codes=True,
        client_reference_id=user_id,
        success_url=f"{base}/dashboard/billing?checkout=success",
        cancel_url=f"{base}/dashboard/billing?checkout=cancel",
        subscription_data={"metadata": {"user_id": user_id, "plan": plan}},
        metadata={"user_id": user_id, "plan": plan},
    )

    url = session.get("url")
    if not url:
        raise HTTPException(status_code=500, detail="stripe session missing url")
    return {"url": url}


@app.post("/billing/portal")
async def billing_portal(request: Request) -> Dict[str, Any]:
    user_id = await _require_user_id(request)
    secret_key = _require_stripe_secret_key()
    stripe.api_key = secret_key

    db: Database = request.app.state.db
    customer_id = await fetch_user_stripe_customer_id(db, user_id)
    if not customer_id:
        raise HTTPException(status_code=400, detail="no stripe customer for user")

    base = _frontend_base_url()
    session = await run_in_threadpool(
        stripe.billing_portal.Session.create,
        customer=customer_id,
        return_url=f"{base}/dashboard/billing",
    )
    url = session.get("url")
    if not url:
        raise HTTPException(status_code=500, detail="stripe portal session missing url")
    return {"url": url}


@app.post("/subscriptions/dev/set-plan")
async def dev_set_plan(request: Request, body: DevSetPlanBody) -> Dict[str, Any]:
    if not config.dev_admin_token:
        raise HTTPException(status_code=404, detail="not enabled")

    provided = request.headers.get("x-dev-admin-token")
    if provided != config.dev_admin_token:
        raise HTTPException(status_code=403, detail="forbidden")

    user_id = await _require_user_id(request)
    db: Database = request.app.state.db
    plan = body.plan.strip().lower()
    if plan not in {"free", "plus", "premium"}:
        raise HTTPException(status_code=400, detail="plan must be 'free', 'plus', or 'premium'")

    await set_subscription_plan(db, user_id, plan=plan)
    return {"ok": True, "user_id": user_id, "plan": plan}


@app.get("/guilds/{guild_id}/settings", response_model=GuildSettings)
async def get_settings(guild_id: str, request: Request) -> GuildSettings:
    await _require_guild_access(request, guild_id)
    db: Database = request.app.state.db
    return await fetch_guild_settings(db, guild_id)


@app.patch("/guilds/{guild_id}/settings", response_model=GuildSettings)
async def update_settings(guild_id: str, body: GuildSettings, request: Request) -> GuildSettings:
    await _require_guild_manage(request, guild_id)
    if guild_id != body.guild_id:
        raise HTTPException(status_code=400, detail="guild_id mismatch")

    db: Database = request.app.state.db
    return await upsert_guild_settings(db, body)


@app.post("/guilds/{guild_id}/connect")
async def connect_guild(guild_id: str, request: Request) -> Dict[str, bool]:
    user_id = await _require_guild_manage(request, guild_id)
    db: Database = request.app.state.db
    await connect_guild_to_user(db, guild_id=guild_id, billing_user_id=user_id)
    await fetch_guild_settings(db, guild_id)
    return {"ok": True}


@app.get("/guilds/{guild_id}/dashboard/overview")
async def dashboard_overview(guild_id: str, request: Request) -> Dict[str, Any]:
    await _require_guild_access(request, guild_id)
    db: Database = request.app.state.db
    plan = await fetch_guild_plan(db, guild_id)
    return {
        "guild_id": guild_id,
        "plan": plan,
        "features": {
            "moderation_logs": plan == "pro",
            "sentiment_reports": plan == "pro",
            "event_recommendations": plan == "pro",
            "analytics_extended": plan == "pro",
        },
    }


@app.get("/guilds/{guild_id}/analytics/message-counts")
async def analytics_message_counts(guild_id: str, request: Request, hours: int = 24) -> Dict[str, Any]:
    await _require_guild_access(request, guild_id)
    db: Database = request.app.state.db
    settings = await fetch_guild_settings(db, guild_id)
    if not settings.analytics_enabled:
        raise HTTPException(status_code=403, detail="analytics disabled for guild")
    plan = await fetch_guild_plan(db, guild_id)

    hours = max(1, min(hours, 24 * 14 if plan == "pro" else 24 * 7))
    to_ts = datetime.now(timezone.utc)
    from_ts = to_ts - timedelta(hours=hours)
    points = await fetch_message_counts(db, guild_id, from_ts, to_ts, limit=5000 if plan == "pro" else 1500)
    return {"guild_id": guild_id, "from": from_ts.isoformat(), "to": to_ts.isoformat(), "points": points}


@app.get("/guilds/{guild_id}/sentiment/daily")
async def sentiment_daily(guild_id: str, request: Request, days: int = 30) -> Dict[str, Any]:
    await _require_guild_access(request, guild_id)
    db: Database = request.app.state.db
    settings = await fetch_guild_settings(db, guild_id)
    if not settings.sentiment_enabled:
        raise HTTPException(status_code=403, detail="sentiment disabled for guild")
    plan = await fetch_guild_plan(db, guild_id)

    days = max(1, min(days, 365 if plan == "pro" else 30))
    to_day = datetime.now(timezone.utc)
    from_day = to_day - timedelta(days=days)
    points = await fetch_sentiment_daily(db, guild_id, from_day, to_day)
    return {"guild_id": guild_id, "from": from_day.date().isoformat(), "to": to_day.date().isoformat(), "points": points}


@app.get("/guilds/{guild_id}/sentiment/report")
async def sentiment_report(guild_id: str, request: Request, day: Optional[str] = None) -> Dict[str, Any]:
    await _require_guild_access(request, guild_id)
    db: Database = request.app.state.db
    settings = await fetch_guild_settings(db, guild_id)
    if not settings.sentiment_enabled:
        raise HTTPException(status_code=403, detail="sentiment disabled for guild")
    plan = await fetch_guild_plan(db, guild_id)
    if plan != "pro":
        raise HTTPException(status_code=402, detail="sentiment report requires pro plan")

    target = datetime.now(timezone.utc)
    if day:
        target = datetime.fromisoformat(day).replace(tzinfo=timezone.utc)
    report = await fetch_sentiment_report(db, guild_id, target)
    return {"guild_id": guild_id, "day": target.date().isoformat(), "report": report}


@app.get("/guilds/{guild_id}/moderation/logs")
async def moderation_logs(guild_id: str, request: Request, limit: int = 200) -> Dict[str, Any]:
    await _require_guild_access(request, guild_id)
    db: Database = request.app.state.db
    settings = await fetch_guild_settings(db, guild_id)
    if not settings.moderation_enabled:
        raise HTTPException(status_code=403, detail="moderation disabled for guild")
    plan = await fetch_guild_plan(db, guild_id)
    if plan != "pro":
        raise HTTPException(status_code=402, detail="moderation log history requires pro plan")

    limit = max(1, min(limit, 500))
    rows = await fetch_moderation_logs(db, guild_id, limit=limit)
    return {"guild_id": guild_id, "items": rows}


@app.post("/webhooks/discord")
async def webhook_discord(payload: WebhookPayload) -> Dict[str, Any]:
    logging.info("Received Discord webhook %s of type %s", payload.id, payload.type)
    return {"received": True}


@app.post("/subscriptions/stripe")
@app.post("/webhooks/stripe")
async def webhook_stripe(request: Request) -> Dict[str, Any]:
    secret_key = _require_stripe_secret_key()
    webhook_secret = _require_stripe_webhook_secret()

    stripe.api_key = secret_key

    payload = await request.body()
    sig_header = request.headers.get("stripe-signature")
    if not sig_header:
        raise HTTPException(status_code=400, detail="missing stripe-signature header")

    try:
        event = stripe.Webhook.construct_event(payload=payload, sig_header=sig_header, secret=webhook_secret)
    except stripe.error.SignatureVerificationError:
        raise HTTPException(status_code=400, detail="invalid stripe signature") from None
    except ValueError:
        raise HTTPException(status_code=400, detail="invalid stripe payload") from None

    event_type = event.get("type")
    data_object = (event.get("data") or {}).get("object") or {}

    db: Database = request.app.state.db

    async def resolve_user_id_from_customer(customer_id: Optional[str]) -> Optional[str]:
        if not customer_id:
            return None
        return await fetch_user_id_by_stripe_customer_id(db, customer_id)

    async def apply_subscription_update(subscription: Dict[str, Any], user_id: Optional[str]) -> None:
        customer_id = subscription.get("customer")
        if not user_id:
            user_id = str(subscription.get("metadata", {}).get("user_id") or "") or None
        if not user_id:
            user_id = await resolve_user_id_from_customer(str(customer_id) if customer_id else None)
        if not user_id:
            logging.warning("Stripe webhook: cannot map subscription to user (customer=%s)", customer_id)
            return

        if customer_id:
            existing = await fetch_user_stripe_customer_id(db, user_id)
            if not existing:
                await set_user_stripe_customer_id(db, user_id, str(customer_id))

        status = str(subscription.get("status") or "unknown")
        cancel_at_period_end = bool(subscription.get("cancel_at_period_end") or False)

        current_period_end = None
        cpe = subscription.get("current_period_end")
        if isinstance(cpe, (int, float)):
            current_period_end = datetime.fromtimestamp(cpe, tz=timezone.utc)

        price_id = None
        items = (subscription.get("items") or {}).get("data") or []
        if items and isinstance(items, list):
            price = (items[0] or {}).get("price") if isinstance(items[0], dict) else None
            if isinstance(price, dict):
                price_id = price.get("id")

        plan = "free"
        if status in {"active", "trialing"}:
            if price_id == config.stripe_plus_price_id:
                plan = "plus"
            elif price_id == config.stripe_premium_price_id:
                plan = "premium"

        await upsert_stripe_subscription(
            db,
            user_id,
            plan=plan,
            status=status,
            stripe_subscription_id=str(subscription.get("id") or "") or None,
            stripe_price_id=str(price_id) if price_id else None,
            current_period_end=current_period_end,
            cancel_at_period_end=cancel_at_period_end,
        )

    if event_type == "checkout.session.completed":
        if data_object.get("mode") == "subscription":
            user_id = str(data_object.get("client_reference_id") or "") or None
            customer_id = str(data_object.get("customer") or "") or None
            subscription_id = str(data_object.get("subscription") or "") or None

            if user_id and customer_id:
                existing = await fetch_user_stripe_customer_id(db, user_id)
                if not existing:
                    await set_user_stripe_customer_id(db, user_id, customer_id)

            if subscription_id:
                subscription = await run_in_threadpool(
                    stripe.Subscription.retrieve,
                    subscription_id,
                    expand=["items.data.price"],
                )
                await apply_subscription_update(dict(subscription), user_id)

    if event_type in {"customer.subscription.created", "customer.subscription.updated", "customer.subscription.deleted"}:
        subscription = dict(data_object)
        if event_type != "customer.subscription.deleted" and subscription.get("id"):
            subscription = dict(
                await run_in_threadpool(
                    stripe.Subscription.retrieve,
                    str(subscription["id"]),
                    expand=["items.data.price"],
                )
            )
        await apply_subscription_update(subscription, None)

    return {"received": True}


def run(host: str = "0.0.0.0", port: int = 8000) -> None:
    uvicorn.run("backend.api.main:app", host=host, port=port, reload=False, log_level=config.log_level.lower())


if __name__ == "__main__":
    run()
