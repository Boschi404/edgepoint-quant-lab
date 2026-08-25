# Debugging and full startup runbook

## One-command full local start

Inside the sterile container or any machine with Rust, Node and npm:

```bash
cp .env.example .env
scripts/start-full.sh
```

This starts:

- Axum API on `QS_BIND`, default `0.0.0.0:8080`
- Vite UI on `QS_UI_PORT`, default `3000`

It writes a timestamped log session:

```text
logs/session-YYYYMMDDTHHMMSSZ/
├── api.log
├── boot.log
├── environment.log
└── ui.log
```

`logs/latest` points to the most recent session.

## Tail logs

```bash
scripts/tail-logs.sh
```

or a specific session:

```bash
scripts/tail-logs.sh logs/session-20260101T000000Z
```

## Status

```bash
scripts/status-full.sh
```

## Stop

```bash
scripts/stop-full.sh
```

## Collect a debug bundle

```bash
scripts/collect-debug-bundle.sh
```

This creates:

```text
debug-bundles/debug-YYYYMMDDTHHMMSSZ.tar.gz
```

Included:

- latest logs
- configs
- sample data fixture
- project file list
- project status
- run JSON/JSONL artifacts and manifests
- environment metadata

## Docker Compose debug mode

If Docker is available:

```bash
docker compose -f docker-compose.debug.yml up --build
```

Logs are streamed to console and also written to:

```text
logs/docker/api.log
logs/docker/ui.log
```

## Useful API debug commands

```bash
curl http://localhost:8080/api/health
curl -X POST http://localhost:8080/api/runs
curl http://localhost:8080/api/runs
curl http://localhost:8080/api/recoverable
curl http://localhost:8080/api/runs/{run_id}/snapshot
curl http://localhost:8080/api/runs/{run_id}/search-state
curl http://localhost:8080/api/runs/{run_id}/ranking
curl http://localhost:8080/api/runs/{run_id}/validation
curl http://localhost:8080/api/runs/{run_id}/artifacts
curl http://localhost:8080/api/runs/{run_id}/results/metrics
```

## Most important files during debugging

```text
runs/catalog/runs.sqlite
runs/checkpoints/{run_id}/latest.checkpoint.json
runs/checkpoints/{run_id}/search_state.latest.json
runs/results/{run_id}/evaluations.jsonl
runs/results/{run_id}/trades.jsonl
runs/results/{run_id}/equity.jsonl
runs/results/{run_id}/metrics.jsonl
runs/results/{run_id}/*.columns.json
runs/artifacts/{run_id}/report.json
runs/artifacts/{run_id}/backup_manifest.json
runs/artifacts/{run_id}/live_export/manifest.json
```
