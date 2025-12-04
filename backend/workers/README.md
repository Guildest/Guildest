# Worker Pool
- Separate worker processes for moderation, analytics, sentiment.
- Consume queue messages; ack/fail with retries.
- Use shared config and telemetry from ../common.
