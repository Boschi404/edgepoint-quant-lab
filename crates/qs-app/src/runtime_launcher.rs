use crate::component_factory;
use qs_api::{RunHandle, RunLauncher, RunManager};
use qs_core::*;
use qs_data::normalizers::OhlcvColumnMapping;
use qs_data::{ConfiguredDataset, DataSource, NormalizationConfig};
use qs_orchestrator::PipelineOrchestrator;
use qs_search::{generate_budgeted, GenerationBudget, RuntimeSearchState};
use qs_storage::{
    AtomicCheckpointStore, CatalogRunRecord, JsonlResultStore, RunCatalog, StorageLayout,
};
use serde::Deserialize;
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Clone)]
pub struct AppRunLauncher {
    pub runs: RunManager,
    pub storage_root: PathBuf,
}

impl AppRunLauncher {
    pub fn new(runs: RunManager, storage_root: PathBuf) -> Self {
        Self { runs, storage_root }
    }
}

impl RunLauncher for AppRunLauncher {
    fn launch(&self, handle: RunHandle) {
        let runs = self.runs.clone();
        let storage_root = self.storage_root.clone();
        tokio::spawn(async move {
            let run_id = RunId(handle.summary.run_id.clone());
            let now = chrono::Utc::now().timestamp_millis();
            let layout = StorageLayout::new(storage_root.clone());
            if let Err(err) = layout.ensure(&run_id) {
                publish_runtime_error(&handle, "StoragePrepare", err.to_string());
                let _ = runs.set_state(&run_id.0, PersistentRunState::Failed);
                return;
            }

            if let Err(err) = persist_initial_catalog(&storage_root, &handle, now) {
                publish_runtime_error(&handle, "RunCatalog", err.to_string());
                let _ = runs.set_state(&run_id.0, PersistentRunState::Failed);
                return;
            }

            let registry = match component_factory::build_static_strategy_registry() {
                Ok(value) => value,
                Err(err) => {
                    publish_runtime_error(&handle, "StrategyRegistry", err.to_string());
                    let _ = runs.set_state(&run_id.0, PersistentRunState::Failed);
                    return;
                }
            };
            let components = component_factory::build_default_components(registry.clone());
            let selected_components = components.iter().map(|c| c.id()).collect::<Vec<_>>();
            let orchestrator = PipelineOrchestrator::new(components);
            let checkpoints = AtomicCheckpointStore::new(layout);
            let recovered_checkpoint = match checkpoints.load_latest(&run_id) {
                Ok(value) => Some(value),
                Err(_) => None,
            };
            let recovered_results =
                match JsonlResultStore::new(storage_root.clone()).read_evaluations(&run_id) {
                    Ok(value) => value,
                    Err(_) => Vec::new(),
                };
            let mut ctx = PipelineContext {
                run_id: Some(run_id.clone()),
                run_config: Some(RunConfig {
                    seed: 123456,
                    pipeline_version: handle.summary.pipeline_version.clone(),
                    selected_components,
                }),
                component_states: match &recovered_checkpoint {
                    Some(checkpoint) => ComponentStateMap {
                        completed: checkpoint.completed_components.iter().cloned().collect(),
                        running: None,
                    },
                    None => ComponentStateMap::default(),
                },
                datasets: Default::default(),
                parameter_spaces: registry.parameter_spaces(),
                candidate_sets: Default::default(),
                partial_results: recovered_results,
                progress: Some(handle.progress.clone()),
                cancellation: handle.cancellation.clone(),
                pause: handle.pause.clone(),
                metadata: match &recovered_checkpoint {
                    Some(checkpoint) => checkpoint.metadata.clone(),
                    None => RunMetadata {
                        created_at: now,
                        updated_at: now,
                        tags: Default::default(),
                    },
                },
                bag: Default::default(),
            };
            seed_context_from_config(&mut ctx, &handle, &storage_root);
            rebuild_candidate_sets_from_spaces(&mut ctx);
            if let Some(checkpoint) = recovered_checkpoint {
                if let Some(search_state) = checkpoint.search_state {
                    if serde_json::from_value::<RuntimeSearchState>(search_state.clone()).is_ok() {
                        ctx.bag
                            .insert("search_runtime_state".into(), search_state.clone());
                    }
                    ctx.bag.insert("search_state".into(), search_state);
                }
                ctx.bag
                    .insert("ranking_state".into(), checkpoint.ranking_state);
                publish_lifecycle(
                    &handle,
                    RunStatus::Running,
                    "checkpoint loaded; resuming run",
                );
            }

            publish_lifecycle(&handle, RunStatus::Running, "orchestrator started");
            let final_state = match orchestrator
                .run_with_checkpoints(&mut ctx, &checkpoints)
                .await
            {
                Ok(state) => state,
                Err(err) => {
                    publish_runtime_error(&handle, "Orchestrator", err.to_string());
                    PersistentRunState::Failed
                }
            };

            let status = match &final_state {
                PersistentRunState::Completed => RunStatus::Completed,
                PersistentRunState::Paused => RunStatus::Paused,
                PersistentRunState::Failed | PersistentRunState::Interrupted => RunStatus::Failed,
                PersistentRunState::Running => RunStatus::Running,
            };
            publish_lifecycle(
                &handle,
                status,
                &format!("run finished with state {:?}", final_state),
            );
            let _ = runs.set_state(&run_id.0, final_state.clone());
            let _ = persist_final_catalog(&storage_root, &run_id, final_state);
        });
    }
}

fn rebuild_candidate_sets_from_spaces(ctx: &mut PipelineContext) {
    let budget = match ctx
        .bag
        .get("parameter_generation_budget")
        .and_then(|value| value.as_u64())
    {
        Some(value) => value as usize,
        None => 64,
    };
    let seed = match ctx.run_config.as_ref().map(|config| config.seed) {
        Some(value) => value,
        None => 0,
    };
    for (strategy_id, space) in ctx.parameter_spaces.clone() {
        if ctx.candidate_sets.contains_key(&strategy_id) {
            continue;
        }
        match generate_budgeted(
            &space,
            GenerationBudget {
                max_candidates: budget,
            },
            seed,
        ) {
            Ok(candidates) => {
                ctx.candidate_sets.insert(strategy_id, candidates);
            }
            Err(_) => {}
        }
    }
}

fn seed_context_from_config(ctx: &mut PipelineContext, handle: &RunHandle, storage_root: &PathBuf) {
    let path = match std::env::var("QS_DATASETS_CONFIG") {
        Ok(value) => PathBuf::from(value),
        Err(_) => PathBuf::from("configs/datasets.toml"),
    };
    match load_dataset_configs(&path) {
        Ok(configs) => match serde_json::to_value(configs) {
            Ok(value) => {
                ctx.bag.insert("dataset_configs".into(), value);
            }
            Err(err) => publish_runtime_error(handle, "Config", err.to_string()),
        },
        Err(err) => publish_runtime_error(handle, "Config", err),
    }
    ctx.bag
        .insert("parameter_generation_budget".into(), serde_json::json!(64));
    ctx.bag.insert(
        "storage_root".into(),
        serde_json::json!(storage_root.to_string_lossy().to_string()),
    );
}

#[derive(Debug, Deserialize)]
struct DatasetsToml {
    datasets: Vec<DatasetToml>,
}

#[derive(Debug, Deserialize)]
struct DatasetToml {
    dataset_id: String,
    source_uri: String,
    format_hint: Option<String>,
    timezone: String,
    timestamp_unit: String,
    symbol: String,
    timeframe_name: String,
    timeframe_seconds: u64,
    columns: DatasetColumnsToml,
}

#[derive(Debug, Deserialize)]
struct DatasetColumnsToml {
    timestamp: String,
    open: String,
    high: String,
    low: String,
    close: String,
    volume: Option<String>,
    spread: Option<String>,
}

fn load_dataset_configs(path: &PathBuf) -> Result<Vec<ConfiguredDataset>, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    let parsed: DatasetsToml =
        toml::from_str(&text).map_err(|e| format!("failed to parse {}: {e}", path.display()))?;
    if parsed.datasets.is_empty() {
        return Err("dataset config must contain at least one dataset".into());
    }
    let mut out = Vec::new();
    for d in parsed.datasets {
        validate_dataset_config(&d)?;
        let mapping = OhlcvColumnMapping {
            timestamp: d.columns.timestamp,
            open: d.columns.open,
            high: d.columns.high,
            low: d.columns.low,
            close: d.columns.close,
            volume: d.columns.volume,
            spread: d.columns.spread,
            instrument: Instrument {
                symbol: d.symbol,
                venue: None,
                asset_class: None,
            },
            timeframe: Timeframe {
                name: d.timeframe_name,
                seconds: d.timeframe_seconds,
            },
            dataset_id: DatasetId(d.dataset_id.clone()),
        };
        out.push(ConfiguredDataset {
            dataset_id: DatasetId(d.dataset_id),
            source: DataSource {
                uri: d.source_uri,
                format_hint: d.format_hint,
            },
            normalization: NormalizationConfig {
                timezone: d.timezone,
                timestamp_unit: d.timestamp_unit,
            },
            mapping,
        });
    }
    Ok(out)
}

fn validate_dataset_config(config: &DatasetToml) -> Result<(), String> {
    if config.dataset_id.trim().is_empty() {
        return Err("dataset_id cannot be empty".into());
    }
    if config.source_uri.trim().is_empty() {
        return Err(format!(
            "dataset {} source_uri cannot be empty",
            config.dataset_id
        ));
    }
    if config.timezone.trim().is_empty() {
        return Err(format!(
            "dataset {} timezone cannot be empty",
            config.dataset_id
        ));
    }
    if config.timestamp_unit != "millis"
        && config.timestamp_unit != "seconds"
        && config.timestamp_unit != "nanos"
    {
        return Err(format!(
            "dataset {} unsupported timestamp_unit {}",
            config.dataset_id, config.timestamp_unit
        ));
    }
    if config.timeframe_seconds == 0 {
        return Err(format!(
            "dataset {} timeframe_seconds must be positive",
            config.dataset_id
        ));
    }
    for (name, value) in [
        ("timestamp", &config.columns.timestamp),
        ("open", &config.columns.open),
        ("high", &config.columns.high),
        ("low", &config.columns.low),
        ("close", &config.columns.close),
    ] {
        if value.trim().is_empty() {
            return Err(format!(
                "dataset {} column {name} cannot be empty",
                config.dataset_id
            ));
        }
    }
    Ok(())
}

fn persist_initial_catalog(
    root: &PathBuf,
    handle: &RunHandle,
    now: i64,
) -> Result<(), StorageError> {
    let catalog = RunCatalog::open(&root.join("catalog").join("runs.sqlite"))?;
    catalog.upsert_run(&CatalogRunRecord {
        run_id: RunId(handle.summary.run_id.clone()),
        state: PersistentRunState::Running,
        created_at: now,
        updated_at: now,
        pipeline_version: handle.summary.pipeline_version.clone(),
        seed: 123456,
        metadata: RunMetadata {
            created_at: now,
            updated_at: now,
            tags: BTreeMap::new(),
        },
    })
}

fn persist_final_catalog(
    root: &PathBuf,
    run_id: &RunId,
    state: PersistentRunState,
) -> Result<(), StorageError> {
    let catalog = RunCatalog::open(&root.join("catalog").join("runs.sqlite"))?;
    catalog.set_state(run_id, state, chrono::Utc::now().timestamp_millis())
}

fn publish_lifecycle(handle: &RunHandle, status: RunStatus, message: &str) {
    handle.progress.publish(ProgressEvent {
        schema_version: 1,
        run_id: RunId(handle.summary.run_id.clone()),
        stage: "RunSupervisor".into(),
        status,
        worker_id: None,
        current: 0,
        total: None,
        percent: None,
        best_score_so_far: None,
        message: message.into(),
        error: None,
        timestamp: chrono::Utc::now().timestamp_millis(),
    });
}

fn publish_runtime_error(handle: &RunHandle, stage: &str, message: String) {
    handle.progress.publish(ProgressEvent {
        schema_version: 1,
        run_id: RunId(handle.summary.run_id.clone()),
        stage: stage.into(),
        status: RunStatus::Failed,
        worker_id: None,
        current: 0,
        total: None,
        percent: None,
        best_score_so_far: None,
        message: message.clone(),
        error: Some(SerializableError {
            code: "RUNTIME_ERROR".into(),
            category: ErrorCategory::Internal,
            message,
            component_id: None,
            strategy_id: None,
            parameter_set_id: None,
            retryable: false,
            timestamp: chrono::Utc::now().timestamp_millis(),
        }),
        timestamp: chrono::Utc::now().timestamp_millis(),
    });
}
