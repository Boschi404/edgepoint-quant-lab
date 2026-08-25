use crate::{ids::*, run_state::RunStatus, SerializableError};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::broadcast;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProgressEvent {
    pub schema_version: u32,
    pub run_id: RunId,
    pub stage: String,
    pub status: RunStatus,
    pub worker_id: Option<String>,
    pub current: u64,
    pub total: Option<u64>,
    pub percent: Option<f64>,
    pub best_score_so_far: Option<f64>,
    pub message: String,
    pub error: Option<SerializableError>,
    pub timestamp: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunProgressSnapshot { pub run_id: RunId, pub latest: Option<ProgressEvent>, pub sequence: u64 }

#[derive(Clone)]
pub struct ProgressSink {
    sender: broadcast::Sender<(u64, ProgressEvent)>,
    snapshot: Arc<RwLock<RunProgressSnapshot>>,
    sequence: Arc<std::sync::atomic::AtomicU64>,
}

impl ProgressSink {
    pub fn new(run_id: RunId, capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender, snapshot: Arc::new(RwLock::new(RunProgressSnapshot { run_id, latest: None, sequence: 0 })), sequence: Arc::new(std::sync::atomic::AtomicU64::new(0)) }
    }

    pub fn publish(&self, event: ProgressEvent) {
        let seq = self.sequence.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
        { let mut s = self.snapshot.write(); s.latest = Some(event.clone()); s.sequence = seq; }
        let _ = self.sender.send((seq, event));
    }

    pub fn subscribe(&self) -> broadcast::Receiver<(u64, ProgressEvent)> { self.sender.subscribe() }
    pub fn snapshot(&self) -> RunProgressSnapshot { self.snapshot.read().clone() }
}
