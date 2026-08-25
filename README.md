# Quant System

Sistema modulare in Rust per **ricerca parametri, backtest, validazione statistica e preparazione all'export live** di strategie quantitative.

Il progetto è pensato come base production-oriented per costruire una piattaforma locale monoutente in grado di:

- ingerire dati di mercato da formati esterni configurabili;
- normalizzarli in un modello interno stabile;
- validare la qualità dei dati;
- caricare strategie come plugin;
- generare e valutare set di parametri;
- eseguire backtest deterministici;
- calcolare metriche classiche e di stabilità;
- produrre validazioni statistiche baseline;
- salvare risultati incrementali;
- effettuare checkpoint e recovery;
- esporre una Web UI realtime;
- esportare artifact per bot Python, MT5 EA e adapter futuri.

> Nota importante: questa repository è una **piattaforma infrastrutturale avanzata**, non una promessa di profittabilità. La strategia inclusa è una fixture di integrazione, non una strategia trading pronta per il live.

---

## Indice

1. [Cos'è questo progetto](#cosè-questo-progetto)
2. [Cosa fa](#cosa-fa)
3. [Cosa non è](#cosa-non-è)
4. [Architettura](#architettura)
5. [Crate principali](#crate-principali)
6. [Flusso di una run](#flusso-di-una-run)
7. [Contratti dati](#contratti-dati)
8. [Strategie plugin](#strategie-plugin)
9. [Search parametri](#search-parametri)
10. [Backtest ed execution model](#backtest-ed-execution-model)
11. [Validation statistica](#validation-statistica)
12. [Storage, checkpoint e recovery](#storage-checkpoint-e-recovery)
13. [Export live](#export-live)
14. [API e WebSocket](#api-e-websocket)
15. [Web UI](#web-ui)
16. [Installazione](#installazione)
17. [Avvio rapido](#avvio-rapido)
18. [Uso manuale via API](#uso-manuale-via-api)
19. [Debug e log](#debug-e-log)
20. [Test e validazione](#test-e-validazione)
21. [Struttura file prodotta da una run](#struttura-file-prodotta-da-una-run)
22. [Esempi](#esempi)
23. [Troubleshooting](#troubleshooting)
24. [Limiti attuali](#limiti-attuali)
25. [Roadmap consigliata](#roadmap-consigliata)

---

# Cos'è questo progetto

`quant-system` è un sistema locale, modulare e componibile per ricerca e validazione di strategie quantitative.

È costruito intorno a questi principi:

- **core indipendente dalla logica trading**;
- strategie come **plugin**;
- ogni blocco operativo come `PipelineComponent`;
- comunicazione tra componenti solo via `PipelineContext` e tipi standard;
- contratti dati versionabili;
- checkpoint atomici;
- recovery da crash/interruzione;
- UI realtime via WebSocket;
- export strutturato per uso live.

L'obiettivo è separare nettamente:

```text
core infrastructure
strategy plugins
storage
validation
search
API/UI
live export
```

---

# Cosa fa

Il sistema può eseguire una run che, nella configurazione attuale, fa:

```text
1. legge configurazione dataset
2. ingerisce CSV tramite adapter di bordo
3. normalizza OHLCV in MarketDataset
4. valida qualità dati
5. carica strategy plugin statici
6. genera candidate parameter set
7. inizializza runtime search state
8. esegue backtest batch sui candidate
9. salva risultati incrementali JSONL
10. calcola metriche
11. genera validation report baseline
12. crea final ranking
13. produce live export pack
14. compatta JSONL in formato columnar JSON
15. scrive report finale
16. salva checkpoint atomici
17. espone tutto via API/UI
```

Output principali:

```text
runs/catalog/runs.sqlite
runs/checkpoints/{run_id}/latest.checkpoint.json
runs/checkpoints/{run_id}/search_state.latest.json
runs/results/{run_id}/evaluations.jsonl
runs/results/{run_id}/trades.jsonl
runs/results/{run_id}/equity.jsonl
runs/results/{run_id}/metrics.jsonl
runs/results/{run_id}/*.columns.json
runs/artifacts/{run_id}/report.json
runs/artifacts/{run_id}/backup_manifest.json
runs/artifacts/{run_id}/live_export/manifest.json
runs/artifacts/{run_id}/live_export/selected_parameters.json
runs/artifacts/{run_id}/live_export/python_bot_pack/strategy_config.json
runs/artifacts/{run_id}/live_export/mt5_pack/parameters.set
```

---

# Cosa non è

Questo progetto **non è**:

- un bot live completo;
- una strategia profittevole pronta;
- un motore istituzionale già completo per margin/multi-symbol/partial fills complessi;
- una piattaforma multiutente SaaS;
- un sistema garantito senza validazione `cargo check`/`cargo test` nel container.

È una base architetturale e operativa avanzata su cui costruire.

---

# Architettura

Struttura generale:

```text
quant-system/
├── crates/
│   ├── qs-core/
│   ├── qs-storage/
│   ├── qs-orchestrator/
│   ├── qs-data/
│   ├── qs-strategy-api/
│   ├── qs-example-strategy/
│   ├── qs-search/
│   ├── qs-backtest/
│   ├── qs-validation/
│   ├── qs-metrics/
│   ├── qs-export/
│   ├── qs-api/
│   └── qs-app/
├── ui/
├── configs/
├── data/
├── docs/
├── scripts/
├── runs/
├── logs/
└── debug-bundles/
```

Principio centrale:

```text
I componenti non si chiamano tra loro.
L'orchestratore decide l'ordine.
Il PipelineContext contiene dati, stato, progress, checkpoint, cancellation e storage handles.
```

---

# Crate principali

## `qs-core`

Contiene:

- ID standard: `RunId`, `StrategyId`, `DatasetId`, `ParameterSetId`;
- `PipelineComponent`;
- `PipelineContext`;
- contratti dati standard;
- `ProgressEvent`;
- errori tipizzati;
- pause/cancellation token;
- stati run.

## `qs-storage`

Contiene:

- checkpoint atomici;
- catalogo SQLite;
- JSONL result store;
- compaction columnar JSON;
- recovery service;
- retention planner;
- backup manifest.

## `qs-orchestrator`

Contiene:

- validazione ordine componenti;
- lifecycle run;
- failure policy;
- checkpoint intorno ai componenti;
- salvataggio context checkpoint.

## `qs-data`

Contiene:

- adapter raw data;
- CSV adapter;
- normalizzatore OHLCV;
- DataQualityGate.

## `qs-strategy-api`

Definisce il contratto plugin:

```rust
pub trait StrategyPlugin: Send + Sync {
    fn metadata(&self) -> StrategyMetadata;
    fn parameter_space(&self) -> ParameterSpace;
    fn validate_parameters(&self, params: &ParameterSet) -> Result<(), StrategyError>;
    fn run(&self, input: StrategyRunInput) -> Result<StrategyRunOutput, StrategyError>;
}
```

## `qs-example-strategy`

Contiene una strategia statica di integrazione:

```text
MovingAverageToyStrategy
```

Serve per testare il wiring, non per trading live.

## `qs-search`

Contiene:

- grid generation;
- sparse deterministic generation;
- neighborhood distance;
- intensification planner;
- runtime search state;
- batch scheduling primitives.

## `qs-backtest`

Contiene:

- `BacktestEngine`;
- `ExecutionModel`;
- `OrderIntent`;
- `Fill`;
- `ExecutionConstraints`;
- market/limit/stop fill helpers;
- conversione signals → fills → trades.

## `qs-validation`

Contiene baseline report per:

- WalkForward;
- MonteCarlo;
- SensitivityAnalysis;
- RegimeAnalysis;
- ExecutionStress;
- ParameterDecay.

## `qs-api`

Espone:

- HTTP API Axum;
- WebSocket progress;
- endpoints risultati;
- endpoints artifact;
- recovery endpoints.

## `qs-app`

Binary principale.

Responsabilità:

- wiring componenti;
- registry strategie;
- launcher run;
- startup recovery;
- server Axum.

---

# Flusso di una run

Pipeline di default:

```text
DataIngestion
DataNormalizer
DataQualityGate
ParameterGenerator
ParameterSearch
StrategyRunner
WalkForward
MonteCarlo
SensitivityAnalysis
RegimeAnalysis
VarianceStabilityAnalysis
ExecutionStress
ParameterDecay
FinalRanking
LiveExport
ReportGenerator
```

Ogni componente implementa:

```rust
PipelineComponent
```

e comunica solo via:

```rust
PipelineContext
```

---

# Contratti dati

Tipi principali:

- `MarketDataset`
- `MarketBar`
- `ParameterSpace`
- `ParameterSet`
- `Trade`
- `EquityPoint`
- `EvaluationResult`
- `MetricBundle`
- `DataQualityReport`
- `ProgressEvent`

Il core non conosce CSV, broker, MT5, exchange o formato esterno. Tutto passa prima da adapter/normalizer.

---

# Strategie plugin

Il core non contiene logica di trading.

Una strategia dichiara:

- metadata;
- schema parametri;
- validazione parametri;
- run su dataset normalizzato;
- output in segnali standard.

Esempio incluso:

```text
crates/qs-example-strategy/src/lib.rs
```

Contiene:

```rust
MovingAverageToyStrategy
```

---

# Search parametri

Il sistema supporta:

- generazione grid;
- generazione sparse deterministica se lo spazio supera il budget;
- distanza di vicinato normalizzata;
- intensification planner;
- completion candidates;
- runtime state serializzabile;
- batch evaluation primitives;
- per-candidate search checkpoint.

File chiave:

```text
crates/qs-search/src/generator.rs
crates/qs-search/src/advanced.rs
crates/qs-search/src/runtime.rs
```

Checkpoint search:

```text
runs/checkpoints/{run_id}/search_state.latest.json
```

---

# Backtest ed execution model

Il backtest baseline segue ora:

```text
SignalEvent
→ OrderIntent
→ Fill
→ Trade
→ EquityPoint
→ MetricBundle
```

Supporto attuale:

- market fill;
- limit fill helper;
- stop fill helper;
- lot step;
- tick size;
- fee;
- slippage;
- long/short;
- equity curve.

File:

```text
crates/qs-backtest/src/lib.rs
crates/qs-backtest/src/execution.rs
```

---

# Validation statistica

Baseline implementate:

## WalkForward

- fold temporali;
- total R per fold;
- average R per fold;
- consistency ratio.

## MonteCarlo

- bootstrap deterministico;
- p05/p50/p95 total R;
- probability negative.

## SensitivityAnalysis

- trade dispersion;
- fragility score.

## RegimeAnalysis

- positive/negative buckets;
- inter-bucket variance.

## ExecutionStress

- stress loss baseline da slippage.

## ParameterDecay

- first half vs second half;
- decay ratio.

Endpoint:

```text
GET /api/runs/{run_id}/validation
```

---

# Storage, checkpoint e recovery

## Catalogo

```text
runs/catalog/runs.sqlite
```

## Checkpoint run

```text
runs/checkpoints/{run_id}/latest.checkpoint.json
```

Pattern atomico:

```text
write temp
fsync file
fsync directory
rename atomico
fsync directory
```

## Checkpoint search

```text
runs/checkpoints/{run_id}/search_state.latest.json
```

## Result log JSONL

```text
runs/results/{run_id}/evaluations.jsonl
runs/results/{run_id}/trades.jsonl
runs/results/{run_id}/equity.jsonl
runs/results/{run_id}/metrics.jsonl
```

## Columnar compaction

```text
runs/results/{run_id}/evaluations.columns.json
runs/results/{run_id}/trades.columns.json
runs/results/{run_id}/equity.columns.json
runs/results/{run_id}/metrics.columns.json
runs/results/{run_id}/compaction_manifest.json
```

## Recovery

All'avvio, run rimaste `Running` diventano:

```text
Interrupted
```

Endpoint:

```text
GET  /api/recoverable
POST /api/runs/{run_id}/recover
```

---

# Export live

Output:

```text
runs/artifacts/{run_id}/live_export/
├── manifest.json
├── selected_parameters.json
├── python_bot_pack/
│   ├── strategy_config.json
│   └── README.md
└── mt5_pack/
    ├── parameters.set
    └── README.md
```

Report:

```text
runs/artifacts/{run_id}/report.json
```

Backup manifest:

```text
runs/artifacts/{run_id}/backup_manifest.json
```

---

# API e WebSocket

## Health

```text
GET /api/health
```

## Runs

```text
GET  /api/runs
POST /api/runs
GET  /api/runs/{run_id}
POST /api/runs/{run_id}/pause
POST /api/runs/{run_id}/resume
POST /api/runs/{run_id}/cancel
POST /api/runs/{run_id}/recover
GET  /api/recoverable
```

## Progress

```text
GET /api/runs/{run_id}/snapshot
WS  /api/ws/runs/{run_id}
```

## Results

```text
GET /api/runs/{run_id}/ranking
GET /api/runs/{run_id}/validation
GET /api/runs/{run_id}/search-state
GET /api/runs/{run_id}/results/evaluations
GET /api/runs/{run_id}/results/trades
GET /api/runs/{run_id}/results/equity
GET /api/runs/{run_id}/results/metrics
```

## Artifacts

```text
GET /api/runs/{run_id}/artifacts
GET /api/runs/{run_id}/artifacts/{path}
```

---

# Web UI

La UI è in:

```text
ui/
```

Mostra:

- run list;
- create run;
- pause/resume/recover/cancel;
- progress realtime;
- ranking;
- artifacts;
- metrics;
- trades;
- equity;
- search state;
- validation.

---

# Installazione

## Requisiti consigliati

Metodo preferito:

```text
Docker + Docker Compose
```

Oppure ambiente locale con:

```text
Rust toolchain
Cargo
Node.js
npm
sqlite3
jq
```

---

## Installazione con ambiente sterile Docker

```bash
cd quant-system
make dev
make shell
```

Dentro il container:

```bash
make doctor
make full-check
```

---

## Installazione locale senza Docker

```bash
rustup component add rustfmt clippy
cd ui
npm install
cd ..
```

Verifica:

```bash
make doctor
python3 scripts/static-validate.py
cd ui && npm run build
```

---

# Avvio rapido

Crea `.env`:

```bash
cp .env.example .env
```

Avvia API + UI:

```bash
make start
```

Oppure:

```bash
scripts/start-full.sh
```

UI:

```text
http://127.0.0.1:3000
```

API:

```text
http://127.0.0.1:8080
```

Ferma tutto:

```bash
make stop
```

Status:

```bash
make status
```

Log:

```bash
make logs
```

---

# Uso manuale via API

## Health

```bash
curl http://localhost:8080/api/health
```

## Crea run

```bash
curl -X POST http://localhost:8080/api/runs
```

Esempio risposta:

```json
{
  "run_id": "run_xxx",
  "state": "Running",
  "created_at": 123,
  "updated_at": 123,
  "pipeline_version": "0.1.0"
}
```

## Lista run

```bash
curl http://localhost:8080/api/runs
```

## Ranking

```bash
curl http://localhost:8080/api/runs/{run_id}/ranking
```

## Metriche

```bash
curl http://localhost:8080/api/runs/{run_id}/results/metrics
```

## Trades

```bash
curl http://localhost:8080/api/runs/{run_id}/results/trades
```

## Equity

```bash
curl http://localhost:8080/api/runs/{run_id}/results/equity
```

## Validation

```bash
curl http://localhost:8080/api/runs/{run_id}/validation
```

## Artifact list

```bash
curl http://localhost:8080/api/runs/{run_id}/artifacts
```

## Download manifest

```bash
curl http://localhost:8080/api/runs/{run_id}/artifacts/live_export/manifest.json
```

---

# Debug e log

## Avvio con log completi

```bash
make start
```

Crea:

```text
logs/session-YYYYMMDDTHHMMSSZ/
├── api.log
├── boot.log
├── environment.log
└── ui.log
```

`logs/latest` punta all'ultima sessione.

## Tail log live

```bash
make logs
```

## Debug bundle

```bash
make debug-bundle
```

Produce:

```text
debug-bundles/debug-YYYYMMDDTHHMMSSZ.tar.gz
```

Include:

- log;
- config;
- sample data;
- artifact run;
- JSONL risultati;
- report;
- manifest;
- environment metadata.

## Docker Compose debug

```bash
docker compose -f docker-compose.debug.yml up --build
```

Oppure:

```bash
make debug-compose
```

Log:

```text
logs/docker/api.log
logs/docker/ui.log
```

---

# Test e validazione

## Static validation

```bash
make static-validate
```

oppure:

```bash
python3 scripts/static-validate.py
```

Controlla:

- TOML validi;
- dipendenze duplicate;
- assenza di `unwrap()` / `expect()` / `unwrap_or` nei crate;
- file essenziali presenti.

## UI build

```bash
cd ui
npm run build
```

## NPM audit

```bash
cd ui
npm audit --audit-level=moderate
```

## Rust full check

Dentro Docker/devcontainer:

```bash
make full-check
```

Esegue:

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check
```

## End-to-end smoke test

```bash
make e2e
```

Verifica:

- API health;
- creazione run;
- completamento run;
- ranking endpoint;
- artifact endpoint;
- evaluations/trades/equity/metrics endpoint;
- file artifact e risultati.

---

# Struttura file prodotta da una run

```text
runs/
├── catalog/
│   └── runs.sqlite
├── checkpoints/
│   └── {run_id}/
│       ├── latest.checkpoint.json
│       └── search_state.latest.json
├── results/
│   └── {run_id}/
│       ├── evaluations.jsonl
│       ├── trades.jsonl
│       ├── equity.jsonl
│       ├── metrics.jsonl
│       ├── evaluations.columns.json
│       ├── trades.columns.json
│       ├── equity.columns.json
│       ├── metrics.columns.json
│       └── compaction_manifest.json
└── artifacts/
    └── {run_id}/
        ├── report.json
        ├── backup_manifest.json
        └── live_export/
            ├── manifest.json
            ├── selected_parameters.json
            ├── python_bot_pack/
            │   ├── strategy_config.json
            │   └── README.md
            └── mt5_pack/
                ├── parameters.set
                └── README.md
```

---

# Esempi

## Config dataset

File:

```text
configs/datasets.toml
```

Esempio:

```toml
[[datasets]]
dataset_id = "sample_dataset"
source_uri = "file://./data/sample_ohlcv.csv"
format_hint = "csv"
timezone = "UTC"
timestamp_unit = "millis"
symbol = "SAMPLE"
timeframe_name = "1m"
timeframe_seconds = 60

[datasets.columns]
timestamp = "timestamp"
open = "open"
high = "high"
low = "low"
close = "close"
volume = "volume"
spread = "spread"
```

## Esempio CSV

```csv
timestamp,open,high,low,close,volume,spread
1704067200000,100.0,101.0,99.5,100.5,1000,0.01
```

## Esempio MT5 export

```text
lookback=20
risk_per_trade=0.005
```

## Esempio selected parameters

```json
{
  "schema_version": 1,
  "run_id": "run_xxx",
  "selected": [
    {
      "strategy_id": "moving_average_toy",
      "parameter_set_id": "...",
      "parameters": {
        "lookback": 20
      }
    }
  ]
}
```

---

# Troubleshooting

## `cargo: command not found`

Soluzione:

```bash
make dev
make shell
```

Poi:

```bash
make full-check
```

---

## `rustc` / `cargo` mancanti nel doctor

Normale se sei fuori dal container.

Controlla:

```bash
make doctor
```

Se mancano tool:

```bash
make dev
make shell
```

---

## API non parte

Controlla:

```bash
logs/latest/api.log
logs/latest/boot.log
```

Oppure:

```bash
make logs
```

Possibili cause:

- Rust non installato;
- porta 8080 occupata;
- errore compilazione;
- config dataset non valida.

Soluzioni:

```bash
make status
make stop
QS_BIND=0.0.0.0:8081 make start
```

---

## UI non parte

Controlla:

```bash
logs/latest/ui.log
```

Possibili cause:

- npm mancante;
- `node_modules` non installato;
- porta 3000 occupata.

Soluzioni:

```bash
cd ui
npm install
npm run build
```

oppure cambia porta:

```bash
QS_UI_PORT=3001 make start
```

---

## Run fallisce in DataIngestion

Controlla:

```text
configs/datasets.toml
```

Verifica:

- `source_uri` esiste;
- colonne configurate corrispondono al CSV;
- `timestamp_unit` è `millis`, `seconds` o `nanos`;
- `timeframe_seconds > 0`.

---

## Run fallisce in DataQualityGate

Controlla il dataset:

- timestamp non monotoni;
- OHLC inconsistenti;
- spread negativo;
- prezzi non positivi;
- gap eccessivi.

---

## Nessun artifact prodotto

Verifica se la run è `Completed`:

```bash
curl http://localhost:8080/api/runs/{run_id}
```

Se fallita:

```bash
cat logs/latest/api.log
```

---

## Ranking 404

Significa che non esiste:

```text
runs/artifacts/{run_id}/report.json
```

Cause tipiche:

- run non completata;
- `ReportGenerator` non eseguito;
- export fallito.

---

## Search state 404

Significa che non esiste:

```text
runs/checkpoints/{run_id}/search_state.latest.json
```

Cause:

- run non arrivata a `StrategyRunner`;
- errore prima della search;
- run molto breve completata prima del polling e file non scritto per errore.

---

## Recovery non funziona

Controlla:

```bash
curl http://localhost:8080/api/recoverable
```

Poi:

```bash
curl -X POST http://localhost:8080/api/runs/{run_id}/recover
```

Se fallisce:

- checkpoint mancante;
- checksum checkpoint non valido;
- catalogo SQLite non contiene la run;
- artifact/runs root diverso da quello usato prima.

Verifica:

```text
QS_RUNS_ROOT
runs/checkpoints/{run_id}/latest.checkpoint.json
```

---

## Creare debug bundle

Se non riesci a capire il problema:

```bash
make debug-bundle
```

Poi analizza:

```text
debug-bundles/debug-*.tar.gz
```

---

# Limiti attuali

## Parquet

Attualmente il sistema scrive:

```text
*.columns.json
```

non ancora:

```text
*.parquet
```

Il backend Parquet vero è previsto come evoluzione.

## Search

Il sistema ha runtime state, batch, intensification planner e checkpoint per candidate, ma la separazione perfetta `ParameterSearch` come unico scheduler può essere ulteriormente raffinata.

## Backtest

Il backtest supporta fills baseline, ma non ancora:

- margin;
- leverage;
- partial fills realistici;
- multi-symbol;
- session calendar;
- broker constraints complessi.

## Validation

Le validation sono baseline reali, ma non ancora versioni quantitative avanzate complete.

## Build Rust

Il progetto deve essere validato nel devcontainer con:

```bash
make full-check
make e2e
```

---

# Roadmap consigliata

## v0.1

- far passare `make full-check`;
- far passare `make e2e`;
- correggere eventuali errori Rust;
- taggare una prima release tecnica.

## v0.2

- backend Parquet vero;
- query API su columnar/Parquet;
- UI charts equity/drawdown;
- result tables paginated.

## v0.3

- search scheduler completamente separato;
- intensification runtime completo;
- controlled completion runtime;
- checkpoint batch-level nativo.

## v0.4

- backtest portfolio-grade;
- margin/leverage;
- multi-symbol;
- partial fills realistici.

## v0.5

- validation avanzata;
- block bootstrap;
- regime classifier;
- sensitivity surfaces;
- execution stress avanzato;
- parameter decay robusto.

---

# Comandi principali

```bash
make help
make dev
make shell
make doctor
make static-validate
make full-check
make e2e
make start
make stop
make status
make logs
make debug-bundle
make debug-compose
make ui-build
make prod-build
```

---

# Conclusione

Questo progetto è una base avanzata per una piattaforma di ricerca e validazione quantitativa.

È progettato per essere:

- modulare;
- estendibile;
- riproducibile;
- recuperabile dopo crash;
- osservabile;
- pronta per export live strutturato.

Prima dell'uso operativo reale, è necessario completare la validazione nel container:

```bash
make dev
make shell
make full-check
make e2e
```

e poi procedere con Parquet vero, hardening backtest e validation avanzata.
