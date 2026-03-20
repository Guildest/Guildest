#!/usr/bin/env python3
import json
import os
import time
from datetime import datetime, timedelta, timezone
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import Optional
from urllib.parse import parse_qs, urlparse


HOST = os.environ.get("MOCK_DISCORD_HOST", "127.0.0.1")
PORT = int(os.environ.get("MOCK_DISCORD_PORT", "9191"))
GUILD_ID = os.environ.get("MOCK_GUILD_ID", "bench-guild")
CHANNEL_COUNT = int(os.environ.get("MOCK_CHANNEL_COUNT", "6"))
MESSAGES_PER_CHANNEL = int(os.environ.get("MOCK_MESSAGES_PER_CHANNEL", "1200"))
MESSAGE_CONTENT_BYTES = int(os.environ.get("MOCK_MESSAGE_CONTENT_BYTES", "4096"))
ATTACHMENTS_PER_MESSAGE = int(os.environ.get("MOCK_ATTACHMENTS_PER_MESSAGE", "2"))
RESPONSE_DELAY_MS = int(os.environ.get("MOCK_RESPONSE_DELAY_MS", "10"))
AUTHOR_COUNT = int(os.environ.get("MOCK_AUTHOR_COUNT", "32"))

NOW = datetime.now(timezone.utc)
CHANNELS = [
    {"id": f"channel-{index + 1}", "type": 0}
    for index in range(CHANNEL_COUNT)
]
ATTACHMENTS = [{"id": f"attachment-{index}"} for index in range(ATTACHMENTS_PER_MESSAGE)]
MESSAGE_CONTENT = "x" * MESSAGE_CONTENT_BYTES


def channel_messages(channel_id: str, before: Optional[int], limit: int) -> list[dict]:
    channel_number = int(channel_id.rsplit("-", 1)[-1])
    channel_offset = channel_number * 10_000_000
    if before is None:
        start = MESSAGES_PER_CHANNEL
    else:
        start = min((before - channel_offset) - 1, MESSAGES_PER_CHANNEL)

    end = max(start - limit, 0)
    messages = []
    for value in range(start, end, -1):
        offset_minutes = (MESSAGES_PER_CHANNEL - value) + 1
        timestamp = NOW - timedelta(minutes=offset_minutes)
        author_index = (value % AUTHOR_COUNT) + 1
        messages.append(
            {
                "attachments": ATTACHMENTS,
                "author": {
                    "bot": False,
                    "id": f"user-{author_index}",
                },
                "content": MESSAGE_CONTENT,
                "id": str(channel_offset + value),
                "message_reference": None,
                "referenced_message": None,
                "timestamp": timestamp.isoformat().replace("+00:00", "Z"),
            }
        )
    return messages


class Handler(BaseHTTPRequestHandler):
    def do_GET(self) -> None:
        parsed = urlparse(self.path)
        path = parsed.path
        query = parse_qs(parsed.query)
        time.sleep(RESPONSE_DELAY_MS / 1000.0)

        if path == f"/api/v10/guilds/{GUILD_ID}/channels":
            return self._write_json(CHANNELS)

        if path.startswith("/api/v10/channels/") and path.endswith("/messages"):
            parts = path.split("/")
            channel_id = parts[4]
            limit = min(int(query.get("limit", ["100"])[0]), 100)
            before_value = query.get("before", [None])[0]
            before = int(before_value) if before_value else None
            return self._write_json(channel_messages(channel_id, before, limit))

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
    server.serve_forever()
