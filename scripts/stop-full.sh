#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PID_DIR="${QS_PID_DIR:-$ROOT_DIR/.pids}"
LOG_DIR="${QS_LOG_DIR:-$ROOT_DIR/logs}"
mkdir -p "$LOG_DIR"
STOP_LOG="$LOG_DIR/stop-$(date -u +%Y%m%dT%H%M%SZ).log"

stop_pid_file() {
  local name="$1"
  local file="$PID_DIR/$name.pid"
  if [[ ! -f "$file" ]]; then
    echo "[$name] no pid file" | tee -a "$STOP_LOG"
    return 0
  fi
  local pid
  pid="$(cat "$file")"
  if kill -0 "$pid" >/dev/null 2>&1; then
    echo "[$name] stopping pid $pid" | tee -a "$STOP_LOG"
    kill "$pid" >/dev/null 2>&1 || true
    for _ in $(seq 1 20); do
      if ! kill -0 "$pid" >/dev/null 2>&1; then break; fi
      sleep 0.5
    done
    if kill -0 "$pid" >/dev/null 2>&1; then
      echo "[$name] forcing kill pid $pid" | tee -a "$STOP_LOG"
      kill -9 "$pid" >/dev/null 2>&1 || true
    fi
  else
    echo "[$name] pid $pid not running" | tee -a "$STOP_LOG"
  fi
  rm -f "$file"
}

stop_pid_file api
stop_pid_file ui

echo "Stopped. Log: $STOP_LOG"
