#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT_DIR/scripts/env-load.sh"
PID_DIR="${QS_PID_DIR:-$ROOT_DIR/.pids}"

status_one() {
  local name="$1"
  local file="$PID_DIR/$name.pid"
  if [[ -f "$file" ]] && kill -0 "$(cat "$file")" >/dev/null 2>&1; then
    echo "$name: running pid $(cat "$file")"
  else
    echo "$name: stopped"
  fi
}

status_one api
status_one ui
curl -fsS "http://127.0.0.1:${QS_BIND##*:}/api/health" >/dev/null 2>&1 && echo "api health: ok" || echo "api health: unavailable"
echo "latest logs: $ROOT_DIR/logs/latest"
