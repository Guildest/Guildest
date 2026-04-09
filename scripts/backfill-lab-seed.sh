#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ENV_FILE="${BACKFILL_LAB_ENV_FILE:-$ROOT_DIR/.env.backfill-lab}"
COMPOSE_FILE="$ROOT_DIR/infra/docker-compose.backfill-lab.yml"
PROJECT_NAME="${BACKFILL_LAB_PROJECT_NAME:-guildest-backfill-lab}"
COMPOSE_BIN="${BACKFILL_LAB_COMPOSE_BIN:-docker-compose}"

set -a
source "$ENV_FILE"
set +a

"$COMPOSE_BIN" -f "$COMPOSE_FILE" -p "$PROJECT_NAME" exec -T postgres \
  psql -v ON_ERROR_STOP=1 -U guildest -d guildest <<SQL
INSERT INTO guild_inventory (
    guild_id,
    guild_name,
    owner_id,
    member_count,
    is_active,
    last_seen_at
)
VALUES (
    '${MOCK_GUILD_ID}',
    '${BACKFILL_LAB_GUILD_NAME}',
    '${BACKFILL_LAB_USER_ID}',
    128,
    TRUE,
    NOW()
)
ON CONFLICT (guild_id) DO UPDATE
SET guild_name = EXCLUDED.guild_name,
    owner_id = EXCLUDED.owner_id,
    member_count = EXCLUDED.member_count,
    is_active = TRUE,
    last_seen_at = NOW();

DELETE FROM dashboard_session_guilds
WHERE session_id = '${BACKFILL_LAB_SESSION_ID}';

DELETE FROM dashboard_sessions
WHERE session_id = '${BACKFILL_LAB_SESSION_ID}';

INSERT INTO dashboard_sessions (
    session_id,
    discord_user_id,
    username,
    global_name,
    avatar,
    expires_at,
    created_at,
    last_seen_at
)
VALUES (
    '${BACKFILL_LAB_SESSION_ID}',
    '${BACKFILL_LAB_USER_ID}',
    '${BACKFILL_LAB_USERNAME}',
    '${BACKFILL_LAB_GLOBAL_NAME}',
    NULL,
    NOW() + INTERVAL '30 days',
    NOW(),
    NOW()
);

INSERT INTO dashboard_session_guilds (
    session_id,
    guild_id,
    guild_name,
    icon,
    is_owner,
    has_admin,
    permissions_text
)
VALUES (
    '${BACKFILL_LAB_SESSION_ID}',
    '${MOCK_GUILD_ID}',
    '${BACKFILL_LAB_GUILD_NAME}',
    NULL,
    TRUE,
    TRUE,
    '8'
);
SQL

echo "seeded dashboard session cookie: guildest_session=${BACKFILL_LAB_SESSION_ID}"
