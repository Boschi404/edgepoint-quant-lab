# API contract

Current backend endpoints:

```text
GET    /api/health
GET    /api/runs
POST   /api/runs
GET    /api/runs/:run_id
POST   /api/runs/:run_id/pause
POST   /api/runs/:run_id/resume
POST   /api/runs/:run_id/cancel
GET    /api/runs/:run_id/snapshot
WS     /api/ws/runs/:run_id
```

WebSocket messages use a versioned envelope:

```json
{
  "schema_version": 1,
  "message_type": "Progress",
  "run_id": "run_x",
  "sequence": 42,
  "payload": {}
}
```

The browser is not authoritative. Reconnect starts with a snapshot.

GET /api/runs/{run_id}/ranking
GET /api/runs/{run_id}/artifacts
GET /api/runs/{run_id}/artifacts/{path}
GET /api/runs/{run_id}/results/evaluations
GET /api/runs/{run_id}/results/trades
GET /api/runs/{run_id}/results/metrics
POST /api/runs/{run_id}/recover
GET /api/recoverable
GET /api/runs/{run_id}/results/equity
GET /api/runs/{run_id}/search-state
GET /api/runs/{run_id}/validation
