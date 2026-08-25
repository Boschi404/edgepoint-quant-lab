use axum::{
    body::Body,
    extract::{Path, State, WebSocketUpgrade},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use axum::extract::ws::Message;
use futures_util::{SinkExt, StreamExt};
use parking_lot::RwLock;
use qs_core::*;
use qs_storage::{AtomicCheckpointStore, StorageLayout};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::{Component, PathBuf}, sync::Arc};
use tower_http::services::ServeDir;

pub trait RunLauncher: Send + Sync {
    fn launch(&self, handle: RunHandle);
}

#[derive(Clone)]
pub struct ApiState {
    pub runs: RunManager,
    pub launcher: Option<Arc<dyn RunLauncher>>,
    pub storage_root: PathBuf,
}

impl ApiState {
    pub fn new(runs: RunManager, launcher: Option<Arc<dyn RunLauncher>>, storage_root: PathBuf) -> Self { Self { runs, launcher, storage_root } }
}

impl Default for ApiState {
    fn default() -> Self { Self { runs: RunManager::default(), launcher: None, storage_root: PathBuf::from("./runs") } }
}

pub fn router(state: ApiState) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/runs", get(list_runs).post(create_run))
        .route("/api/runs/:run_id", get(get_run))
        .route("/api/runs/:run_id/pause", post(pause_run))
        .route("/api/runs/:run_id/resume", post(resume_run))
        .route("/api/runs/:run_id/cancel", post(cancel_run))
        .route("/api/recoverable", get(list_recoverable_runs))
        .route("/api/runs/:run_id/recover", post(recover_run))
        .route("/api/runs/:run_id/snapshot", get(run_snapshot))
        .route("/api/runs/:run_id/ranking", get(run_ranking))
        .route("/api/runs/:run_id/search-state", get(run_search_state))
        .route("/api/runs/:run_id/validation", get(run_validation))
        .route("/api/runs/:run_id/artifacts", get(list_artifacts))
        .route("/api/runs/:run_id/artifacts/*artifact_path", get(get_artifact))
        .route("/api/runs/:run_id/results/evaluations", get(run_evaluations))
        .route("/api/runs/:run_id/results/trades", get(run_trades))
        .route("/api/runs/:run_id/results/equity", get(run_equity))
        .route("/api/runs/:run_id/results/metrics", get(run_metrics))
        .route("/api/ws/runs/:run_id", get(ws_run))
        .fallback_service(ServeDir::new("ui/dist"))
        .with_state(state)
}

async fn health() -> &'static str { "ok" }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WsEnvelope<T> {
    pub schema_version: u32,
    pub message_type: String,
    pub run_id: Option<RunId>,
    pub sequence: u64,
    pub payload: T,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunSummary {
    pub run_id: String,
    pub state: PersistentRunState,
    pub created_at: i64,
    pub updated_at: i64,
    pub pipeline_version: String,
}

#[derive(Clone)]
pub struct RunHandle {
    pub summary: RunSummary,
    pub progress: ProgressSink,
    pub pause: PauseToken,
    pub cancellation: CancellationToken,
}

#[derive(Clone, Default)]
pub struct RunManager {
    inner: Arc<RwLock<BTreeMap<String, RunHandle>>>,
}

impl RunManager {
    pub fn create_pending(&self) -> RunHandle {
        let run_id = format!("run_{}", uuid::Uuid::new_v4().simple());
        self.create_handle(run_id, "run created and awaiting orchestrator assignment")
    }

    pub fn create_recovered(&self, run_id: String) -> RunHandle {
        self.create_handle(run_id, "recoverable run queued for resume from checkpoint")
    }

    fn create_handle(&self, run_id: String, message: &str) -> RunHandle {
        let now = chrono::Utc::now().timestamp_millis();
        let progress = ProgressSink::new(RunId(run_id.clone()), 4096);
        let handle = RunHandle {
            summary: RunSummary { run_id: run_id.clone(), state: PersistentRunState::Running, created_at: now, updated_at: now, pipeline_version: "0.1.0".into() },
            progress: progress.clone(),
            pause: PauseToken::default(),
            cancellation: CancellationToken::default(),
        };
        progress.publish(ProgressEvent {
            schema_version: 1,
            run_id: RunId(run_id.clone()),
            stage: "RunLifecycle".into(),
            status: RunStatus::Pending,
            worker_id: None,
            current: 0,
            total: None,
            percent: Some(0.0),
            best_score_so_far: None,
            message: message.into(),
            error: None,
            timestamp: now,
        });
        self.inner.write().insert(run_id, handle.clone());
        handle
    }

    pub fn list(&self) -> Vec<RunSummary> { self.inner.read().values().map(|h| h.summary.clone()).collect() }
    pub fn get(&self, run_id: &str) -> Option<RunHandle> { self.inner.read().get(run_id).cloned() }

    pub fn set_state(&self, run_id: &str, state: PersistentRunState) -> Option<RunHandle> {
        let mut guard = self.inner.write();
        let handle = guard.get_mut(run_id)?;
        handle.summary.state = state.clone();
        handle.summary.updated_at = chrono::Utc::now().timestamp_millis();
        Some(handle.clone())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecoverableRunSummary {
    pub run_id: String,
    pub state: PersistentRunState,
}

async fn list_recoverable_runs(State(state): State<ApiState>) -> Json<Vec<RecoverableRunSummary>> {
    let catalog_path = state.storage_root.join("catalog").join("runs.sqlite");
    let from_catalog = qs_storage::RunCatalog::open(&catalog_path)
        .and_then(|catalog| catalog.list_runs(500));
    let out = match from_catalog {
        Ok(records) => records.into_iter()
            .filter(|run| matches!(run.state, PersistentRunState::Interrupted | PersistentRunState::Paused))
            .map(|run| RecoverableRunSummary { run_id: run.run_id.0, state: run.state })
            .collect(),
        Err(_) => state.runs.list().into_iter()
            .filter(|run| matches!(run.state, PersistentRunState::Interrupted | PersistentRunState::Paused))
            .map(|run| RecoverableRunSummary { run_id: run.run_id, state: run.state })
            .collect(),
    };
    Json(out)
}

async fn list_runs(State(state): State<ApiState>) -> Json<Vec<RunSummary>> { Json(state.runs.list()) }

async fn create_run(State(state): State<ApiState>) -> Json<RunSummary> {
    let handle = state.runs.create_pending();
    if let Some(launcher) = &state.launcher {
        launcher.launch(handle.clone());
    }
    Json(handle.summary)
}

async fn get_run(State(state): State<ApiState>, Path(run_id): Path<String>) -> Result<Json<RunSummary>, StatusCode> {
    state.runs.get(&run_id).map(|h| Json(h.summary)).ok_or(StatusCode::NOT_FOUND)
}

async fn pause_run(State(state): State<ApiState>, Path(run_id): Path<String>) -> StatusCode {
    if let Some(handle) = state.runs.set_state(&run_id, PersistentRunState::Paused) {
        handle.pause.pause();
        publish_lifecycle(&handle, RunStatus::Paused, "manual pause requested");
        StatusCode::ACCEPTED
    } else { StatusCode::NOT_FOUND }
}

async fn resume_run(State(state): State<ApiState>, Path(run_id): Path<String>) -> StatusCode {
    if let Some(handle) = state.runs.set_state(&run_id, PersistentRunState::Running) {
        handle.pause.resume();
        publish_lifecycle(&handle, RunStatus::Running, "manual resume requested");
        StatusCode::ACCEPTED
    } else { StatusCode::NOT_FOUND }
}

async fn cancel_run(State(state): State<ApiState>, Path(run_id): Path<String>) -> StatusCode {
    if let Some(handle) = state.runs.set_state(&run_id, PersistentRunState::Failed) {
        handle.cancellation.cancel();
        publish_lifecycle(&handle, RunStatus::Cancelled, "manual cancellation requested");
        StatusCode::ACCEPTED
    } else { StatusCode::NOT_FOUND }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ArtifactEntry {
    pub path: String,
    pub bytes: u64,
}

async fn run_evaluations(State(state): State<ApiState>, Path(run_id): Path<String>) -> Result<Json<Vec<serde_json::Value>>, StatusCode> {
    read_jsonl_values(state.storage_root.join("results").join(&run_id).join("evaluations.jsonl")).await.map(Json)
}

async fn run_trades(State(state): State<ApiState>, Path(run_id): Path<String>) -> Result<Json<Vec<serde_json::Value>>, StatusCode> {
    read_jsonl_values(state.storage_root.join("results").join(&run_id).join("trades.jsonl")).await.map(Json)
}

async fn run_equity(State(state): State<ApiState>, Path(run_id): Path<String>) -> Result<Json<Vec<serde_json::Value>>, StatusCode> {
    read_jsonl_values(state.storage_root.join("results").join(&run_id).join("equity.jsonl")).await.map(Json)
}

async fn run_metrics(State(state): State<ApiState>, Path(run_id): Path<String>) -> Result<Json<Vec<serde_json::Value>>, StatusCode> {
    read_jsonl_values(state.storage_root.join("results").join(&run_id).join("metrics.jsonl")).await.map(Json)
}

async fn read_jsonl_values(path: PathBuf) -> Result<Vec<serde_json::Value>, StatusCode> {
    let content = tokio::fs::read_to_string(path).await.map_err(|_| StatusCode::NOT_FOUND)?;
    let mut values = Vec::new();
    for line in content.lines() {
        if line.trim().is_empty() { continue; }
        let value: serde_json::Value = serde_json::from_str(line).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        values.push(value);
    }
    Ok(values)
}

async fn run_validation(State(state): State<ApiState>, Path(run_id): Path<String>) -> Result<Json<serde_json::Value>, StatusCode> {
    let report_path = state.storage_root.join("artifacts").join(&run_id).join("report.json");
    let bytes = tokio::fs::read(&report_path).await.map_err(|_| StatusCode::NOT_FOUND)?;
    let report: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let validation = serde_json::json!({
        "walk_forward_reports": report.get("walk_forward_reports").cloned(),
        "monte_carlo_reports": report.get("monte_carlo_reports").cloned(),
        "sensitivity_reports": report.get("sensitivity_reports").cloned(),
        "regime_reports": report.get("regime_reports").cloned(),
        "execution_stress_reports": report.get("execution_stress_reports").cloned(),
        "parameter_decay_reports": report.get("parameter_decay_reports").cloned()
    });
    Ok(Json(validation))
}

async fn run_search_state(State(state): State<ApiState>, Path(run_id): Path<String>) -> Result<Json<serde_json::Value>, StatusCode> {
    let path = state.storage_root.join("checkpoints").join(&run_id).join("search_state.latest.json");
    let bytes = tokio::fs::read(&path).await.map_err(|_| StatusCode::NOT_FOUND)?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(value))
}

async fn run_ranking(State(state): State<ApiState>, Path(run_id): Path<String>) -> Result<Json<serde_json::Value>, StatusCode> {
    let report_path = state.storage_root.join("artifacts").join(&run_id).join("report.json");
    let bytes = tokio::fs::read(&report_path).await.map_err(|_| StatusCode::NOT_FOUND)?;
    let report: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let ranking = match report.get("ranking_state").cloned() { Some(value) => value, None => serde_json::json!({}) };
    Ok(Json(ranking))
}

async fn list_artifacts(State(state): State<ApiState>, Path(run_id): Path<String>) -> Result<Json<Vec<ArtifactEntry>>, StatusCode> {
    let base = state.storage_root.join("artifacts").join(&run_id);
    let mut entries = Vec::new();
    let mut stack = vec![base.clone()];
    while let Some(dir) = stack.pop() {
        let mut read_dir = tokio::fs::read_dir(&dir).await.map_err(|_| StatusCode::NOT_FOUND)?;
        while let Some(entry) = read_dir.next_entry().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)? {
            let path = entry.path();
            let metadata = entry.metadata().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            if metadata.is_dir() {
                stack.push(path);
            } else if metadata.is_file() {
                let rel = path.strip_prefix(&base).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                entries.push(ArtifactEntry { path: rel.to_string_lossy().to_string(), bytes: metadata.len() });
            }
        }
    }
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(Json(entries))
}

async fn get_artifact(State(state): State<ApiState>, Path((run_id, artifact_path)): Path<(String, String)>) -> Result<impl IntoResponse, StatusCode> {
    if !is_safe_relative_path(&artifact_path) { return Err(StatusCode::BAD_REQUEST); }
    let path = state.storage_root.join("artifacts").join(&run_id).join(&artifact_path);
    let bytes = tokio::fs::read(&path).await.map_err(|_| StatusCode::NOT_FOUND)?;
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/octet-stream"));
    Ok((headers, Body::from(bytes)))
}

fn is_safe_relative_path(path: &str) -> bool {
    let candidate = std::path::Path::new(path);
    if candidate.is_absolute() { return false; }
    for component in candidate.components() {
        match component {
            Component::Normal(_) => {}
            _ => return false,
        }
    }
    true
}

async fn recover_run(State(state): State<ApiState>, Path(run_id): Path<String>) -> Result<Json<RunSummary>, StatusCode> {
    let checkpoint_store = AtomicCheckpointStore::new(StorageLayout::new(state.storage_root.clone()));
    checkpoint_store.load_latest(&RunId(run_id.clone())).map_err(|_| StatusCode::NOT_FOUND)?;
    let handle = state.runs.create_recovered(run_id);
    if let Some(launcher) = &state.launcher {
        launcher.launch(handle.clone());
    }
    Ok(Json(handle.summary))
}

async fn run_snapshot(State(state): State<ApiState>, Path(run_id): Path<String>) -> Result<Json<RunProgressSnapshot>, StatusCode> {
    state.runs.get(&run_id).map(|h| Json(h.progress.snapshot())).ok_or(StatusCode::NOT_FOUND)
}

async fn ws_run(State(state): State<ApiState>, Path(run_id): Path<String>, ws: WebSocketUpgrade) -> impl IntoResponse {
    let Some(handle) = state.runs.get(&run_id) else { return StatusCode::NOT_FOUND.into_response(); };
    ws.on_upgrade(move |socket| async move {
        let (mut sender, mut receiver) = socket.split();
        let snapshot = handle.progress.snapshot();
        if let Some(event) = snapshot.latest.clone() {
            let envelope = WsEnvelope { schema_version: 1, message_type: "Snapshot".into(), run_id: Some(RunId(run_id.clone())), sequence: snapshot.sequence, payload: event };
            if let Ok(text) = serde_json::to_string(&envelope) { let _ = sender.send(Message::Text(text)).await; }
        }
        let mut rx = handle.progress.subscribe();
        loop {
            tokio::select! {
                recv = rx.recv() => {
                    match recv {
                        Ok((seq, event)) => {
                            let envelope = WsEnvelope { schema_version: 1, message_type: "Progress".into(), run_id: Some(RunId(run_id.clone())), sequence: seq, payload: event };
                            match serde_json::to_string(&envelope) {
                                Ok(text) => { if sender.send(Message::Text(text)).await.is_err() { break; } }
                                Err(_) => break,
                            }
                        }
                        Err(_) => break,
                    }
                }
                msg = receiver.next() => {
                    if msg.is_none() { break; }
                }
            }
        }
    })
}

fn publish_lifecycle(handle: &RunHandle, status: RunStatus, message: &str) {
    let now = chrono::Utc::now().timestamp_millis();
    handle.progress.publish(ProgressEvent {
        schema_version: 1,
        run_id: RunId(handle.summary.run_id.clone()),
        stage: "RunLifecycle".into(),
        status,
        worker_id: None,
        current: 0,
        total: None,
        percent: None,
        best_score_so_far: None,
        message: message.into(),
        error: None,
        timestamp: now,
    });
}
