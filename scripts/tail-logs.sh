#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOG_DIR="${QS_LOG_DIR:-$ROOT_DIR/logs}"
SESSION="${1:-$LOG_DIR/latest}"

if [[ ! -e "$SESSION" ]]; then
  echo "No log session found at $SESSION. Start with scripts/start-full.sh" >&2
  exit 1
fi

echo "Tailing logs from $SESSION"
tail -n 80 -F "$SESSION"/*.log
