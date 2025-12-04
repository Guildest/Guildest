# Backend Layers

- `discord_gateway/`: Fast Discord gateway listener; normalizes events and pushes to queue. No heavy work.
- `queue/`: Queue client and helpers (Redis Streams recommended).
- `workers/`: Worker pool services (moderation, analytics, sentiment/ML).
- `api/`: FastAPI HTTP layer for onboarding, settings, analytics, Stripe hooks.
- `database/`: DB schema migrations and docs.
- `common/`: Shared libs/models/config.
