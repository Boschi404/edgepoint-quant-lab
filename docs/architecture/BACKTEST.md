# Backtest and execution model

Implemented baseline:

- signal-to-trade conversion
- deterministic equity curve
- fees and slippage
- execution constraints
- order intent model
- market, limit and stop fill helpers
- lot-step normalization
- tick-size rounding

Still to harden for production:

- multi-position portfolio accounting
- margin/leverage
- partial fills based on liquidity
- multi-symbol routing
- session calendar constraints
- broker-specific contract specs

## Integrated execution path

The baseline `BacktestEngine` now converts signal entries and exits into `OrderIntent` values and uses market fill helpers to produce entry/exit fills. Trades are derived from fills, including normalized size, tick-rounded price, fees and slippage.
