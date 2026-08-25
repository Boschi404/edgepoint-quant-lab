#!/usr/bin/env bash
set -euo pipefail
export QS_BIND="${QS_BIND:-0.0.0.0:8080}"
exec cargo run -p qs-app
