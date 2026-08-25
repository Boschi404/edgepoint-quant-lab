#!/usr/bin/env bash
set -euo pipefail
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
if command -v cargo-deny >/dev/null 2>&1; then cargo deny check; fi
