# Implementation Plan

## 1. Core contracts
Complete and harden `qs-core` contracts with schema migration tests and serde compatibility snapshots.

## 2. Storage
Extend `qs-storage` with:
- SQLite run catalog
- Parquet writers for evaluations, trades, equity, metrics and validation
- retention policies
- checkpoint history
- recovery scanner for `Running -> Interrupted`

## 3. Orchestrator
Extend `qs-orchestrator` with:
- component registry
- config-driven pipeline composition
- checkpoint hooks before/after components and after search batches
- explicit pause safe points
- clean cancellation checkpoints

## 4. Data layer
Implement concrete adapters without making any one format canonical:
- CSV adapter
- Parquet adapter
- user-defined adapter registry
- configurable normalizers
- full gap/outlier quality reports

## 5. Strategy API
Implement plugin discovery/loading strategy. Prefer static plugin crates first for safety; dynamic loading can be added behind a feature flag after ABI/versioning is finalized.

## 6. Search
Implement:
- grid/lazy generators
- sparse exploration
- Latin Hypercube/Sobol optional samplers
- explicit neighborhood metrics
- intensification around stable plateaus
- controlled completion
- serializable search state

## 7. Backtest
Implement deterministic execution model:
- signal/order-intent conversion
- fill model
- costs/slippage
- trade ledger
- equity curve

## 8. Validation
Fill components:
- WalkForward
- MonteCarlo
- SensitivityAnalysis
- RegimeAnalysis
- VarianceStabilityAnalysis
- ExecutionStress
- ParameterDecay

## 9. UI/API
Implement:
- Axum command APIs
- WebSocket envelope with sequence numbers
- bounded per-client queues
- reconnect snapshots
- React/Svelte dashboard

## 10. Export
Implement:
- selected parameters JSON
- Python bot pack
- MT5 `.set` and JSON pack
- reproducibility manifest with dataset, seed, pipeline version, strategy ids, metrics and timestamps
