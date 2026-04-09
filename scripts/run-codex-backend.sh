#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COMPOSE_FILE="$ROOT_DIR/infra/backend-minimal.compose.yml"
ENV_FILE="${CODEX_ENV_FILE:-$ROOT_DIR/.env.codex-backend}"
PID_FILE="$ROOT_DIR/.codex-backend.pid"

if [[ ! -f "$ENV_FILE" ]]; then
  echo "missing env file: $ENV_FILE" >&2
  exit 1
fi

set -a
source "$ENV_FILE"
set +a

export DATABASE_URL="${DATABASE_URL:-postgres://guildest:guildest@127.0.0.1:15432/guildest_codex}"
export REDIS_URL="${REDIS_URL:-redis://127.0.0.1:16379}"
export API_BIND_ADDR="${API_BIND_ADDR:-127.0.0.1:18080}"
export PUBLIC_API_BASE_URL="${PUBLIC_API_BASE_URL:-http://127.0.0.1:18080}"
export PUBLIC_API_ALLOWED_ORIGIN="${PUBLIC_API_ALLOWED_ORIGIN:-*}"
export PUBLIC_SITE_URL="${PUBLIC_SITE_URL:-http://127.0.0.1:3000}"
export RUST_LOG="${RUST_LOG:-warn}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/Users/ace/projects/guildest/target}"

docker-compose -f "$COMPOSE_FILE" up -d postgres redis

until docker inspect --format '{{.State.Health.Status}}' guildest-codex-postgres 2>/dev/null | grep -q healthy; do
  sleep 1
done

until docker inspect --format '{{.State.Health.Status}}' guildest-codex-redis 2>/dev/null | grep -q healthy; do
  sleep 1
done

cd "$ROOT_DIR"
printf '%s\n' "$$" > "$PID_FILE"
exec cargo run -p api
