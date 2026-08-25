use crate::*;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ComponentVersion { pub semver: String }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum FailurePolicy { AbortRun, SkipComponent, Retry { max: u32 }, PauseRun }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ComponentOutcome { pub message: String }

#[async_trait]
pub trait PipelineComponent: Send + Sync {
    fn id(&self) -> ComponentId;
    fn name(&self) -> &'static str;
    fn version(&self) -> ComponentVersion;
    fn input_contract(&self) -> Vec<DataContract>;
    fn output_contract(&self) -> Vec<DataContract>;
    fn failure_policy(&self) -> FailurePolicy;
    async fn execute(&self, ctx: &mut PipelineContext) -> Result<ComponentOutcome, PipelineError>;
}
