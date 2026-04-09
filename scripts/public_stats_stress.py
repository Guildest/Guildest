#!/usr/bin/env python3
import argparse
import json
import socket
import statistics
import time
import urllib.request
import uuid
from concurrent.futures import ThreadPoolExecutor, as_completed
from datetime import datetime, timezone


def encode_redis_command(*parts):
    encoded = []
    for part in parts:
        if isinstance(part, bytes):
            value = part
        else:
            value = str(part).encode()
        encoded.append(b"$" + str(len(value)).encode() + b"\r\n" + value + b"\r\n")
    return b"*" + str(len(parts)).encode() + b"\r\n" + b"".join(encoded)


def read_redis_reply(reader):
    prefix = reader.read(1)
    if not prefix:
        raise RuntimeError("redis connection closed")

    line = reader.readline().rstrip(b"\r\n")
    if prefix == b"+":
        return line.decode()
    if prefix == b"-":
        raise RuntimeError(f"redis error: {line.decode()}")
    if prefix == b":":
        return int(line)
    if prefix == b"$":
        length = int(line)
        if length == -1:
            return None
        payload = reader.read(length)
        reader.read(2)
        return payload
    if prefix == b"*":
        length = int(line)
        return [read_redis_reply(reader) for _ in range(length)]

    raise RuntimeError(f"unexpected redis reply prefix: {prefix!r}")


def publish_large_events_once(args):
    guild_id = args.guild_id
    channel_id = args.channel_id
    user_id = args.user_id
    now = datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")
    padding = "x" * args.padding_bytes

    started = time.perf_counter()
    with socket.create_connection((args.redis_host, args.redis_port), timeout=30) as conn:
        conn.settimeout(30)
        reader = conn.makefile("rb")
        conn.sendall(encode_redis_command("SELECT", args.redis_db))
        read_redis_reply(reader)

        for index in range(args.events):
            envelope = {
                "event_id": str(uuid.uuid4()),
                "event_name": "message_created",
                "guild_id": guild_id,
                "channel_id": channel_id,
                "user_id": user_id,
                "occurred_at": now,
                "received_at": now,
                "version": 1,
                "payload": {
                    "type": "message_created",
                    "data": {
                        "message_id": f"stress-{index}-{uuid.uuid4()}",
                        "author_id": user_id,
                        "is_bot": False,
                        "is_reply": False,
                        "attachment_count": 0,
                        "content_length": min(args.padding_bytes, 2_000_000_000),
                    },
                },
                "padding": padding,
            }
            payload = json.dumps(envelope, separators=(",", ":")).encode()
            conn.sendall(encode_redis_command("XADD", "events.message", "*", "payload", payload))
            read_redis_reply(reader)

    elapsed_ms = (time.perf_counter() - started) * 1000
    return {
        "events": args.events,
        "padding_bytes": args.padding_bytes,
        "publish_total_ms": round(elapsed_ms, 1),
        "publish_avg_ms": round(elapsed_ms / max(args.events, 1), 2),
    }


def fetch_public_stats(url):
    with urllib.request.urlopen(url, timeout=10) as response:
        return json.load(response)


def publish_large_events(args):
    print(json.dumps(publish_large_events_once(args), indent=2))


def publish_and_wait(args):
    started_stats = fetch_public_stats(args.api_url)
    started = time.perf_counter()
    publish = publish_large_events_once(args)
    published_at = time.perf_counter()
    target_messages = started_stats["messages_tracked"] + args.events

    while True:
        current_stats = fetch_public_stats(args.api_url)
        if current_stats["messages_tracked"] >= target_messages:
            finished = time.perf_counter()
            print(
                json.dumps(
                    {
                        "start_messages": started_stats["messages_tracked"],
                        "end_messages": current_stats["messages_tracked"],
                        "delta_messages": current_stats["messages_tracked"]
                        - started_stats["messages_tracked"],
                        "publish": publish,
                        "wait_after_publish_ms": round(
                            (finished - published_at) * 1000, 1
                        ),
                        "end_to_end_ms": round((finished - started) * 1000, 1),
                    },
                    indent=2,
                )
            )
            return
        time.sleep(0.05)


def fetch_once(url):
    started = time.perf_counter()
    with urllib.request.urlopen(url, timeout=10) as response:
        body = response.read()
    elapsed_ms = (time.perf_counter() - started) * 1000
    return elapsed_ms, len(body)


def run_load(args):
    latencies = []
    sizes = []
    started = time.perf_counter()
    with ThreadPoolExecutor(max_workers=args.concurrency) as executor:
        futures = [executor.submit(fetch_once, args.url) for _ in range(args.requests)]
        for future in as_completed(futures):
            latency_ms, size = future.result()
            latencies.append(latency_ms)
            sizes.append(size)
    total_ms = (time.perf_counter() - started) * 1000
    latencies.sort()

    def percentile(p):
        if not latencies:
            return 0.0
        index = min(len(latencies) - 1, max(0, int(round((p / 100) * (len(latencies) - 1)))))
        return latencies[index]

    print(
        json.dumps(
            {
                "requests": args.requests,
                "concurrency": args.concurrency,
                "total_ms": round(total_ms, 1),
                "requests_per_sec": round(args.requests / max(total_ms / 1000, 0.001), 1),
                "latency_ms": {
                    "min": round(min(latencies), 2),
                    "avg": round(statistics.mean(latencies), 2),
                    "p50": round(percentile(50), 2),
                    "p95": round(percentile(95), 2),
                    "p99": round(percentile(99), 2),
                    "max": round(max(latencies), 2),
                },
                "response_bytes": {
                    "min": min(sizes),
                    "max": max(sizes),
                },
            },
            indent=2,
        )
    )


def build_parser():
    parser = argparse.ArgumentParser(description="Stress helpers for Guildest public stats.")
    subparsers = parser.add_subparsers(dest="command", required=True)

    publish = subparsers.add_parser("publish-large-events")
    publish.add_argument("--redis-host", default="127.0.0.1")
    publish.add_argument("--redis-port", type=int, default=16379)
    publish.add_argument("--redis-db", type=int, default=13)
    publish.add_argument("--events", type=int, default=100)
    publish.add_argument("--padding-bytes", type=int, default=1_000_000)
    publish.add_argument("--guild-id", required=True)
    publish.add_argument("--channel-id", required=True)
    publish.add_argument("--user-id", required=True)
    publish.set_defaults(func=publish_large_events)

    end_to_end = subparsers.add_parser("publish-and-wait")
    end_to_end.add_argument("--api-url", required=True)
    end_to_end.add_argument("--redis-host", default="127.0.0.1")
    end_to_end.add_argument("--redis-port", type=int, default=16379)
    end_to_end.add_argument("--redis-db", type=int, default=13)
    end_to_end.add_argument("--events", type=int, default=100)
    end_to_end.add_argument("--padding-bytes", type=int, default=1_000_000)
    end_to_end.add_argument("--guild-id", required=True)
    end_to_end.add_argument("--channel-id", required=True)
    end_to_end.add_argument("--user-id", required=True)
    end_to_end.set_defaults(func=publish_and_wait)

    load = subparsers.add_parser("load")
    load.add_argument("--url", required=True)
    load.add_argument("--requests", type=int, default=1000)
    load.add_argument("--concurrency", type=int, default=100)
    load.set_defaults(func=run_load)

    return parser


def main():
    parser = build_parser()
    args = parser.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
