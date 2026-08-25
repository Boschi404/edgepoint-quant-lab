#!/usr/bin/env bash
set -euo pipefail

HOST="${QS_E2E_HOST:-http://127.0.0.1:8080}"
LOG_FILE="${QS_E2E_LOG:-/tmp/quant-system-e2e.log}"

cleanup() {
  if [[ -n "${SERVER_PID:-}" ]]; then
    kill "$SERVER_PID" >/dev/null 2>&1 || true
    wait "$SERVER_PID" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

export QS_BIND="${QS_BIND:-0.0.0.0:8080}"
export QS_RUNS_ROOT="${QS_RUNS_ROOT:-./runs}"
export QS_DATASETS_CONFIG="${QS_DATASETS_CONFIG:-configs/datasets.toml}"

cargo run -p qs-app >"$LOG_FILE" 2>&1 &
SERVER_PID=$!

for _ in $(seq 1 60); do
  if curl -fsS "$HOST/api/health" >/dev/null 2>&1; then
    break
  fi
  sleep 1
done

curl -fsS "$HOST/api/health" >/dev/null
RUN_JSON="$(curl -fsS -X POST "$HOST/api/runs")"
RUN_ID="$(printf '%s' "$RUN_JSON" | jq -r '.run_id')"
if [[ -z "$RUN_ID" || "$RUN_ID" == "null" ]]; then
  echo "Failed to create run: $RUN_JSON" >&2
  exit 1
fi

echo "Created run: $RUN_ID"

for _ in $(seq 1 120); do
  STATE="$(curl -fsS "$HOST/api/runs/$RUN_ID" | jq -r '.state')"
  echo "state=$STATE"
  if [[ "$STATE" == "Completed" ]]; then
    break
  fi
  if [[ "$STATE" == "Failed" ]]; then
    echo "Run failed. Server log:" >&2
    cat "$LOG_FILE" >&2
    exit 1
  fi
  sleep 1
done

STATE="$(curl -fsS "$HOST/api/runs/$RUN_ID" | jq -r '.state')"
if [[ "$STATE" != "Completed" ]]; then
  echo "Run did not complete in time; final state=$STATE" >&2
  cat "$LOG_FILE" >&2
  exit 1
fi

curl -fsS "$HOST/api/runs/$RUN_ID/ranking" | jq . >/dev/null
curl -fsS "$HOST/api/runs/$RUN_ID/artifacts" | jq . >/dev/null
curl -fsS "$HOST/api/runs/$RUN_ID/results/metrics" | jq . >/dev/null
curl -fsS "$HOST/api/runs/$RUN_ID/results/trades" | jq . >/dev/null
curl -fsS "$HOST/api/runs/$RUN_ID/results/evaluations" | jq . >/dev/null

test -f "$QS_RUNS_ROOT/artifacts/$RUN_ID/report.json"
test -f "$QS_RUNS_ROOT/artifacts/$RUN_ID/live_export/manifest.json"
test -f "$QS_RUNS_ROOT/results/$RUN_ID/evaluations.jsonl"
test -f "$QS_RUNS_ROOT/results/$RUN_ID/compaction_manifest.json"
test -f "$QS_RUNS_ROOT/results/$RUN_ID/evaluations.columns.json"

echo "E2E smoke test passed for $RUN_ID"
