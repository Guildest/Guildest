#!/usr/bin/env python3
import argparse
import json
import socket
import ssl
import uuid
from dataclasses import dataclass
from datetime import datetime, timedelta, timezone
from typing import Optional
from urllib.parse import unquote, urlparse


BACKFILL_STREAM = "jobs.backfill"


@dataclass
class RedisTarget:
    host: str
    port: int
    db: int
    password: Optional[str]
    use_tls: bool


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Enqueue a Guildest backfill job directly into Redis."
    )
    parser.add_argument("--guild-id", required=True, help="Guild ID to backfill.")
    parser.add_argument(
        "--redis-url",
        default="redis://127.0.0.1:6379",
        help="Redis connection URL. Default: redis://127.0.0.1:6379",
    )
    parser.add_argument(
        "--days",
        type=int,
        default=7,
        help="How many days to backfill. Default: 7",
    )
    parser.add_argument(
        "--requested-by-user-id",
        default=None,
        help="Optional Discord user ID to associate with the request.",
    )
    parser.add_argument(
        "--trigger-source",
        default="benchmark",
        help="Trigger source stored in the job payload. Default: benchmark",
    )
    parser.add_argument(
        "--count",
        type=int,
        default=1,
        help="How many jobs to enqueue. Default: 1",
    )
    parser.add_argument(
        "--job-spacing-seconds",
        type=int,
        default=0,
        help="Offset requested timestamps by N seconds between jobs. Default: 0",
    )
    return parser.parse_args()


def parse_redis_url(url: str) -> RedisTarget:
    parsed = urlparse(url)
    if parsed.scheme not in {"redis", "rediss"}:
        raise ValueError(f"unsupported redis URL scheme: {parsed.scheme}")
    host = parsed.hostname or "127.0.0.1"
    port = parsed.port or (6380 if parsed.scheme == "rediss" else 6379)
    db = int((parsed.path or "/0").lstrip("/") or "0")
    password = unquote(parsed.password) if parsed.password else None
    return RedisTarget(
        host=host,
        port=port,
        db=db,
        password=password,
        use_tls=parsed.scheme == "rediss",
    )


def encode_command(*parts: str) -> bytes:
    encoded = [f"*{len(parts)}\r\n".encode()]
    for part in parts:
        data = part.encode("utf-8")
        encoded.append(f"${len(data)}\r\n".encode())
        encoded.append(data + b"\r\n")
    return b"".join(encoded)


def read_line(conn: socket.socket) -> bytes:
    data = bytearray()
    while True:
        chunk = conn.recv(1)
        if not chunk:
            raise ConnectionError("redis connection closed while reading response")
        data.extend(chunk)
        if data.endswith(b"\r\n"):
            return bytes(data[:-2])


def read_response(conn: socket.socket):
    prefix = conn.recv(1)
    if not prefix:
        raise ConnectionError("redis connection closed before response prefix")
    if prefix == b"+":
        return read_line(conn).decode("utf-8")
    if prefix == b"-":
        message = read_line(conn).decode("utf-8")
        raise RuntimeError(f"redis error: {message}")
    if prefix == b":":
        return int(read_line(conn))
    if prefix == b"$":
        length = int(read_line(conn))
        if length == -1:
            return None
        data = bytearray()
        remaining = length + 2
        while remaining > 0:
            chunk = conn.recv(remaining)
            if not chunk:
                raise ConnectionError("redis connection closed while reading bulk response")
            data.extend(chunk)
            remaining -= len(chunk)
        return bytes(data[:-2]).decode("utf-8")
    if prefix == b"*":
        length = int(read_line(conn))
        if length == -1:
            return None
        return [read_response(conn) for _ in range(length)]
    raise RuntimeError(f"unsupported redis response prefix: {prefix!r}")


def send_command(conn: socket.socket, *parts: str):
    conn.sendall(encode_command(*parts))
    return read_response(conn)


def open_redis_connection(target: RedisTarget) -> socket.socket:
    raw = socket.create_connection((target.host, target.port), timeout=5)
    if target.use_tls:
        context = ssl.create_default_context()
        conn = context.wrap_socket(raw, server_hostname=target.host)
    else:
        conn = raw

    if target.password:
        send_command(conn, "AUTH", target.password)
    if target.db:
        send_command(conn, "SELECT", str(target.db))
    return conn


def isoformat_utc(value: datetime) -> str:
    return value.astimezone(timezone.utc).isoformat().replace("+00:00", "Z")


def build_job_payload(
    guild_id: str,
    days: int,
    requested_by_user_id: Optional[str],
    trigger_source: str,
    requested_at: datetime,
) -> str:
    end_at = requested_at
    start_at = end_at - timedelta(days=days)
    payload = {
        "job_id": str(uuid.uuid4()),
        "guild_id": guild_id,
        "requested_by_user_id": requested_by_user_id,
        "days_requested": days,
        "start_at": isoformat_utc(start_at),
        "end_at": isoformat_utc(end_at),
        "requested_at": isoformat_utc(requested_at),
        "trigger_source": trigger_source,
    }
    return json.dumps(payload, separators=(",", ":"))


def main() -> int:
    args = parse_args()
    if args.days <= 0:
        raise SystemExit("--days must be greater than zero")
    if args.count <= 0:
        raise SystemExit("--count must be greater than zero")
    if args.job_spacing_seconds < 0:
        raise SystemExit("--job-spacing-seconds must be zero or greater")

    target = parse_redis_url(args.redis_url)
    conn = open_redis_connection(target)
    try:
        enqueued = []
        now = datetime.now(timezone.utc)
        for index in range(args.count):
            requested_at = now + timedelta(seconds=index * args.job_spacing_seconds)
            payload = build_job_payload(
                guild_id=args.guild_id,
                days=args.days,
                requested_by_user_id=args.requested_by_user_id,
                trigger_source=args.trigger_source,
                requested_at=requested_at,
            )
            stream_id = send_command(conn, "XADD", BACKFILL_STREAM, "*", "payload", payload)
            enqueued.append(
                {
                    "guild_id": args.guild_id,
                    "stream": BACKFILL_STREAM,
                    "stream_id": stream_id,
                    "payload": json.loads(payload),
                }
            )

        print(json.dumps({"enqueued": enqueued}, indent=2))
        return 0
    finally:
        conn.close()


if __name__ == "__main__":
    raise SystemExit(main())
