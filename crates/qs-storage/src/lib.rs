pub mod backup;
pub mod catalog;
pub mod compaction;
pub mod recovery;
pub mod results;
pub mod retention;

pub use backup::*;
pub use catalog::*;
pub use compaction::*;
pub use recovery::*;
pub use results::*;
pub use retention::*;

use qs_core::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

#[derive(Clone, Debug)]
pub struct StorageLayout {
    pub root: PathBuf,
}

impl StorageLayout {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
    pub fn checkpoint_dir(&self, run_id: &RunId) -> PathBuf {
        self.root.join("checkpoints").join(&run_id.0)
    }
    pub fn checkpoint_latest(&self, run_id: &RunId) -> PathBuf {
        self.checkpoint_dir(run_id).join("latest.checkpoint.json")
    }
    pub fn metadata_path(&self, run_id: &RunId) -> PathBuf {
        self.root
            .join("metadata")
            .join(format!("{}.json", run_id.0))
    }
    pub fn results_dir(&self, run_id: &RunId) -> PathBuf {
        self.root.join("results").join(&run_id.0)
    }
    pub fn artifacts_dir(&self, run_id: &RunId) -> PathBuf {
        self.root.join("artifacts").join(&run_id.0)
    }
    pub fn ensure(&self, run_id: &RunId) -> Result<(), StorageError> {
        for d in [
            self.checkpoint_dir(run_id),
            self.root.join("metadata"),
            self.results_dir(run_id),
            self.artifacts_dir(run_id),
        ] {
            fs::create_dir_all(d).map_err(|e| StorageError::Message {
                code: "MKDIR_FAILED".into(),
                message: e.to_string(),
                retryable: true,
            })?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunCheckpoint {
    pub schema_version: u32,
    pub run_id: RunId,
    pub run_state: PersistentRunState,
    pub completed_components: Vec<ComponentId>,
    pub component_states: BTreeMap<ComponentId, String>,
    pub search_state: Option<serde_json::Value>,
    pub partial_results_index: serde_json::Value,
    pub ranking_state: serde_json::Value,
    pub rng_state: serde_json::Value,
    pub metadata: RunMetadata,
    pub created_at: i64,
    pub checksum: String,
}

impl RunCheckpoint {
    pub fn with_checksum(mut self) -> Result<Self, CheckpointError> {
        self.checksum.clear();
        let payload = serde_json::to_vec(&self).map_err(|e| CheckpointError::Message {
            code: "CHECKPOINT_SERIALIZE".into(),
            message: e.to_string(),
            retryable: false,
        })?;
        self.checksum = format!("sha256:{:x}", Sha256::digest(payload));
        Ok(self)
    }
    pub fn verify_checksum(&self) -> Result<(), CheckpointError> {
        let expected = self.checksum.clone();
        let actual = self.clone().with_checksum()?.checksum;
        if expected == actual {
            Ok(())
        } else {
            Err(CheckpointError::Message {
                code: "CHECKPOINT_CHECKSUM_MISMATCH".into(),
                message: format!("expected {expected}, actual {actual}"),
                retryable: false,
            })
        }
    }
}

pub struct AtomicCheckpointStore {
    layout: StorageLayout,
}
impl AtomicCheckpointStore {
    pub fn new(layout: StorageLayout) -> Self {
        Self { layout }
    }

    pub fn save_latest(&self, checkpoint: &RunCheckpoint) -> Result<(), CheckpointError> {
        let cp = checkpoint.clone().with_checksum()?;
        self.layout
            .ensure(&cp.run_id)
            .map_err(PipelineError::from)
            .map_err(|e| CheckpointError::Message {
                code: "CHECKPOINT_PREPARE".into(),
                message: e.to_string(),
                retryable: true,
            })?;
        let path = self.layout.checkpoint_latest(&cp.run_id);
        atomic_write_json(&path, &cp)
    }

    pub fn load_latest(&self, run_id: &RunId) -> Result<RunCheckpoint, CheckpointError> {
        let path = self.layout.checkpoint_latest(run_id);
        let mut f = File::open(&path).map_err(|e| CheckpointError::Message {
            code: "CHECKPOINT_OPEN".into(),
            message: e.to_string(),
            retryable: true,
        })?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf)
            .map_err(|e| CheckpointError::Message {
                code: "CHECKPOINT_READ".into(),
                message: e.to_string(),
                retryable: true,
            })?;
        let cp: RunCheckpoint =
            serde_json::from_slice(&buf).map_err(|e| CheckpointError::Message {
                code: "CHECKPOINT_PARSE".into(),
                message: e.to_string(),
                retryable: false,
            })?;
        cp.verify_checksum()?;
        Ok(cp)
    }
}

pub fn atomic_write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), CheckpointError> {
    let parent = path.parent().ok_or_else(|| CheckpointError::Message {
        code: "CHECKPOINT_NO_PARENT".into(),
        message: path.display().to_string(),
        retryable: false,
    })?;
    fs::create_dir_all(parent).map_err(|e| CheckpointError::Message {
        code: "CHECKPOINT_MKDIR".into(),
        message: e.to_string(),
        retryable: true,
    })?;
    let tmp = path.with_extension("tmp");
    {
        let mut f = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&tmp)
            .map_err(|e| CheckpointError::Message {
                code: "CHECKPOINT_TMP_OPEN".into(),
                message: e.to_string(),
                retryable: true,
            })?;
        let bytes = serde_json::to_vec_pretty(value).map_err(|e| CheckpointError::Message {
            code: "CHECKPOINT_SERIALIZE".into(),
            message: e.to_string(),
            retryable: false,
        })?;
        f.write_all(&bytes).map_err(|e| CheckpointError::Message {
            code: "CHECKPOINT_WRITE".into(),
            message: e.to_string(),
            retryable: true,
        })?;
        f.sync_all().map_err(|e| CheckpointError::Message {
            code: "CHECKPOINT_FSYNC_FILE".into(),
            message: e.to_string(),
            retryable: true,
        })?;
    }
    sync_dir(parent)?;
    fs::rename(&tmp, path).map_err(|e| CheckpointError::Message {
        code: "CHECKPOINT_RENAME".into(),
        message: e.to_string(),
        retryable: true,
    })?;
    sync_dir(parent)?;
    Ok(())
}

#[cfg(unix)]
fn sync_dir(path: &Path) -> Result<(), CheckpointError> {
    let dir = File::open(path).map_err(|e| CheckpointError::Message {
        code: "DIR_OPEN".into(),
        message: e.to_string(),
        retryable: true,
    })?;
    dir.sync_all().map_err(|e| CheckpointError::Message {
        code: "DIR_FSYNC".into(),
        message: e.to_string(),
        retryable: true,
    })
}

#[cfg(not(unix))]
fn sync_dir(_path: &Path) -> Result<(), CheckpointError> {
    Ok(())
}
