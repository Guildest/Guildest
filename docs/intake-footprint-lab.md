# Intake Footprint Lab

This worktree adds a standalone parser lab for the Guildest intake path. The goal is to model a server that is extremely large in membership and still emits hundreds of messages per second, then measure what the parser costs in both throughput and memory.

## What it does

- Replays synthetic Discord `MESSAGE_CREATE` gateway payloads at a configurable rate
- Uses burst-based pacing so high-rate runs do not become timer-bound
- Supports two parser modes:
  - `thin`: deserialize only the fields Guildest actually needs, borrow strings where possible, count attachments without building vectors, parse snowflakes directly to `u64`, and derive message time from the snowflake instead of parsing RFC3339 timestamps
  - `owned`: deserialize into owned `String` and `Vec` fields to provide a heavier baseline
- Exposes Prometheus metrics on `127.0.0.1:29091/metrics`
- Tracks allocator pressure with process-local gauges so peak live bytes are visible during the run
- Samples parse latency rather than recording a histogram observation for every single message by default

## Why the thin mode is the small-footprint path

For this workload, the cheapest path is:

1. Parse only the event types you care about.
2. Keep Discord snowflakes as `u64` in the hot path instead of allocating `String`s immediately.
3. Derive `MESSAGE_CREATE` time from the snowflake when possible instead of parsing the timestamp string.
4. Borrow strings instead of cloning them unless the next stage must own them.
5. Count arrays and field presence without materializing nested objects.
6. Normalize to a compact internal struct before queueing or persistence.

That combination removes most per-message heap churn from JSON parsing itself. It does not solve database or queue pressure, but it gives you a clean lower bound for the intake stage.

## Run it

From this worktree:

```bash
cargo run -p intake-lab
```

Useful overrides:

```bash
INTAKE_LAB_PARSER_MODE=thin
INTAKE_LAB_MESSAGE_RATE=1000
INTAKE_LAB_PARSE_DURATION_SAMPLE_RATE=64
INTAKE_LAB_WORKERS=8
INTAKE_LAB_TICK_MS=20
INTAKE_LAB_SAMPLE_COUNT=4096
INTAKE_LAB_AUTHOR_POOL_SIZE=25000000
INTAKE_LAB_GUILD_COUNT=10000
INTAKE_LAB_CHANNELS_PER_GUILD=64
INTAKE_LAB_SHARD_COUNT=16
INTAKE_LAB_CONTENT_BYTES=128
INTAKE_LAB_ATTACHMENT_COUNT=2
INTAKE_LAB_RUNTIME_SECONDS=60
```

To compare the heavier parser:

```bash
INTAKE_LAB_PARSER_MODE=owned cargo run -p intake-lab
```

Large-fleet example:

```bash
INTAKE_LAB_PARSER_MODE=thin \
INTAKE_LAB_MESSAGE_RATE=50000 \
INTAKE_LAB_PARSE_DURATION_SAMPLE_RATE=128 \
INTAKE_LAB_WORKERS=16 \
INTAKE_LAB_TICK_MS=20 \
INTAKE_LAB_SAMPLE_COUNT=32768 \
INTAKE_LAB_AUTHOR_POOL_SIZE=25000000 \
INTAKE_LAB_GUILD_COUNT=250000 \
INTAKE_LAB_CHANNELS_PER_GUILD=128 \
INTAKE_LAB_SHARD_COUNT=64 \
INTAKE_LAB_CONTENT_BYTES=192 \
INTAKE_LAB_ATTACHMENT_COUNT=3 \
INTAKE_LAB_RUNTIME_SECONDS=60 \
cargo run -p intake-lab
```

## Prometheus

The local and docker Prometheus configs now include `guildest-intake-lab`.

Example queries:

```promql
rate(intake_lab_messages_parsed_total{mode="thin"}[30s])
```

```promql
rate(intake_lab_input_bytes_total{mode="thin"}[30s])
```

```promql
intake_lab_allocator_live_bytes
```

```promql
intake_lab_allocator_peak_live_bytes
```

```promql
rate(intake_lab_parse_failures_total[30s])
```

```promql
intake_lab_config{mode="thin"}
```

## Interpreting results

- If `thin` and `owned` have similar throughput, parsing is not your bottleneck.
- If `owned` shows much higher `allocator_live_bytes` or `allocator_peak_live_bytes`, the cost is object materialization, not JSON tokenization.
- If `thin` stays flat in memory while rate increases, the next bottleneck is likely downstream storage or queueing rather than parsing.
- If you need exact per-message latency histograms, set `INTAKE_LAB_PARSE_DURATION_SAMPLE_RATE=1`. The default keeps histogram overhead low enough for large-scale load tests.
- If higher `INTAKE_LAB_MESSAGE_RATE` values stop increasing `messages_per_second`, the lab is now runtime-bound and you need either more workers, a larger `tick_ms`, a release build, or multiple intake processes.
