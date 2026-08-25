# Results and artifacts

The current runtime persists three classes of output.

## 1. Incremental JSONL results

Written during `StrategyRunner` when `storage_root` is configured in `PipelineContext`:

```text
runs/results/{run_id}/evaluations.jsonl
runs/results/{run_id}/trades.jsonl
runs/results/{run_id}/metrics.jsonl
```

JSONL is append-only and useful for recovery/debugging. It is the durable baseline before Parquet compaction.

## 2. Live export artifacts

Written by `LiveExport`:

```text
runs/artifacts/{run_id}/live_export/manifest.json
runs/artifacts/{run_id}/live_export/selected_parameters.json
runs/artifacts/{run_id}/live_export/python_bot_pack/strategy_config.json
runs/artifacts/{run_id}/live_export/mt5_pack/parameters.set
```

## 3. Report summary

Written by `ReportGenerator`:

```text
runs/artifacts/{run_id}/report.json
```

This includes ranking summary and artifact references.

## API access

```text
GET /api/runs/{run_id}/ranking
GET /api/runs/{run_id}/artifacts
GET /api/runs/{run_id}/artifacts/{path}
```

Artifact paths are validated as relative normal paths to prevent path traversal.

## Next step

Columnar JSON compaction is now implemented after terminal export and writes `*.columns.json` files. The next storage upgrade is replacing/augmenting those files with true Parquet writers while preserving JSONL as the crash-recovery log.

## Columnar compaction output

After `LiveExport`, the storage compactor writes:

```text
runs/results/{run_id}/evaluations.columns.json
runs/results/{run_id}/trades.columns.json
runs/results/{run_id}/equity.columns.json
runs/results/{run_id}/metrics.columns.json
runs/results/{run_id}/compaction_manifest.json
```

These files use a simple column-array structure and are intended as an intermediate columnar equivalent until the Parquet backend is enabled.

## Idempotent recovery behavior

When a run is relaunched through recovery, persisted `evaluations.jsonl` rows are loaded into `PipelineContext.partial_results`. `StrategyRunner` builds an evaluated-id set from those rows and skips already evaluated parameter sets, preventing duplicate append rows during resume.
