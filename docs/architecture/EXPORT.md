# Live export architecture

Canonical output is JSON. Platform-specific files are derived from canonical JSON.

Minimum pack:

```text
artifacts/{run_id}/live_export/
├── manifest.json
├── selected_parameters.json
├── python_bot_pack/strategy_config.json
└── mt5_pack/parameters.set
```

A bot must verify the manifest before using parameters live.
