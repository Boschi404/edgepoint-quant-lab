#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT_DIR/scripts/env-load.sh"

LOG_DIR="${QS_LOG_DIR:-$ROOT_DIR/logs}"
PID_DIR="${QS_PID_DIR:-$ROOT_DIR/.pids}"
mkdir -p "$LOG_DIR" "$PID_DIR" "$QS_RUNS_ROOT"

STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
SESSION_DIR="$LOG_DIR/session-$STAMP"
mkdir -p "$SESSION_DIR"
ln -sfn "$SESSION_DIR" "$LOG_DIR/latest"

API_LOG="$SESSION_DIR/api.log"
UI_LOG="$SESSION_DIR/ui.log"
BOOT_LOG="$SESSION_DIR/boot.log"
ENV_LOG="$SESSION_DIR/environment.log"

cat > "$ENV_LOG" <<ENVINFO
timestamp_utc=$STAMP
root=$ROOT_DIR
QS_BIND=$QS_BIND
QS_RUNS_ROOT=$QS_RUNS_ROOT
QS_DATASETS_CONFIG=$QS_DATASETS_CONFIG
RUST_LOG=$RUST_LOG
RUST_BACKTRACE=$RUST_BACKTRACE
QS_UI_HOST=$QS_UI_HOST
QS_UI_PORT=$QS_UI_PORT
PATH=$PATH
ENVINFO

{
  echo "[boot] Quant System full start"
  echo "[boot] root: $ROOT_DIR"
  echo "[boot] session: $SESSION_DIR"
  echo "[boot] checking tools"
  bash "$ROOT_DIR/scripts/doctor.sh" || true
  echo "[boot] node: $(command -v node || echo missing)"
  echo "[boot] npm: $(command -v npm || echo missing)"
  echo "[boot] cargo: $(command -v cargo || echo missing)"
} | tee "$BOOT_LOG"

if [[ -f "$PID_DIR/api.pid" ]] && kill -0 "$(cat "$PID_DIR/api.pid")" >/dev/null 2>&1; then
  echo "API already running with PID $(cat "$PID_DIR/api.pid"). Stop it first with scripts/stop-full.sh" | tee -a "$BOOT_LOG"
  exit 1
fi
if [[ -f "$PID_DIR/ui.pid" ]] && kill -0 "$(cat "$PID_DIR/ui.pid")" >/dev/null 2>&1; then
  echo "UI already running with PID $(cat "$PID_DIR/ui.pid"). Stop it first with scripts/stop-full.sh" | tee -a "$BOOT_LOG"
  exit 1
fi

cd "$ROOT_DIR"

echo "[boot] starting API -> $API_LOG" | tee -a "$BOOT_LOG"
(
  set -x
  export QS_BIND QS_RUNS_ROOT QS_DATASETS_CONFIG RUST_LOG RUST_BACKTRACE
  cargo run -p qs-app
) >>"$API_LOG" 2>&1 &
echo $! > "$PID_DIR/api.pid"

echo "[boot] starting UI -> $UI_LOG" | tee -a "$BOOT_LOG"
(
  set -x
  cd "$ROOT_DIR/ui"
  if [[ ! -d node_modules ]]; then npm install; fi
  npm run dev -- --host "$QS_UI_HOST" --port "$QS_UI_PORT"
) >>"$UI_LOG" 2>&1 &
echo $! > "$PID_DIR/ui.pid"

API_URL="http://127.0.0.1:${QS_BIND##*:}/api/health"
echo "[boot] waiting for API health at $API_URL" | tee -a "$BOOT_LOG"
for i in $(seq 1 90); do
  if curl -fsS "$API_URL" >>"$BOOT_LOG" 2>&1; then
    echo "[boot] API healthy" | tee -a "$BOOT_LOG"
    break
  fi
  sleep 1
  if ! kill -0 "$(cat "$PID_DIR/api.pid")" >/dev/null 2>&1; then
    echo "[boot] API process exited early. Last log lines:" | tee -a "$BOOT_LOG"
    tail -80 "$API_LOG" | tee -a "$BOOT_LOG"
    exit 1
  fi
  if [[ "$i" == "90" ]]; then
    echo "[boot] API health timeout. Last log lines:" | tee -a "$BOOT_LOG"
    tail -120 "$API_LOG" | tee -a "$BOOT_LOG"
    exit 1
  fi
done

cat <<DONE | tee -a "$BOOT_LOG"
[boot] started successfully
[boot] API PID: $(cat "$PID_DIR/api.pid")
[boot] UI PID:  $(cat "$PID_DIR/ui.pid")
[boot] API:     http://127.0.0.1:${QS_BIND##*:}
[boot] UI:      http://127.0.0.1:$QS_UI_PORT
[boot] logs:    $SESSION_DIR
[boot] tail:    scripts/tail-logs.sh
[boot] stop:    scripts/stop-full.sh
DONE
