# Run state machine

Authoritative state is persisted by the backend, never by the browser.

```text
Pending -> Running -> Completed
             |  |  \
             |  |   -> Failed
             |  -> Paused -> Running
             -> Interrupted -> Running
```

`Cancelled` is a command/result event. A cancelled run is persisted as a terminal failed/cancelled record depending on retention policy.

## Safe points

Pause and cancellation are cooperative. Components must check tokens:

- before a batch
- after a batch
- before any long write
- before artifact generation
- before component completion

## Crash recovery

On application startup:

1. scan catalog for `Running`
2. mark as `Interrupted`
3. verify latest checkpoint checksum
4. expose as recoverable in UI
5. resume only by explicit user command
