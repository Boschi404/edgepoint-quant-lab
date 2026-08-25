use qs_core::*;
use qs_storage::{AtomicCheckpointStore, RunCheckpoint};
use std::collections::BTreeMap;

pub fn checkpoint_from_context(ctx: &PipelineContext, state: PersistentRunState) -> Result<RunCheckpoint, PipelineError> {
    let run_id = ctx.run_id.clone().ok_or_else(|| PipelineError::Invariant { message: "run_id missing in context".into() })?;
    let completed_components = ctx.component_states.completed.iter().cloned().collect();
    let mut component_states = BTreeMap::new();
    if let Some(running) = &ctx.component_states.running {
        component_states.insert(running.clone(), "Running".into());
    }
    for completed in &ctx.component_states.completed {
        component_states.insert(completed.clone(), "Completed".into());
    }
    let cp = RunCheckpoint {
        schema_version: 1,
        run_id,
        run_state: state,
        completed_components,
        component_states,
        search_state: match ctx.bag.get("search_runtime_state").cloned() { Some(value) => Some(value), None => ctx.bag.get("search_state").cloned() },
        partial_results_index: serde_json::json!({ "partial_results_count": ctx.partial_results.len() }),
        ranking_state: match ctx.bag.get("ranking_state").cloned() { Some(value) => value, None => serde_json::json!({}) },
        rng_state: serde_json::json!({ "seed": ctx.run_config.as_ref().map(|c| c.seed) }),
        metadata: ctx.metadata.clone(),
        created_at: chrono::Utc::now().timestamp_millis(),
        checksum: String::new(),
    };
    Ok(cp)
}

pub fn save_context_checkpoint(ctx: &PipelineContext, store: &AtomicCheckpointStore, state: PersistentRunState) -> Result<(), PipelineError> {
    let checkpoint = checkpoint_from_context(ctx, state)?;
    store.save_latest(&checkpoint)?;
    Ok(())
}
