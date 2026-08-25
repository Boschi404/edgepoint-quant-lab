use qs_core::*;
use serde::{Deserialize, Serialize};
use std::{fs, path::{Path, PathBuf}};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BackupManifest {
    pub schema_version: u32,
    pub run_id: RunId,
    pub created_at: i64,
    pub files: Vec<BackupFile>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BackupFile {
    pub relative_path: String,
    pub bytes: u64,
}

#[derive(Clone, Debug)]
pub struct RunBackupService {
    pub root: PathBuf,
}

impl RunBackupService {
    pub fn new(root: impl Into<PathBuf>) -> Self { Self { root: root.into() } }

    pub fn build_manifest(&self, run_id: &RunId) -> Result<BackupManifest, StorageError> {
        let mut files = Vec::new();
        for subdir in ["metadata", "checkpoints", "results", "artifacts"] {
            let base = self.root.join(subdir);
            let target = if subdir == "metadata" { base.join(format!("{}.json", run_id.0)) } else { base.join(&run_id.0) };
            collect_files(&self.root, &target, &mut files)?;
        }
        Ok(BackupManifest { schema_version: 1, run_id: run_id.clone(), created_at: chrono::Utc::now().timestamp_millis(), files })
    }

    pub fn write_manifest(&self, run_id: &RunId) -> Result<PathBuf, StorageError> {
        let manifest = self.build_manifest(run_id)?;
        let path = self.root.join("artifacts").join(&run_id.0).join("backup_manifest.json");
        crate::atomic_write_json(&path, &manifest).map_err(|e| StorageError::Message { code: "BACKUP_MANIFEST_WRITE".into(), message: e.to_string(), retryable: true })?;
        Ok(path)
    }
}

fn collect_files(root: &Path, path: &Path, out: &mut Vec<BackupFile>) -> Result<(), StorageError> {
    if !path.exists() { return Ok(()); }
    if path.is_file() {
        let metadata = fs::metadata(path).map_err(storage_err("BACKUP_METADATA"))?;
        let relative = path.strip_prefix(root).map_err(|e| StorageError::Message { code: "BACKUP_STRIP_PREFIX".into(), message: e.to_string(), retryable: false })?;
        out.push(BackupFile { relative_path: relative.to_string_lossy().to_string(), bytes: metadata.len() });
        return Ok(());
    }
    for entry in fs::read_dir(path).map_err(storage_err("BACKUP_READ_DIR"))? {
        let entry = entry.map_err(storage_err("BACKUP_DIR_ENTRY"))?;
        collect_files(root, &entry.path(), out)?;
    }
    Ok(())
}

fn storage_err<E: std::fmt::Display>(code: &'static str) -> impl Fn(E) -> StorageError {
    move |e| StorageError::Message { code: code.into(), message: e.to_string(), retryable: true }
}
