#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ENV_FILE="${BACKFILL_LAB_ENV_FILE:-$ROOT_DIR/.env.backfill-lab}"
COMPOSE_FILE="$ROOT_DIR/infra/docker-compose.backfill-lab.yml"
PROJECT_NAME="${BACKFILL_LAB_PROJECT_NAME:-guildest-backfill-lab}"
COMPOSE_BIN="${BACKFILL_LAB_COMPOSE_BIN:-docker-compose}"

if [[ ! -f "$ENV_FILE" ]]; then
  cp "$ROOT_DIR/infra/backfill-lab.env.example" "$ENV_FILE"
fi

set -a
source "$ENV_FILE"
set +a

wait_for_http() {
  local url="$1"
  local label="$2"

  for _ in $(seq 1 90); do
    if curl -fsS "$url" >/dev/null 2>&1; then
      return
    fi
    sleep 1
  done

  echo "$label did not become ready: $url" >&2
  exit 1
}

wait_for_command() {
  local label="$1"
  local command="$2"

  for _ in $(seq 1 90); do
    if bash -lc "$command" >/dev/null 2>&1; then
      return
    fi
    sleep 1
  done

  echo "$label did not become ready" >&2
  exit 1
}

cleanup() {
  local exit_code=$?
  jobs -p | xargs -r kill 2>/dev/null || true
  "$COMPOSE_BIN" -f "$COMPOSE_FILE" -p "$PROJECT_NAME" down -v --remove-orphans >/dev/null 2>&1 || true
  exit "$exit_code"
}

trap cleanup EXIT INT TERM

"$COMPOSE_BIN" -f "$COMPOSE_FILE" -p "$PROJECT_NAME" up -d
wait_for_command \
  "postgres" \
  "\"$COMPOSE_BIN\" -f \"$COMPOSE_FILE\" -p \"$PROJECT_NAME\" exec -T postgres pg_isready -U guildest -d guildest"
wait_for_command \
  "redis" \
  "\"$COMPOSE_BIN\" -f \"$COMPOSE_FILE\" -p \"$PROJECT_NAME\" exec -T redis valkey-cli ping"

cargo build -p api -p worker

(
  cd "$ROOT_DIR"
  exec python3 infra/mock_discord_api.py
) &
wait_for_http \
  "http://${MOCK_DISCORD_HOST}:${MOCK_DISCORD_PORT}/api/v10/guilds/${MOCK_GUILD_ID}/channels" \
  "mock discord api"

(
  cd "$ROOT_DIR"
  exec target/debug/api
) &
wait_for_http "http://${API_BIND_ADDR}/health" "api"

(
  cd "$ROOT_DIR"
  exec target/debug/worker
) &
wait_for_http "http://${WORKER_METRICS_BIND_ADDR}/metrics" "worker metrics"

cat <<EOF
backfill lab is serving
api: http://${API_BIND_ADDR}
worker metrics: http://${WORKER_METRICS_BIND_ADDR}/metrics
mock discord api: http://${MOCK_DISCORD_HOST}:${MOCK_DISCORD_PORT}/api/v10
press Ctrl-C to stop the lab
EOF

wait
