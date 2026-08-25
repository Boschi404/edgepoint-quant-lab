#!/usr/bin/env bash
# Source this file from launcher scripts. It loads .env if present, otherwise .env.example defaults.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ENV_FILE="${QS_ENV_FILE:-$ROOT_DIR/.env}"
DEFAULT_ENV_FILE="$ROOT_DIR/.env.example"

load_env_file() {
  local file="$1"
  [[ -f "$file" ]] || return 0
  while IFS= read -r line || [[ -n "$line" ]]; do
    [[ -z "$line" || "$line" =~ ^[[:space:]]*# ]] && continue
    if [[ "$line" =~ ^[A-Za-z_][A-Za-z0-9_]*= ]]; then
      local key="${line%%=*}"
      local value="${line#*=}"
      if [[ -z "${!key+x}" ]]; then
        export "$key=$value"
      fi
    fi
  done < "$file"
}

load_env_file "$DEFAULT_ENV_FILE"
load_env_file "$ENV_FILE"

export QS_BIND="${QS_BIND:-0.0.0.0:8080}"
export QS_RUNS_ROOT="${QS_RUNS_ROOT:-$ROOT_DIR/runs}"
export QS_DATASETS_CONFIG="${QS_DATASETS_CONFIG:-$ROOT_DIR/configs/datasets.toml}"
export RUST_LOG="${RUST_LOG:-info,qs_app=debug,qs_api=debug,qs_orchestrator=debug}"
export RUST_BACKTRACE="${RUST_BACKTRACE:-1}"
export QS_UI_HOST="${QS_UI_HOST:-0.0.0.0}"
export QS_UI_PORT="${QS_UI_PORT:-3000}"
