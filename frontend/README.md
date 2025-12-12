# Guildest Frontend (Next.js)

Bootstrapped with `create-next-app` using:
- React + TypeScript (`.tsx`)
- App Router (`src/app`)
- Tailwind CSS
- ESLint

## Setup

### 1) Install deps

```bash
cd frontend
npm install
```

### 2) Configure env

Create `frontend/.env.local`:

```bash
NEXT_PUBLIC_BACKEND_URL=http://localhost:8000
API_BASE=http://localhost:8000
```

Notes:
- `NEXT_PUBLIC_BACKEND_URL` is used for the “Continue with Discord” link.
- `API_BASE` is used by server components and the `/api/backend/*` proxy.

### 3) Run

```bash
npm run dev
```

Open `http://localhost:3000`.

## Auth flow (Discord OAuth)

- Frontend sends you to backend `GET /auth/discord/login?redirect=/dashboard`.
- Backend completes OAuth and redirects back to `GET /auth/callback?token=...&redirect=...`.
- `src/app/auth/callback/route.ts` stores the signed session token in an HttpOnly cookie on the frontend origin.

## Backend proxy

`src/app/api/backend/[...path]/route.ts` forwards requests to `API_BASE` and attaches the session token as `Authorization: Bearer ...`.

Example:
- Frontend calls `GET /api/backend/me`
- Proxy forwards to `GET {API_BASE}/me`

