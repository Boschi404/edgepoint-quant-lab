pub mod writers;
pub use writers::*;

use async_trait::async_trait;
use qs_core::*;
use qs_storage::{atomic_write_json, JsonlToColumnarCompactor, RunBackupService};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReproducibilityManifest {
    pub schema_version: u32,
    pub run_id: RunId,
    pub created_at: i64,
    pub pipeline_version: String,
    pub components: Vec<ComponentManifestEntry>,
    pub strategies: Vec<StrategyManifestEntry>,
    pub datasets: Vec<DatasetManifestEntry>,
    pub seed: u64,
    pub metrics: serde_json::Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ComponentManifestEntry {
    pub id: ComponentId,
    pub version: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StrategyManifestEntry {
    pub strategy_id: StrategyId,
    pub strategy_version: String,
    pub plugin_checksum: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DatasetManifestEntry {
    pub dataset_id: DatasetId,
    pub checksum: Option<String>,
    pub normalization_version: String,
    pub quality_report_id: Option<String>,
}

pub struct LiveExportComponent;
#[async_trait]
impl PipelineComponent for LiveExportComponent {
    fn id(&self) -> ComponentId {
        ComponentId("LiveExport".into())
    }
    fn name(&self) -> &'static str {
        "LiveExport"
    }
    fn version(&self) -> ComponentVersion {
        ComponentVersion {
            semver: "0.1.0".into(),
        }
    }
    fn input_contract(&self) -> Vec<DataContract> {
        vec![DataContract::RankingResults]
    }
    fn output_contract(&self) -> Vec<DataContract> {
        vec![DataContract::LiveExportArtifacts]
    }
    fn failure_policy(&self) -> FailurePolicy {
        FailurePolicy::AbortRun
    }
    async fn execute(&self, ctx: &mut PipelineContext) -> Result<ComponentOutcome, PipelineError> {
        let run_id = ctx.run_id.clone().ok_or_else(|| PipelineError::Invariant {
            message: "run_id missing".into(),
        })?;
        let root = match ctx
            .bag
            .get("storage_root")
            .and_then(|v| v.as_str())
            .map(PathBuf::from)
        {
            Some(value) => value,
            None => PathBuf::from("./runs"),
        };
        let mut results = ctx.partial_results.clone();
        results.sort_by(|a, b| {
            let av = a.metrics.classical.expectancy;
            let bv = b.metrics.classical.expectancy;
            match bv.partial_cmp(&av) {
                Some(ordering) => ordering,
                None => std::cmp::Ordering::Equal,
            }
        });
        let mut selected = Vec::new();
        for result in results.iter().take(3) {
            if let Some(params) = find_params(ctx, &result.strategy_id, &result.parameter_set_id) {
                selected.push(SelectedParameter {
                    strategy_id: result.strategy_id.clone(),
                    parameter_set_id: result.parameter_set_id.clone(),
                    parameters: params.values.clone(),
                    score: result.metrics.classical.expectancy,
                    risk_limits: RiskLimits {
                        max_drawdown_observed: result.metrics.classical.max_drawdown,
                        max_consecutive_losses: result.metrics.classical.max_consecutive_losses,
                    },
                });
            }
        }
        let export = SelectedParameterExport {
            schema_version: 1,
            run_id: run_id.clone(),
            selected,
        };
        let pipeline_version = match ctx.run_config.as_ref().map(|c| c.pipeline_version.clone()) {
            Some(value) => value,
            None => "unknown".into(),
        };
        let seed = match ctx.run_config.as_ref().map(|c| c.seed) {
            Some(value) => value,
            None => 0,
        };
        let components = match ctx.run_config.as_ref() {
            Some(config) => config
                .selected_components
                .iter()
                .cloned()
                .map(|id| ComponentManifestEntry {
                    id,
                    version: "0.1.0".into(),
                })
                .collect(),
            None => Vec::new(),
        };
        let datasets = ctx
            .datasets
            .values()
            .map(|dataset| DatasetManifestEntry {
                dataset_id: dataset.dataset_id.clone(),
                checksum: dataset.metadata.checksum.clone(),
                normalization_version: dataset.metadata.normalization_version.clone(),
                quality_report_id: dataset
                    .quality
                    .as_ref()
                    .map(|_| format!("quality_{}", dataset.dataset_id.0)),
            })
            .collect();
        let strategies = ctx
            .parameter_spaces
            .keys()
            .cloned()
            .map(|strategy_id| StrategyManifestEntry {
                strategy_id,
                strategy_version: "unknown".into(),
                plugin_checksum: None,
            })
            .collect();
        let manifest = ReproducibilityManifest {
            schema_version: 1,
            run_id: run_id.clone(),
            created_at: chrono::Utc::now().timestamp_millis(),
            pipeline_version,
            components,
            strategies,
            datasets,
            seed,
            metrics: match ctx.bag.get("ranking_state").cloned() {
                Some(value) => value,
                None => serde_json::json!({}),
            },
        };
        let writer = LiveExportWriter::new(root.clone());
        let dir = writer.write_pack(&run_id, &manifest, &export)?;
        let compaction = JsonlToColumnarCompactor::new(root);
        match compaction.compact_columnar_json(&run_id) {
            Ok(manifest) => {
                ctx.bag.insert(
                    "compaction_manifest".into(),
                    serde_json::to_value(manifest).map_err(|e| {
                        PipelineError::Export(ExportError::Message {
                            code: "COMPACTION_MANIFEST_SERIALIZE".into(),
                            message: e.to_string(),
                            retryable: false,
                        })
                    })?,
                );
            }
            Err(err) => return Err(PipelineError::Storage(err)),
        }
        ctx.bag.insert(
            "live_export_dir".into(),
            serde_json::json!(dir.to_string_lossy().to_string()),
        );
        Ok(ComponentOutcome {
            message: "live export artifacts written".into(),
        })
    }
}

fn find_params<'a>(
    ctx: &'a PipelineContext,
    strategy_id: &StrategyId,
    parameter_set_id: &ParameterSetId,
) -> Option<&'a ParameterSet> {
    ctx.candidate_sets
        .get(strategy_id)?
        .iter()
        .find(|candidate| &candidate.id == parameter_set_id)
}

pub struct ReportGeneratorComponent;
#[async_trait]
impl PipelineComponent for ReportGeneratorComponent {
    fn id(&self) -> ComponentId {
        ComponentId("ReportGenerator".into())
    }
    fn name(&self) -> &'static str {
        "ReportGenerator"
    }
    fn version(&self) -> ComponentVersion {
        ComponentVersion {
            semver: "0.1.0".into(),
        }
    }
    fn input_contract(&self) -> Vec<DataContract> {
        vec![DataContract::RankingResults]
    }
    fn output_contract(&self) -> Vec<DataContract> {
        vec![DataContract::ReportArtifacts]
    }
    fn failure_policy(&self) -> FailurePolicy {
        FailurePolicy::SkipComponent
    }
    async fn execute(&self, ctx: &mut PipelineContext) -> Result<ComponentOutcome, PipelineError> {
        let run_id = ctx.run_id.clone().ok_or_else(|| PipelineError::Invariant {
            message: "run_id missing".into(),
        })?;
        let root = match ctx
            .bag
            .get("storage_root")
            .and_then(|v| v.as_str())
            .map(PathBuf::from)
        {
            Some(value) => value,
            None => PathBuf::from("./runs"),
        };
        let backup_path = RunBackupService::new(root.clone())
            .write_manifest(&run_id)
            .map_err(PipelineError::Storage)?;
        let summary = serde_json::json!({
            "schema_version": 1,
            "run_id": run_id.0.clone(),
            "evaluations": ctx.partial_results.len(),
            "ranking_state": ctx.bag.get("ranking_state").cloned(),
            "live_export_dir": ctx.bag.get("live_export_dir").cloned(),
            "backup_manifest": backup_path.to_string_lossy().to_string(),
            "walk_forward_reports": ctx.bag.get("walk_forward_reports").cloned(),
            "monte_carlo_reports": ctx.bag.get("monte_carlo_reports").cloned(),
            "sensitivity_reports": ctx.bag.get("sensitivity_reports").cloned(),
            "regime_reports": ctx.bag.get("regime_reports").cloned(),
            "execution_stress_reports": ctx.bag.get("execution_stress_reports").cloned(),
            "parameter_decay_reports": ctx.bag.get("parameter_decay_reports").cloned(),
            "compaction_manifest": ctx.bag.get("compaction_manifest").cloned()
        });
        let report_path = root.join("artifacts").join(&run_id.0).join("report.json");
        atomic_write_json(&report_path, &summary).map_err(|e| {
            PipelineError::Export(ExportError::Message {
                code: "REPORT_WRITE".into(),
                message: e.to_string(),
                retryable: true,
            })
        })?;
        ctx.bag.insert("report_summary".into(), summary);
        ctx.bag.insert(
            "report_path".into(),
            serde_json::json!(report_path.to_string_lossy().to_string()),
        );
        Ok(ComponentOutcome {
            message: "report summary generated and written".into(),
        })
    }
}
