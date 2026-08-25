# Parameter search architecture

Search phases:

1. parameter-space generation
2. sparse exploration
3. scoring and ranking
4. neighborhood intensification
5. controlled completion
6. final ranking handoff

The search state must be checkpointable and include:

- evaluated ids
- pending ids
- failed ids
- phase
- best-so-far
- queue/frontier state
- RNG/seed state

The baseline implementation includes grid generation and normalized mixed-distance neighborhood logic.

## Implemented baseline

Budgeted generation now chooses between:

- exhaustive grid when estimated space cardinality is within budget
- deterministic sparse sampling when the space exceeds budget

The sparse sampler is seed-based, reproducible and emits candidates with `ParameterSetSource::RandomSparse`.

Next search hardening remains priority-queue intensification, plateau clustering and controlled completion after evaluation feedback.

## Intensification planning

`qs-search::advanced` includes a reusable intensification planner that ranks evaluated candidates with a robust score and produces:

- selected top candidate ids
- neighborhood intensification candidates
- completion candidates

The current runtime still evaluates the generated candidate set in `StrategyRunner`; the next step is to move batch scheduling fully into the search component so it can checkpoint after each batch.

## Runtime search state updates

`StrategyRunner` updates `search_state` after each candidate evaluation with:

- pending
- evaluated
- failed
- best_score_so_far

This makes mid-run checkpoints more useful even before the full search scheduler is moved out of the runner.

## Per-candidate checkpoint

During `StrategyRunner`, the current `search_state` is atomically persisted to:

```text
runs/checkpoints/{run_id}/search_state.latest.json
```

This provides a lightweight per-candidate progress checkpoint in addition to component-level checkpoints.

## Runtime scheduler primitives

`RuntimeSearchState` and `RuntimeSearchConfig` provide batch scheduling primitives:

- `next_batch()`
- `record_evaluation()`
- `record_failure()`
- `maybe_advance_phase()`
- progress snapshots

The runtime state is serializable and initialized by `ParameterSearch`. Candidate-level progress is persisted separately by `StrategyRunner` until the full evaluator loop is moved into the search component.

## Runtime use by StrategyRunner

When `search_runtime_state` is present in `PipelineContext`, `StrategyRunner` uses `RuntimeSearchState::next_batch()` to evaluate candidates in batches, records evaluations/failures, advances phases and persists per-candidate search state. This is the bridge toward a fully separate search scheduler component.
