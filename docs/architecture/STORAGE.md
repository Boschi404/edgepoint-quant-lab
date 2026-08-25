# Storage design

Storage is split by access pattern.

- catalog: lightweight run index
- metadata: JSON metadata snapshots
- checkpoints: atomic, versioned, checksummed
- results: columnar compressed tables
- artifacts: reports and live export packs

Target layout:

```text
runs/
├── catalog/runs.sqlite
├── metadata/{run_id}.json
├── checkpoints/{run_id}/latest.checkpoint.json
├── results/{run_id}/evaluations.parquet
├── results/{run_id}/trades.parquet
├── results/{run_id}/equity.parquet
├── results/{run_id}/metrics.parquet
└── artifacts/{run_id}/live_export/
```

## Backup manifest

`RunBackupService` scans metadata, checkpoints, results and artifacts for a run and writes:

```text
runs/artifacts/{run_id}/backup_manifest.json
```

This manifest is also referenced in `report.json` and is intended to support operator backup/export workflows.
