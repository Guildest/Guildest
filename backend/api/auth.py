from __future__ import annotations

import base64
import hmac
import json
import secrets
import time
from dataclasses import dataclass
from hashlib import sha256
from typing import Any, Optional


def _b64url_encode(raw: bytes) -> str:
    return base64.urlsafe_b64encode(raw).rstrip(b"=").decode("ascii")


def _b64url_decode(raw: str) -> bytes:
    padding = "=" * (-len(raw) % 4)
    return base64.urlsafe_b64decode((raw + padding).encode("ascii"))


def _sign(secret: str, payload: str) -> str:
    digest = hmac.new(secret.encode("utf-8"), payload.encode("utf-8"), sha256).digest()
    return _b64url_encode(digest)


def make_signed_token(secret: str, data: dict[str, Any]) -> str:
    payload = _b64url_encode(json.dumps(data, separators=(",", ":"), ensure_ascii=True).encode("utf-8"))
    sig = _sign(secret, payload)
    return f"{payload}.{sig}"


def verify_signed_token(secret: str, token: str) -> Optional[dict[str, Any]]:
    try:
        payload_b64, sig = token.split(".", 1)
    except ValueError:
        return None

    expected = _sign(secret, payload_b64)
    if not hmac.compare_digest(expected, sig):
        return None

    try:
        data = json.loads(_b64url_decode(payload_b64).decode("utf-8"))
    except Exception:
        return None

    exp = data.get("exp")
    if exp is not None:
        try:
            exp_num = int(exp)
        except Exception:
            return None
        if int(time.time()) >= exp_num:
            return None

    return data


SESSION_COOKIE_NAME = "guildest_session"
OAUTH_STATE_COOKIE_NAME = "guildest_oauth_state"


def new_oauth_state() -> str:
    return secrets.token_urlsafe(32)


def make_oauth_state_cookie(secret: str, state: str, redirect_path: str, ttl_seconds: int = 600) -> str:
    now = int(time.time())
    return make_signed_token(
        secret,
        {
            "state": state,
            "redirect": redirect_path,
            "exp": now + ttl_seconds,
        },
    )


def parse_oauth_state_cookie(secret: str, token: str) -> Optional[tuple[str, str]]:
    data = verify_signed_token(secret, token)
    if not data:
        return None
    state = data.get("state")
    redirect_path = data.get("redirect")
    if not isinstance(state, str) or not isinstance(redirect_path, str):
        return None
    return state, redirect_path


@dataclass(frozen=True)
class SessionCookie:
    session_id: str


def make_session_cookie(secret: str, session_id: str, ttl_seconds: int) -> str:
    now = int(time.time())
    return make_signed_token(secret, {"sid": session_id, "exp": now + ttl_seconds})


def parse_session_cookie(secret: str, token: str) -> Optional[SessionCookie]:
    data = verify_signed_token(secret, token)
    if not data:
        return None
    sid = data.get("sid")
    if not isinstance(sid, str) or sid.strip() == "":
        return None
    return SessionCookie(session_id=sid)

