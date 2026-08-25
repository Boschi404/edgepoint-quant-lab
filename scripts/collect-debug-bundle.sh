#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT_DIR/scripts/env-load.sh"
OUT_DIR="${QS_DEBUG_OUT:-$ROOT_DIR/debug-bundles}"
mkdir -p "$OUT_DIR"
STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
BUNDLE_DIR="$OUT_DIR/debug-$STAMP"
mkdir -p "$BUNDLE_DIR"

copy_if_exists() {
  local src="$1"
  local dst="$2"
  if [[ -e "$src" ]]; then
    mkdir -p "$(dirname "$dst")"
    cp -R "$src" "$dst"
  fi
}

{
  echo "timestamp_utc=$STAMP"
  echo "root=$ROOT_DIR"
  echo "QS_BIND=$QS_BIND"
  echo "QS_RUNS_ROOT=$QS_RUNS_ROOT"
  echo "QS_DATASETS_CONFIG=$QS_DATASETS_CONFIG"
  echo "RUST_LOG=$RUST_LOG"
  echo "RUST_BACKTRACE=$RUST_BACKTRACE"
  echo "node=$(node --version 2>/dev/null || echo missing)"
  echo "npm=$(npm --version 2>/dev/null || echo missing)"
  echo "cargo=$(cargo --version 2>/dev/null || echo missing)"
  echo "rustc=$(rustc --version 2>/dev/null || echo missing)"
  echo "git=$(git rev-parse --short HEAD 2>/dev/null || echo unavailable)"
} > "$BUNDLE_DIR/environment.txt"

copy_if_exists "$ROOT_DIR/logs/latest" "$BUNDLE_DIR/logs-latest"
copy_if_exists "$ROOT_DIR/configs" "$BUNDLE_DIR/configs"
copy_if_exists "$ROOT_DIR/data/sample_ohlcv.csv" "$BUNDLE_DIR/data/sample_ohlcv.csv"
copy_if_exists "$ROOT_DIR/PROJECT_FILES.txt" "$BUNDLE_DIR/PROJECT_FILES.txt"
copy_if_exists "$ROOT_DIR/docs/PROJECT_STATUS.md" "$BUNDLE_DIR/PROJECT_STATUS.md"

if [[ -d "$QS_RUNS_ROOT" ]]; then
  mkdir -p "$BUNDLE_DIR/runs-summary"
  find "$QS_RUNS_ROOT" -maxdepth 4 -type f \( -name '*.json' -o -name '*.jsonl' -o -name '*.log' -o -name '*.set' \) -print > "$BUNDLE_DIR/runs-summary/files.txt" || true
  while IFS= read -r file; do
    rel="${file#$QS_RUNS_ROOT/}"
    mkdir -p "$BUNDLE_DIR/runs-summary/$(dirname "$rel")"
    cp "$file" "$BUNDLE_DIR/runs-summary/$rel" 2>/dev/null || true
  done < "$BUNDLE_DIR/runs-summary/files.txt"
fi

ARCHIVE="$OUT_DIR/debug-$STAMP.tar.gz"
tar -czf "$ARCHIVE" -C "$OUT_DIR" "debug-$STAMP"
echo "Debug bundle: $ARCHIVE"
