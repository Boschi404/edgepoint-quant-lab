# Run supervisor

`qs-app` wires the HTTP API to a concrete `AppRunLauncher`.

Flow:

```text
POST /api/runs
  -> qs-api creates RunHandle
  -> RunLauncher::launch(handle)
  -> AppRunLauncher prepares storage layout
  -> initial catalog record is written
  -> PipelineContext is built
  -> PipelineOrchestrator::run_with_checkpoints()
  -> ProgressEvent stream is published
  -> final catalog state is persisted
```

The API crate owns HTTP/WebSocket contracts but does not know concrete components. `qs-app` injects the launcher, keeping the API boundary reusable.

## Current runtime behavior

The launcher runs the configured component chain in a background Tokio task. Components publish progress through `ProgressSink`. Checkpoints are saved before the run, before/after components and at terminal states.

## Next hardening steps

- persistent run handles rehydrated from SQLite on startup
- explicit resume command loading checkpoint into `PipelineContext`
- per-run task registry
- bounded worker pool for concurrent runs
- structured cancellation terminal state
