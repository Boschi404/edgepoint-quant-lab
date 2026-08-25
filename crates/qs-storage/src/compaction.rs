use qs_core::*;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::{Path, PathBuf}};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompactionManifest {
    pub schema_version: u32,
    pub run_id: RunId,
    pub created_at: i64,
    pub source_files: Vec<String>,
    pub output_files: Vec<String>,
    pub status: CompactionStatus,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CompactionStatus {
    Planned,
    Completed,
    Failed,
}

#[derive(Clone, Debug)]
pub struct JsonlToColumnarCompactor {
    pub root: PathBuf,
}

impl JsonlToColumnarCompactor {
    pub fn new(root: impl Into<PathBuf>) -> Self { Self { root: root.into() } }

    pub fn compact_columnar_json(&self, run_id: &RunId) -> Result<CompactionManifest, StorageError> {
        let results_dir = self.root.join("results").join(&run_id.0);
        fs::create_dir_all(&results_dir).map_err(storage_err("COMPACTION_MKDIR", true))?;

        let jobs = [
            ("evaluations.jsonl", "evaluations.columns.json"),
            ("trades.jsonl", "trades.columns.json"),
            ("equity.jsonl", "equity.columns.json"),
            ("metrics.jsonl", "metrics.columns.json"),
        ];

        let mut outputs = Vec::new();
        let mut notes = Vec::new();
        for (source, output) in jobs.iter() {
            let source_path = results_dir.join(source);
            let output_path = results_dir.join(output);
            match compact_one_file(&source_path, &output_path) {
                Ok(row_count) => {
                    outputs.push((*output).to_owned());
                    notes.push(format!("{source}: compacted {row_count} rows"));
                }
                Err(err) if *source == "trades.jsonl" || *source == "equity.jsonl" => {
                    notes.push(format!("{source}: optional result file unavailable or empty: {err}"));
                }
                Err(err) => return Err(err),
            }
        }

        let manifest = CompactionManifest {
            schema_version: 1,
            run_id: run_id.clone(),
            created_at: chrono::Utc::now().timestamp_millis(),
            source_files: jobs.iter().map(|(source, _)| (*source).to_owned()).collect(),
            output_files: outputs,
            status: CompactionStatus::Completed,
            notes,
        };
        let path = results_dir.join("compaction_manifest.json");
        crate::atomic_write_json(&path, &manifest).map_err(|e| StorageError::Message { code: "COMPACTION_MANIFEST_WRITE".into(), message: e.to_string(), retryable: true })?;
        Ok(manifest)
    }

    pub fn write_planned_manifest(&self, run_id: &RunId) -> Result<CompactionManifest, StorageError> {
        let results_dir = self.root.join("results").join(&run_id.0);
        fs::create_dir_all(&results_dir).map_err(storage_err("COMPACTION_MKDIR", true))?;
        let manifest = CompactionManifest {
            schema_version: 1,
            run_id: run_id.clone(),
            created_at: chrono::Utc::now().timestamp_millis(),
            source_files: vec!["evaluations.jsonl".into(), "trades.jsonl".into(), "equity.jsonl".into(), "metrics.jsonl".into()],
            output_files: vec!["evaluations.columns.json".into(), "trades.columns.json".into(), "equity.columns.json".into(), "metrics.columns.json".into()],
            status: CompactionStatus::Planned,
            notes: vec!["Columnar JSON compaction was planned but not executed".into()],
        };
        let path = results_dir.join("compaction_manifest.json");
        crate::atomic_write_json(&path, &manifest).map_err(|e| StorageError::Message { code: "COMPACTION_MANIFEST_WRITE".into(), message: e.to_string(), retryable: true })?;
        Ok(manifest)
    }
}

fn compact_one_file(source_path: &Path, output_path: &Path) -> Result<usize, StorageError> {
    let content = fs::read_to_string(source_path).map_err(storage_err("COMPACTION_READ_SOURCE", true))?;
    let mut columns: BTreeMap<String, Vec<serde_json::Value>> = BTreeMap::new();
    let mut row_count = 0usize;
    for line in content.lines() {
        if line.trim().is_empty() { continue; }
        let value: serde_json::Value = serde_json::from_str(line).map_err(storage_err("COMPACTION_PARSE_JSONL", false))?;
        let flattened = flatten_json(&value);
        for key in flattened.keys() {
            if !columns.contains_key(key) {
                columns.insert(key.clone(), vec![serde_json::Value::Null; row_count]);
            }
        }
        for (key, values) in columns.iter_mut() {
            match flattened.get(key) {
                Some(value) => values.push(value.clone()),
                None => values.push(serde_json::Value::Null),
            }
        }
        row_count += 1;
    }
    if row_count == 0 {
        return Err(StorageError::Message { code: "COMPACTION_EMPTY_SOURCE".into(), message: source_path.display().to_string(), retryable: false });
    }
    let output = serde_json::json!({
        "schema_version": 1,
        "row_count": row_count,
        "columns": columns
    });
    crate::atomic_write_json(output_path, &output).map_err(|e| StorageError::Message { code: "COMPACTION_WRITE_OUTPUT".into(), message: e.to_string(), retryable: true })?;
    Ok(row_count)
}

fn flatten_json(value: &serde_json::Value) -> BTreeMap<String, serde_json::Value> {
    let mut out = BTreeMap::new();
    flatten_into("", value, &mut out);
    out
}

fn flatten_into(prefix: &str, value: &serde_json::Value, out: &mut BTreeMap<String, serde_json::Value>) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, child) in map {
                let next = if prefix.is_empty() { key.clone() } else { format!("{prefix}.{key}") };
                flatten_into(&next, child, out);
            }
        }
        serde_json::Value::Array(_) => {
            out.insert(prefix.to_owned(), value.clone());
        }
        _ => {
            out.insert(prefix.to_owned(), value.clone());
        }
    }
}

fn storage_err<E: std::fmt::Display>(code: &'static str, retryable: bool) -> impl Fn(E) -> StorageError {
    move |e| StorageError::Message { code: code.into(), message: e.to_string(), retryable }
}
