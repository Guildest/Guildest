#!/usr/bin/env python3
import json
import os
import threading
import time
from dataclasses import dataclass
from datetime import datetime, timedelta, timezone
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import Optional
from urllib.parse import parse_qs, urlparse


HOST = os.environ.get("MOCK_DISCORD_HOST", "127.0.0.1")
PORT = int(os.environ.get("MOCK_DISCORD_PORT", "9191"))
GUILD_ID = os.environ.get("MOCK_GUILD_ID", "bench-guild")
GUILD_IDS = os.environ.get("MOCK_GUILD_IDS", "")
GUILD_COUNT = int(os.environ.get("MOCK_GUILD_COUNT", "1"))
CHANNELS_PER_GUILD = int(os.environ.get("MOCK_CHANNEL_COUNT", "6"))
MESSAGES_PER_CHANNEL = int(os.environ.get("MOCK_MESSAGES_PER_CHANNEL", "1200"))
MESSAGE_CONTENT_BYTES = int(os.environ.get("MOCK_MESSAGE_CONTENT_BYTES", "4096"))
ATTACHMENTS_PER_MESSAGE = int(os.environ.get("MOCK_ATTACHMENTS_PER_MESSAGE", "2"))
RESPONSE_DELAY_MS = int(os.environ.get("MOCK_RESPONSE_DELAY_MS", "10"))
AUTHOR_COUNT = int(os.environ.get("MOCK_AUTHOR_COUNT", "32"))
REPLY_EVERY = int(os.environ.get("MOCK_REPLY_EVERY", "0"))
BOT_EVERY = int(os.environ.get("MOCK_BOT_EVERY", "0"))
THREAD_CHANNEL_EVERY = int(os.environ.get("MOCK_THREAD_CHANNEL_EVERY", "0"))
RATE_LIMIT_EVERY_N_MESSAGES_REQUESTS = int(
    os.environ.get(
        "MOCK_RATE_LIMIT_EVERY_N_MESSAGES_REQUESTS",
        os.environ.get("MOCK_RATE_LIMIT_EVERY", "0"),
    )
)
RATE_LIMIT_RETRY_AFTER_MS = int(os.environ.get("MOCK_RATE_LIMIT_RETRY_AFTER_MS", "250"))
MESSAGE_INTERVAL_SECONDS = int(os.environ.get("MOCK_MESSAGE_INTERVAL_SECONDS", "60"))

NOW = datetime.now(timezone.utc)
MESSAGE_CONTENT = "x" * MESSAGE_CONTENT_BYTES
REQUEST_LOCK = threading.Lock()
REQUEST_COUNT = 0

GUILD_ID_BASE = 9_000_000_000_000_000
CHANNEL_ID_BASE = 8_000_000_000_000_000
MESSAGE_ID_BASE = 7_000_000_000_000_000
USER_ID_BASE = 6_000_000_000_000_000
MESSAGE_ID_STRIDE = 1_000_000_000


@dataclass(frozen=True)
class ChannelRecord:
    channel_id: str
    guild_id: str
    index_within_guild: int
    global_index: int
    kind: int
    name: str


def build_guild_ids() -> list[str]:
    if GUILD_IDS.strip():
        guild_ids = [value.strip() for value in GUILD_IDS.split(",") if value.strip()]
        if guild_ids:
            return guild_ids

    if GUILD_COUNT <= 1:
        return [GUILD_ID]

    return [str(GUILD_ID_BASE + index) for index in range(GUILD_COUNT)]


def build_channels(guild_ids: list[str]) -> tuple[dict[str, list[dict]], dict[str, ChannelRecord]]:
    channels_by_guild: dict[str, list[dict]] = {}
    lookup: dict[str, ChannelRecord] = {}
    global_channel_index = 0

    for guild_position, guild_id in enumerate(guild_ids):
        channels: list[dict] = []
        for channel_index in range(CHANNELS_PER_GUILD):
            channel_id = str(CHANNEL_ID_BASE + (guild_position * 100_000) + channel_index + 1)
            kind = 11 if THREAD_CHANNEL_EVERY and (channel_index + 1) % THREAD_CHANNEL_EVERY == 0 else 0
            name = f"bench-channel-{guild_position + 1:04d}-{channel_index + 1:04d}"
            record = ChannelRecord(
                channel_id=channel_id,
                guild_id=guild_id,
                index_within_guild=channel_index,
                global_index=global_channel_index,
                kind=kind,
                name=name,
            )
            lookup[channel_id] = record
            channels.append(
                {
                    "id": channel_id,
                    "guild_id": guild_id,
                    "name": name,
                    "type": kind,
                }
            )
            global_channel_index += 1
        channels_by_guild[guild_id] = channels

    return channels_by_guild, lookup


def build_attachment_template() -> list[dict]:
    return [
        {
            "content_type": "image/png",
            "filename": f"sample-{index}.png",
            "id": str(MESSAGE_ID_BASE + 100_000 + index),
            "size": 16_384 + index,
        }
        for index in range(ATTACHMENTS_PER_MESSAGE)
    ]


GUILD_ID_LIST = build_guild_ids()
CHANNELS_BY_GUILD, CHANNEL_LOOKUP = build_channels(GUILD_ID_LIST)
ATTACHMENTS = build_attachment_template()


def message_id_for(channel: ChannelRecord, ordinal: int) -> int:
    return MESSAGE_ID_BASE + (channel.global_index * MESSAGE_ID_STRIDE) + ordinal


def author_id_for(channel: ChannelRecord, ordinal: int) -> str:
    author_index = ((channel.global_index * MESSAGES_PER_CHANNEL) + ordinal) % max(AUTHOR_COUNT, 1)
    return str(USER_ID_BASE + author_index + 1)


def message_timestamp_for(ordinal: int) -> datetime:
    offset_seconds = (MESSAGES_PER_CHANNEL - ordinal) * MESSAGE_INTERVAL_SECONDS
    return NOW - timedelta(seconds=offset_seconds)


def build_message(channel: ChannelRecord, ordinal: int) -> dict:
    message_id = message_id_for(channel, ordinal)
    is_reply = REPLY_EVERY > 0 and ordinal % REPLY_EVERY == 0 and ordinal > 1
    reply_message_id = message_id_for(channel, ordinal - 1)
    is_bot = BOT_EVERY > 0 and ordinal % BOT_EVERY == 0
    payload = {
        "attachments": ATTACHMENTS,
        "author": {
            "bot": is_bot,
            "id": author_id_for(channel, ordinal),
        },
        "content": MESSAGE_CONTENT,
        "id": str(message_id),
        "message_reference": None,
        "referenced_message": None,
        "timestamp": message_timestamp_for(ordinal).isoformat().replace("+00:00", "Z"),
    }
    if is_reply:
        payload["message_reference"] = {
            "channel_id": channel.channel_id,
            "guild_id": channel.guild_id,
            "message_id": str(reply_message_id),
        }
        payload["referenced_message"] = {"id": str(reply_message_id)}
    return payload


def channel_messages(channel: ChannelRecord, before: Optional[int], limit: int) -> list[dict]:
    if before is None:
        start = MESSAGES_PER_CHANNEL
    else:
        start = min(before - (channel.global_index * MESSAGE_ID_STRIDE) - MESSAGE_ID_BASE - 1, MESSAGES_PER_CHANNEL)

    end = max(start - limit, 0)
    messages = []
    for ordinal in range(start, end, -1):
        messages.append(build_message(channel, ordinal))
    return messages


def should_rate_limit(path: str) -> bool:
    global REQUEST_COUNT
    if RATE_LIMIT_EVERY_N_MESSAGES_REQUESTS <= 0 or not path.endswith("/messages"):
        return False

    with REQUEST_LOCK:
        REQUEST_COUNT += 1
        return REQUEST_COUNT % RATE_LIMIT_EVERY_N_MESSAGES_REQUESTS == 0


class Handler(BaseHTTPRequestHandler):
    def do_GET(self) -> None:
        parsed = urlparse(self.path)
        path = parsed.path
        query = parse_qs(parsed.query)
        time.sleep(RESPONSE_DELAY_MS / 1000.0)

        if should_rate_limit(path):
            return self._write_json(
                {
                    "global": False,
                    "message": "rate limited",
                    "retry_after": RATE_LIMIT_RETRY_AFTER_MS / 1000.0,
                },
                status=429,
            )

        if path == "/healthz":
            return self._write_json(
                {
                    "guild_count": len(GUILD_ID_LIST),
                    "channels_per_guild": CHANNELS_PER_GUILD,
                    "messages_per_channel": MESSAGES_PER_CHANNEL,
                }
            )

        guild_channels_prefix = "/api/v10/guilds/"
        if path.startswith(guild_channels_prefix) and path.endswith("/channels"):
            guild_id = path[len(guild_channels_prefix) : -len("/channels")]
            channels = CHANNELS_BY_GUILD.get(guild_id)
            if channels is None:
                return self._write_json({"error": "guild not found"}, status=404)
            return self._write_json(channels)

        if path.startswith("/api/v10/channels/") and path.endswith("/messages"):
            parts = path.split("/")
            channel_id = parts[4]
            channel = CHANNEL_LOOKUP.get(channel_id)
            if channel is None:
                return self._write_json({"error": "channel not found"}, status=404)
            limit = min(int(query.get("limit", ["100"])[0]), 100)
            before_value = query.get("before", [None])[0]
            before = int(before_value) if before_value else None
            return self._write_json(channel_messages(channel, before, limit))

        if path.startswith("/api/v10/channels/"):
            channel_id = path.rsplit("/", 1)[-1]
            channel = CHANNEL_LOOKUP.get(channel_id)
            if channel is None:
                return self._write_json({"error": "channel not found"}, status=404)
            return self._write_json(
                {
                    "guild_id": channel.guild_id,
                    "id": channel.channel_id,
                    "name": channel.name,
                    "type": channel.kind,
                }
            )

        self._write_json({"error": "not found"}, status=404)

    def log_message(self, format: str, *args) -> None:
        return

    def _write_json(self, payload, status: int = 200) -> None:
        body = json.dumps(payload).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)


if __name__ == "__main__":
    server = ThreadingHTTPServer((HOST, PORT), Handler)
    print(f"mock discord api listening on http://{HOST}:{PORT}", flush=True)
    print(
        json.dumps(
            {
                "guild_ids": GUILD_ID_LIST,
                "channels_per_guild": CHANNELS_PER_GUILD,
                "messages_per_channel": MESSAGES_PER_CHANNEL,
                "message_content_bytes": MESSAGE_CONTENT_BYTES,
                "attachments_per_message": ATTACHMENTS_PER_MESSAGE,
                "reply_every": REPLY_EVERY,
                "bot_every": BOT_EVERY,
                "rate_limit_every": RATE_LIMIT_EVERY_N_MESSAGES_REQUESTS,
            }
        ),
        flush=True,
    )
    server.serve_forever()
