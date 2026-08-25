use crate::PipelineError;
use std::sync::Arc;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken as TokioCancellationToken;

#[derive(Clone, Default)]
pub struct CancellationToken { inner: TokioCancellationToken }
impl CancellationToken {
    pub fn cancel(&self) { self.inner.cancel(); }
    pub fn check_cancelled(&self) -> Result<(), PipelineError> { if self.inner.is_cancelled() { Err(PipelineError::Cancelled) } else { Ok(()) } }
}

#[derive(Clone, Default)]
pub struct PauseToken { state: Arc<parking_lot::RwLock<bool>>, notify: Arc<Notify> }
impl PauseToken {
    pub fn pause(&self) { *self.state.write() = true; }
    pub fn resume(&self) { *self.state.write() = false; self.notify.notify_waiters(); }
    pub fn is_paused(&self) -> bool { *self.state.read() }
    pub async fn wait_if_paused(&self) -> Result<(), PipelineError> {
        while self.is_paused() { self.notify.notified().await; }
        Ok(())
    }
}
