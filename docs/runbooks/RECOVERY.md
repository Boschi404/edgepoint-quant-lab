# Recovery runbook

1. Stop the process if it is still partially alive.
2. Restart `qs-app`.
3. Startup recovery marks catalog records from `Running` to `Interrupted`.
4. Verify latest checkpoint checksum.
5. Resume only via explicit operator/UI action.
6. If checksum verification fails, archive the run as failed and keep artifacts for analysis.

Never edit checkpoint files manually unless operating in a forensic recovery copy.

## API recovery

Recoverable runs can be inspected with:

```bash
curl http://localhost:8080/api/recoverable
```

A run with a valid checkpoint can be relaunched with:

```bash
curl -X POST http://localhost:8080/api/runs/{run_id}/recover
```

The launcher loads the latest checkpoint, restores completed component state, reloads persisted evaluations from JSONL and resumes the orchestrator.

## Candidate set rebuild

During recovery, the launcher rebuilds candidate sets from registered parameter spaces before the orchestrator resumes. This prevents a recovered run from skipping `ParameterGenerator` while having an empty in-memory candidate set.
