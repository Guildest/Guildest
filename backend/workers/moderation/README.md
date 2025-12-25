# Moderation Worker

Consumes queue events and evaluates messages for moderation signals.

## Current behavior

- Runs lightweight heuristic checks (mass mentions, invite links, link spam).
- Always “decides” an action (e.g. `reviewed`, `flagged`).
- **Paid-only persistence:** writes to Postgres `moderation_logs` only when the guild is connected to a `plus` or `premium` subscriber (per-guild audit history).

## Notes

- Keeps DB writes low for free users (no audit trail stored).
- Can be extended to dispatch alerts to a configured mod-log channel.
