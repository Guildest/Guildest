# Guildest

Monorepo for the Guildest Discord bot and supporting web services.

- `frontend/`: Next.js dashboard for analytics, settings, and billing.
- `backend/`: Services for Discord gateway, queue, workers, HTTP API, and database schemas.

## Stack Overview
- Discord gateway: Hikari (Python)
- Queue: Redis Streams (preferred) or RabbitMQ
- Workers: Python microservices (moderation, analytics, sentiment)
- API: FastAPI
- DB: Postgres (Neon/Supabase/RDS)
- Frontend: Next.js
