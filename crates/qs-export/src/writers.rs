use crate::ReproducibilityManifest;
use qs_core::*;
use qs_storage::atomic_write_json;
use serde::{Deserialize, Serialize};
use std::{fs, io::Write, path::{Path, PathBuf}};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SelectedParameterExport {
    pub schema_version: u32,
    pub run_id: RunId,
    pub selected: Vec<SelectedParameter>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SelectedParameter {
    pub strategy_id: StrategyId,
    pub parameter_set_id: ParameterSetId,
    pub parameters: std::collections::BTreeMap<String, ParameterValue>,
    pub score: f64,
    pub risk_limits: RiskLimits,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RiskLimits {
    pub max_drawdown_observed: f64,
    pub max_consecutive_losses: u32,
}

pub struct LiveExportWriter { pub root: PathBuf }

impl LiveExportWriter {
    pub fn new(root: impl Into<PathBuf>) -> Self { Self { root: root.into() } }

    pub fn write_pack(&self, run_id: &RunId, manifest: &ReproducibilityManifest, selected: &SelectedParameterExport) -> Result<PathBuf, ExportError> {
        let dir = self.root.join("artifacts").join(&run_id.0).join("live_export");
        fs::create_dir_all(dir.join("python_bot_pack")).map_err(export_err("EXPORT_MKDIR"))?;
        fs::create_dir_all(dir.join("mt5_pack")).map_err(export_err("EXPORT_MKDIR"))?;

        atomic_write_json(&dir.join("manifest.json"), manifest).map_err(|e| ExportError::Message { code: "EXPORT_MANIFEST".into(), message: e.to_string(), retryable: true })?;
        atomic_write_json(&dir.join("selected_parameters.json"), selected).map_err(|e| ExportError::Message { code: "EXPORT_SELECTED".into(), message: e.to_string(), retryable: true })?;
        atomic_write_json(&dir.join("python_bot_pack").join("strategy_config.json"), selected).map_err(|e| ExportError::Message { code: "EXPORT_PY_CONFIG".into(), message: e.to_string(), retryable: true })?;
        write_mt5_set(&dir.join("mt5_pack").join("parameters.set"), selected)?;
        write_text(&dir.join("python_bot_pack").join("README.md"), PY_README)?;
        write_text(&dir.join("mt5_pack").join("README.md"), MT5_README)?;
        Ok(dir)
    }
}

fn write_mt5_set(path: &Path, selected: &SelectedParameterExport) -> Result<(), ExportError> {
    let mut file = std::fs::File::create(path).map_err(export_err("EXPORT_MT5_OPEN"))?;
    for item in &selected.selected {
        writeln!(file, "; strategy_id={}", item.strategy_id.0).map_err(export_err("EXPORT_MT5_WRITE"))?;
        writeln!(file, "; parameter_set_id={}", item.parameter_set_id.0).map_err(export_err("EXPORT_MT5_WRITE"))?;
        for (key, value) in &item.parameters {
            writeln!(file, "{}={}", key, param_to_mt5(value)).map_err(export_err("EXPORT_MT5_WRITE"))?;
        }
    }
    file.sync_all().map_err(export_err("EXPORT_MT5_FSYNC"))?;
    Ok(())
}

fn param_to_mt5(value: &ParameterValue) -> String {
    match value {
        ParameterValue::Int(v) => v.to_string(),
        ParameterValue::Float(v) => v.to_string(),
        ParameterValue::Bool(v) => if *v { "true".into() } else { "false".into() },
        ParameterValue::Enum(v) | ParameterValue::Text(v) => v.clone(),
    }
}

fn write_text(path: &Path, content: &str) -> Result<(), ExportError> {
    std::fs::write(path, content).map_err(export_err("EXPORT_TEXT_WRITE"))
}

fn export_err<E: std::fmt::Display>(code: &'static str) -> impl Fn(E) -> ExportError {
    move |e| ExportError::Message { code: code.into(), message: e.to_string(), retryable: true }
}

const PY_README: &str = "# Python bot pack\n\nUse `strategy_config.json` as the canonical machine-readable configuration. Validate manifest checksum and dataset provenance before live deployment.\n";
const MT5_README: &str = "# MT5 pack\n\n`parameters.set` is provided for EA inputs. `selected_parameters.json` remains the canonical export.\n";
