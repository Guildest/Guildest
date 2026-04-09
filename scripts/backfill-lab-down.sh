#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COMPOSE_FILE="$ROOT_DIR/infra/docker-compose.backfill-lab.yml"
PROJECT_NAME="${BACKFILL_LAB_PROJECT_NAME:-guildest-backfill-lab}"
STATE_DIR="${BACKFILL_LAB_STATE_DIR:-$ROOT_DIR/.backfill-lab}"
COMPOSE_BIN="${BACKFILL_LAB_COMPOSE_BIN:-docker-compose}"

for name in worker api mock-discord; do
  pid_file="$STATE_DIR/$name.pid"
  if [[ -f "$pid_file" ]]; then
    pid="$(<"$pid_file")"
    if kill -0 "$pid" 2>/dev/null; then
      kill "$pid" 2>/dev/null || true
    fi
    rm -f "$pid_file"
  fi
done

"$COMPOSE_BIN" -f "$COMPOSE_FILE" -p "$PROJECT_NAME" down -v --remove-orphans
