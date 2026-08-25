use crate::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunConfig { pub seed: u64, pub pipeline_version: String, pub selected_components: Vec<ComponentId> }

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct RunMetadata { pub created_at: i64, pub updated_at: i64, pub tags: BTreeMap<String, String> }

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ComponentStateMap { pub completed: BTreeSet<ComponentId>, pub running: Option<ComponentId> }
impl ComponentStateMap { pub fn is_completed(&self, id: &ComponentId) -> bool { self.completed.contains(id) } }

#[derive(Default)]
pub struct PipelineContext {
    pub run_id: Option<RunId>,
    pub run_config: Option<RunConfig>,
    pub component_states: ComponentStateMap,
    pub datasets: BTreeMap<DatasetId, MarketDataset>,
    pub parameter_spaces: BTreeMap<StrategyId, ParameterSpace>,
    pub candidate_sets: BTreeMap<StrategyId, Vec<ParameterSet>>,
    pub partial_results: Vec<EvaluationResult>,
    pub progress: Option<ProgressSink>,
    pub cancellation: CancellationToken,
    pub pause: PauseToken,
    pub metadata: RunMetadata,
    pub bag: BTreeMap<String, serde_json::Value>,
}

impl PipelineContext {
    pub fn emit(&self, event: ProgressEvent) { if let Some(sink) = &self.progress { sink.publish(event); } }
}
