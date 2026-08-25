use qs_core::*;
use serde::Serialize;
use std::{fs::OpenOptions, io::Write, path::PathBuf};

/// Append-only JSONL result writer.
///
/// This is the durable baseline writer used before/alongside Parquet. Production deployments can
/// add a Parquet implementation behind the same calls. JSONL remains useful for recovery logs and
/// debugging because each line is independently parseable.
#[derive(Clone, Debug)]
pub struct JsonlResultStore {
    root: PathBuf,
}

impl JsonlResultStore {
    pub fn new(root: impl Into<PathBuf>) -> Self { Self { root: root.into() } }

    pub fn append_evaluation(&self, run_id: &RunId, result: &EvaluationResult) -> Result<(), StorageError> {
        self.append(run_id, "evaluations.jsonl", result)
    }

    pub fn append_trade(&self, run_id: &RunId, trade: &Trade) -> Result<(), StorageError> {
        self.append(run_id, "trades.jsonl", trade)
    }

    pub fn append_equity(&self, run_id: &RunId, point: &EquityPoint) -> Result<(), StorageError> {
        self.append(run_id, "equity.jsonl", point)
    }

    pub fn append_metric<T: Serialize>(&self, run_id: &RunId, metric: &T) -> Result<(), StorageError> {
        self.append(run_id, "metrics.jsonl", metric)
    }


    pub fn read_evaluations(&self, run_id: &RunId) -> Result<Vec<EvaluationResult>, StorageError> {
        self.read_jsonl(run_id, "evaluations.jsonl")
    }

    pub fn read_trades(&self, run_id: &RunId) -> Result<Vec<Trade>, StorageError> {
        self.read_jsonl(run_id, "trades.jsonl")
    }

    pub fn read_equity(&self, run_id: &RunId) -> Result<Vec<EquityPoint>, StorageError> {
        self.read_jsonl(run_id, "equity.jsonl")
    }

    fn read_jsonl<T>(&self, run_id: &RunId, file_name: &str) -> Result<Vec<T>, StorageError>
    where
        T: for<'de> serde::Deserialize<'de>,
    {
        let path = self.root.join("results").join(&run_id.0).join(file_name);
        let content = match std::fs::read_to_string(&path) {
            Ok(value) => value,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(err) => return Err(StorageError::Message { code: "RESULTS_READ".into(), message: err.to_string(), retryable: true }),
        };
        let mut out = Vec::new();
        for line in content.lines() {
            if line.trim().is_empty() { continue; }
            let value = serde_json::from_str(line).map_err(|e| StorageError::Message { code: "RESULTS_PARSE".into(), message: e.to_string(), retryable: false })?;
            out.push(value);
        }
        Ok(out)
    }

    fn append<T: Serialize>(&self, run_id: &RunId, file_name: &str, value: &T) -> Result<(), StorageError> {
        let dir = self.root.join("results").join(&run_id.0);
        std::fs::create_dir_all(&dir).map_err(|e| StorageError::Message { code: "RESULTS_MKDIR".into(), message: e.to_string(), retryable: true })?;
        let path = dir.join(file_name);
        let mut file = OpenOptions::new().create(true).append(true).open(&path).map_err(|e| StorageError::Message { code: "RESULTS_OPEN".into(), message: e.to_string(), retryable: true })?;
        let line = serde_json::to_string(value).map_err(|e| StorageError::Message { code: "RESULTS_SERIALIZE".into(), message: e.to_string(), retryable: false })?;
        file.write_all(line.as_bytes()).map_err(|e| StorageError::Message { code: "RESULTS_WRITE".into(), message: e.to_string(), retryable: true })?;
        file.write_all(b"\n").map_err(|e| StorageError::Message { code: "RESULTS_WRITE_NEWLINE".into(), message: e.to_string(), retryable: true })?;
        file.sync_data().map_err(|e| StorageError::Message { code: "RESULTS_FSYNC".into(), message: e.to_string(), retryable: true })?;
        Ok(())
    }
}
