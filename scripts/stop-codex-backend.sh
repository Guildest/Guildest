#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COMPOSE_FILE="$ROOT_DIR/infra/backend-minimal.compose.yml"
PID_FILE="$ROOT_DIR/.codex-backend.pid"

if [[ -f "$PID_FILE" ]]; then
  pid="$(cat "$PID_FILE")"
  if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
    kill "$pid"
  fi
  rm -f "$PID_FILE"
fi

docker-compose -f "$COMPOSE_FILE" down
