pub mod checkpointing;
pub use checkpointing::*;

use qs_core::*;
use qs_storage::AtomicCheckpointStore;

pub struct PipelineOrchestrator {
    components: Vec<Box<dyn PipelineComponent>>,
}

impl PipelineOrchestrator {
    pub fn new(components: Vec<Box<dyn PipelineComponent>>) -> Self {
        Self { components }
    }

    pub fn validate_contract_order(&self) -> Result<(), PipelineError> {
        let mut available: Vec<DataContract> = Vec::new();
        for c in &self.components {
            for required in c.input_contract() {
                if !available
                    .iter()
                    .any(|x| std::mem::discriminant(x) == std::mem::discriminant(&required))
                {
                    return Err(PipelineError::Invariant {
                        message: format!(
                            "component {} requires missing contract {:?}",
                            c.id(),
                            required
                        ),
                    });
                }
            }
            available.extend(c.output_contract());
        }
        Ok(())
    }

    pub async fn run(
        &self,
        ctx: &mut PipelineContext,
    ) -> Result<PersistentRunState, PipelineError> {
        self.run_internal(ctx, None).await
    }

    pub async fn run_with_checkpoints(
        &self,
        ctx: &mut PipelineContext,
        checkpoints: &AtomicCheckpointStore,
    ) -> Result<PersistentRunState, PipelineError> {
        self.run_internal(ctx, Some(checkpoints)).await
    }

    async fn run_internal(
        &self,
        ctx: &mut PipelineContext,
        checkpoints: Option<&AtomicCheckpointStore>,
    ) -> Result<PersistentRunState, PipelineError> {
        self.validate_contract_order()?;
        if let Some(store) = checkpoints {
            save_context_checkpoint(ctx, store, PersistentRunState::Running)?;
        }

        for component in &self.components {
            if ctx.component_states.is_completed(&component.id()) {
                continue;
            }
            ctx.pause.wait_if_paused().await?;
            ctx.cancellation.check_cancelled()?;
            ctx.component_states.running = Some(component.id());
            emit_component_status(
                ctx,
                component.as_ref(),
                RunStatus::Running,
                "component started",
                None,
            );
            if let Some(store) = checkpoints {
                save_context_checkpoint(ctx, store, PersistentRunState::Running)?;
            }

            let mut attempts = 0u32;
            loop {
                match component.execute(ctx).await {
                    Ok(outcome) => {
                        ctx.component_states.completed.insert(component.id());
                        ctx.component_states.running = None;
                        emit_component_status(
                            ctx,
                            component.as_ref(),
                            RunStatus::Completed,
                            &outcome.message,
                            None,
                        );
                        if let Some(store) = checkpoints {
                            save_context_checkpoint(ctx, store, PersistentRunState::Running)?;
                        }
                        break;
                    }
                    Err(e) => match component.failure_policy() {
                        FailurePolicy::Retry { max } if attempts < max => {
                            attempts += 1;
                            emit_component_status(
                                ctx,
                                component.as_ref(),
                                RunStatus::Running,
                                &format!("retrying component attempt {attempts}/{max}"),
                                None,
                            );
                            continue;
                        }
                        FailurePolicy::SkipComponent => {
                            emit_component_status(
                                ctx,
                                component.as_ref(),
                                RunStatus::Failed,
                                "component skipped after failure",
                                Some(e),
                            );
                            if let Some(store) = checkpoints {
                                save_context_checkpoint(ctx, store, PersistentRunState::Running)?;
                            }
                            break;
                        }
                        FailurePolicy::PauseRun => {
                            ctx.pause.pause();
                            emit_component_status(
                                ctx,
                                component.as_ref(),
                                RunStatus::Paused,
                                "run paused after failure",
                                Some(e),
                            );
                            if let Some(store) = checkpoints {
                                save_context_checkpoint(ctx, store, PersistentRunState::Paused)?;
                            }
                            return Ok(PersistentRunState::Paused);
                        }
                        FailurePolicy::AbortRun | FailurePolicy::Retry { .. } => {
                            emit_component_status(
                                ctx,
                                component.as_ref(),
                                RunStatus::Failed,
                                "run failed",
                                Some(e),
                            );
                            if let Some(store) = checkpoints {
                                save_context_checkpoint(ctx, store, PersistentRunState::Failed)?;
                            }
                            return Ok(PersistentRunState::Failed);
                        }
                    },
                }
            }
        }
        if let Some(store) = checkpoints {
            save_context_checkpoint(ctx, store, PersistentRunState::Completed)?;
        }
        Ok(PersistentRunState::Completed)
    }
}

fn emit_component_status(
    ctx: &PipelineContext,
    component: &dyn PipelineComponent,
    status: RunStatus,
    message: &str,
    error: Option<PipelineError>,
) {
    let Some(run_id) = ctx.run_id.clone() else {
        return;
    };
    let err = error.map(|e| e.to_serializable(Some(component.id())));
    ctx.emit(ProgressEvent {
        schema_version: 1,
        run_id,
        stage: component.name().into(),
        status,
        worker_id: None,
        current: 0,
        total: None,
        percent: None,
        best_score_so_far: None,
        message: message.into(),
        error: err,
        timestamp: chrono::Utc::now().timestamp_millis(),
    });
}
