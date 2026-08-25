#!/usr/bin/env bash
set -euo pipefail

missing=0
check_cmd() {
  if command -v "$1" >/dev/null 2>&1; then
    printf "[ok]   %s: %s\n" "$1" "$(command -v "$1")"
  else
    printf "[miss] %s\n" "$1"
    missing=1
  fi
}

check_cmd rustc
check_cmd cargo
check_cmd rustfmt
check_cmd sqlite3
check_cmd jq
check_cmd node
check_cmd npm

if command -v cargo >/dev/null 2>&1; then
  echo "Rust version: $(rustc --version)"
  echo "Cargo version: $(cargo --version)"
fi

if [[ $missing -ne 0 ]]; then
  echo "Some tools are missing. If you are on the host, run: make dev && make shell"
  exit 1
fi
