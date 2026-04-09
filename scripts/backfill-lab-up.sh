#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ENV_FILE="${BACKFILL_LAB_ENV_FILE:-$ROOT_DIR/.env.backfill-lab}"
COMPOSE_FILE="$ROOT_DIR/infra/docker-compose.backfill-lab.yml"
PROJECT_NAME="${BACKFILL_LAB_PROJECT_NAME:-guildest-backfill-lab}"
STATE_DIR="${BACKFILL_LAB_STATE_DIR:-$ROOT_DIR/.backfill-lab}"
COMPOSE_BIN="${BACKFILL_LAB_COMPOSE_BIN:-docker-compose}"

mkdir -p "$STATE_DIR"

if [[ ! -f "$ENV_FILE" ]]; then
  cp "$ROOT_DIR/infra/backfill-lab.env.example" "$ENV_FILE"
fi

set -a
source "$ENV_FILE"
set +a

start_process() {
  local name="$1"
  local command="$2"
  local pid_file="$STATE_DIR/$name.pid"
  local log_file="$STATE_DIR/$name.log"

  if [[ -f "$pid_file" ]]; then
    local existing_pid
    existing_pid="$(<"$pid_file")"
    if kill -0 "$existing_pid" 2>/dev/null; then
      return
    fi
    rm -f "$pid_file"
  fi

  nohup bash -lc "cd '$ROOT_DIR' && set -a && source '$ENV_FILE' && set +a && exec $command" \
    >"$log_file" 2>&1 &
  echo "$!" >"$pid_file"
}

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

"$COMPOSE_BIN" -f "$COMPOSE_FILE" -p "$PROJECT_NAME" up -d
wait_for_command \
  "postgres" \
  "\"$COMPOSE_BIN\" -f \"$COMPOSE_FILE\" -p \"$PROJECT_NAME\" exec -T postgres pg_isready -U guildest -d guildest"
wait_for_command \
  "redis" \
  "\"$COMPOSE_BIN\" -f \"$COMPOSE_FILE\" -p \"$PROJECT_NAME\" exec -T redis valkey-cli ping"

cargo build -p api -p worker

start_process "mock-discord" "python3 infra/mock_discord_api.py"
wait_for_http \
  "http://${MOCK_DISCORD_HOST}:${MOCK_DISCORD_PORT}/api/v10/guilds/${MOCK_GUILD_ID}/channels" \
  "mock discord api"

start_process "api" "target/debug/api"
wait_for_http "http://${API_BIND_ADDR}/health" "api"

start_process "worker" "target/debug/worker"
wait_for_http "http://${WORKER_METRICS_BIND_ADDR}/metrics" "worker metrics"

cat <<EOF
backfill lab is running
env file: $ENV_FILE
api: http://${API_BIND_ADDR}
worker metrics: http://${WORKER_METRICS_BIND_ADDR}/metrics
mock discord api: http://${MOCK_DISCORD_HOST}:${MOCK_DISCORD_PORT}/api/v10
logs: $STATE_DIR
EOF
