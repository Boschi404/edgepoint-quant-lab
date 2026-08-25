# Validation architecture

Implemented baseline reports:

- WalkForward: fold totals, average R and consistency ratio
- MonteCarlo: deterministic trade bootstrap with p05/p50/p95 and probability negative
- SensitivityAnalysis: local trade dispersion and fragility score
- RegimeAnalysis: positive/negative trade buckets and inter-bucket variance
- ExecutionStress: simple slippage-per-trade stress loss
- ParameterDecay: first-half vs second-half edge decay ratio

These are baseline statistical validators. Production hardening should add rolling/expanding walk-forward, block bootstrap, explicit market regime classifiers, execution latency/fill models and parameter surface perturbation.

## API

Validation reports are exposed through:

```text
GET /api/runs/{run_id}/validation
```
