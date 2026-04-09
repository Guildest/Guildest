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

"$ROOT_DIR/scripts/backfill-lab-seed.sh" >/dev/null

response="$(
  curl -fsS \
    -X POST \
    -H "Cookie: guildest_session=${BACKFILL_LAB_SESSION_ID}" \
    "http://${API_BIND_ADDR}/v1/dashboard/guilds/${MOCK_GUILD_ID}/backfill?days=${BACKFILL_LAB_DAYS}"
)"

job_id="$(
  printf '%s' "$response" | python3 -c 'import json,sys; print(json.load(sys.stdin)["job_id"])'
)"

echo "queued backfill job: $job_id"

for _ in $(seq 1 180); do
  status="$(
    "$COMPOSE_BIN" -f "$COMPOSE_FILE" -p "$PROJECT_NAME" exec -T postgres \
      psql -At -U guildest -d guildest \
      -c "SELECT status FROM historical_backfill_jobs WHERE job_id = '${job_id}'"
  )"

  if [[ "$status" == "completed" ]]; then
    break
  fi

  if [[ "$status" == "failed" ]]; then
    "$COMPOSE_BIN" -f "$COMPOSE_FILE" -p "$PROJECT_NAME" exec -T postgres \
      psql -At -U guildest -d guildest \
      -c "SELECT COALESCE(last_error, '') FROM historical_backfill_jobs WHERE job_id = '${job_id}'" \
      >&2
    exit 1
  fi

  sleep 1
done

if [[ "${status:-}" != "completed" ]]; then
  echo "backfill job did not complete in time: $job_id" >&2
  exit 1
fi

summary="$(
  "$COMPOSE_BIN" -f "$COMPOSE_FILE" -p "$PROJECT_NAME" exec -T postgres \
    psql -At -U guildest -d guildest <<SQL
SELECT
    status || '|' || messages_indexed
FROM historical_backfill_jobs
WHERE job_id = '${job_id}';
SELECT
    COUNT(*) || '|' ||
    COALESCE(MIN(occurred_at)::TEXT, '') || '|' ||
    COALESCE(MAX(occurred_at)::TEXT, '')
FROM message_index
WHERE guild_id = '${MOCK_GUILD_ID}';
SQL
)"

echo "$summary"
