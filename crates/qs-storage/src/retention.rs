use qs_core::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RetentionPolicy {
    pub keep_metadata_forever: bool,
    pub keep_best_artifacts: bool,
    pub keep_full_trades_for_top_n: Option<usize>,
    pub delete_failed_after_days: Option<u32>,
    pub compact_after_days: Option<u32>,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            keep_metadata_forever: true,
            keep_best_artifacts: true,
            keep_full_trades_for_top_n: Some(50),
            delete_failed_after_days: Some(30),
            compact_after_days: Some(90),
        }
    }
}

#[derive(Clone, Debug)]
pub struct RetentionPlanner {
    pub root: PathBuf,
    pub policy: RetentionPolicy,
}

impl RetentionPlanner {
    pub fn plan(&self) -> Result<Vec<RetentionAction>, StorageError> {
        // Production implementation should inspect catalog states and artifact sizes.
        // Kept explicit so destructive deletion is never hidden in a helper.
        Ok(Vec::new())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum RetentionAction {
    DeleteArtifact { path: String },
    CompactResults { run_id: RunId },
    DeleteFailedRun { run_id: RunId },
}
