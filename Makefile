SHELL := /usr/bin/env bash
COMPOSE := docker compose

.PHONY: help dev shell build check fmt clippy test deny clean doctor init-runs

help:
	@echo "Quant System developer commands"
	@echo ""
	@echo "Host commands, require Docker only:"
	@echo "  make dev        Build/start sterile dev container"
	@echo "  make shell      Open shell inside dev container"
	@echo "  make down       Stop dev container"
	@echo ""
	@echo "Inside container commands:"
	@echo "  make check      cargo check --workspace"
	@echo "  make fmt        cargo fmt --all"
	@echo "  make clippy     cargo clippy --workspace --all-targets"
	@echo "  make test       cargo test --workspace"
	@echo "  make deny       cargo deny check, if installed"
	@echo "  make doctor     Verify expected tools"
	@echo "  make init-runs  Create local runs/ directories
	@echo "  make ui-dev     Start frontend dev server inside container"
	@echo "  make ui-build   Build frontend"
	@echo "  make prod-build Build production Docker image
	@echo "  make api        Run Axum API inside container"
	@echo "  make full-check Run fmt/check/clippy/test/deny"
	@echo "  make e2e       Run local API end-to-end smoke test"
	@echo "  make static-validate Run repository static validation"
	@echo "  make start      Start API + UI with timestamped logs"
	@echo "  make stop       Stop API + UI started by make start"
	@echo "  make status     Show API/UI process status"
	@echo "  make logs       Tail latest API/UI logs"
	@echo "  make debug-bundle Create tar.gz bundle with logs/configs/run artifacts"
	@echo "  make debug-compose Start full stack with docker-compose.debug.yml"""

# Docker UX
dev:
	$(COMPOSE) up -d --build dev

shell:
	$(COMPOSE) run --rm dev bash

down:
	$(COMPOSE) down

# Rust UX
check:
	cargo check --workspace

fmt:
	cargo fmt --all

clippy:
	cargo clippy --workspace --all-targets -- -D warnings

test:
	cargo test --workspace

deny:
	@if command -v cargo-deny >/dev/null 2>&1; then cargo deny check; else echo "cargo-deny not installed"; fi

clean:
	cargo clean

doctor:
	@./scripts/doctor.sh

init-runs:
	@mkdir -p runs/catalog runs/metadata runs/checkpoints runs/results runs/artifacts
	@echo "runs/ layout created"

ui-dev:
	cd ui && npm install && npm run dev

ui-build:
	cd ui && npm install && npm run build

prod-build:
	docker build -f Dockerfile.production -t quant-system:prod .

api:
	./scripts/run-api.sh

full-check:
	./scripts/full-check.sh

e2e:
	./scripts/e2e-smoke.sh

static-validate:
	python3 scripts/static-validate.py

start:
	./scripts/start-full.sh

stop:
	./scripts/stop-full.sh

status:
	./scripts/status-full.sh

logs:
	./scripts/tail-logs.sh

debug-bundle:
	./scripts/collect-debug-bundle.sh

debug-compose:
	docker compose -f docker-compose.debug.yml up --build
