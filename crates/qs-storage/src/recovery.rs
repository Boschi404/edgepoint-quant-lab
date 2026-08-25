use crate::{AtomicCheckpointStore, CatalogRunRecord, RunCatalog};
use qs_core::*;

pub struct RecoveryService {
    pub checkpoint_store: AtomicCheckpointStore,
}

impl RecoveryService {
    pub fn mark_interrupted_on_startup(&self, catalog: &RunCatalog) -> Result<usize, StorageError> {
        catalog.mark_running_as_interrupted(chrono::Utc::now().timestamp_millis())
    }

    pub fn verify_recoverable(&self, run_id: &RunId) -> Result<bool, CheckpointError> {
        match self.checkpoint_store.load_latest(run_id) {
            Ok(_) => Ok(true),
            Err(e) => Err(e),
        }
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct RecoverableRun {
    pub run_id: RunId,
    pub checkpoint_valid: bool,
}

impl RecoveryService {
    pub fn list_recoverable(&self, runs: &[CatalogRunRecord]) -> Vec<RecoverableRun> {
        let mut out = Vec::new();
        for run in runs {
            if matches!(
                run.state,
                PersistentRunState::Interrupted | PersistentRunState::Paused
            ) {
                let checkpoint_valid = match self.verify_recoverable(&run.run_id) {
                    Ok(value) => value,
                    Err(_) => false,
                };
                out.push(RecoverableRun {
                    run_id: run.run_id.clone(),
                    checkpoint_valid,
                });
            }
        }
        out
    }
}
