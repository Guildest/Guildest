# Common

Shared code for configuration, logging, and schemas.

## Scope
- Config loaders for env-driven settings (Discord token, Redis URL, API base).
- Logging helpers; lightweight JSON logs recommended.
- Shared payload/schema definitions for queue messages and workers.

## Notes
- Database is not provisioned yet; keep DB-related config optional until ready.
- Prefer environment variables; avoid hardcoding defaults beyond local dev.
