# Strategy plugin model

The core does not know trading logic. A strategy plugin declares:

- metadata
- parameter space
- parameter validation
- run function producing standard signals/order intents

Recommended rollout:

1. static registry of Rust strategy crates
2. process-isolated plugins for untrusted strategies
3. optional WASM sandbox
4. dynamic native libraries only after ABI/versioning is frozen
