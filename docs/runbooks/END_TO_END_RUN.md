# End-to-end runbook

This repository now includes a minimal end-to-end runtime path using the static example strategy and the configured CSV adapter.

## Start backend

```bash
make dev
make shell
make api
```

## Create a run

```bash
curl -X POST http://localhost:8080/api/runs
```

The run will:

1. read `configs/datasets.toml`
2. ingest the configured CSV source through the boundary adapter
3. normalize OHLCV into `MarketDataset`
4. run the data quality gate
5. load static strategy parameter spaces
6. generate candidate parameter sets
7. run the baseline backtest engine
8. compute metrics
9. build validation summaries and final ranking
10. write live export artifacts
11. checkpoint around component transitions

## Artifacts

Check:

```text
runs/artifacts/{run_id}/live_export/
```

Expected files:

```text
manifest.json
selected_parameters.json
python_bot_pack/strategy_config.json
mt5_pack/parameters.set
```

## Notes

The bundled CSV and `MovingAverageToyStrategy` are integration fixtures. They prove the pipeline wiring, not trading profitability.

## Query results through API

```bash
curl http://localhost:8080/api/runs/{run_id}/ranking
curl http://localhost:8080/api/runs/{run_id}/artifacts
```

Incremental result logs are written to:

```text
runs/results/{run_id}/evaluations.jsonl
runs/results/{run_id}/trades.jsonl
runs/results/{run_id}/metrics.jsonl
```

## Automated smoke test

Inside the sterile container:

```bash
make e2e
```

The smoke test starts the API, creates a run, waits for completion, checks ranking/artifact/result endpoints and verifies expected files exist.

## Result endpoints

```bash
curl http://localhost:8080/api/runs/{run_id}/results/evaluations
curl http://localhost:8080/api/runs/{run_id}/results/trades
curl http://localhost:8080/api/runs/{run_id}/results/metrics
```
