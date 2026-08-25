pub mod advanced;
pub mod generator;
pub mod runtime;
pub use advanced::*;
pub use generator::*;
pub use runtime::*;

use async_trait::async_trait;
use qs_core::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

pub trait ParameterNeighborhood: Send + Sync {
    fn distance(
        &self,
        a: &ParameterSet,
        b: &ParameterSet,
        space: &ParameterSpace,
    ) -> Result<f64, SearchError>;
    fn neighbors(
        &self,
        center: &ParameterSet,
        radius: f64,
        budget: usize,
        space: &ParameterSpace,
    ) -> Result<Vec<ParameterSet>, SearchError>;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SearchPhase {
    SparseExploration,
    Intensification,
    ControlledCompletion,
    Finished,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScoredCandidate {
    pub parameter_set_id: ParameterSetId,
    pub strategy_id: StrategyId,
    pub score: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchState {
    pub schema_version: u32,
    pub evaluated: HashSet<ParameterSetId>,
    pub pending: Vec<ParameterSetId>,
    pub failed: Vec<ParameterSetId>,
    pub best_so_far: Vec<ScoredCandidate>,
    pub phase: SearchPhase,
    pub per_strategy: BTreeMap<StrategyId, serde_json::Value>,
}

pub enum CompletionPolicy {
    None,
    BudgetLimited { max_extra_candidates: usize },
    CoverageTarget { min_dimension_coverage: f64 },
    LocalGridAroundTop { top_n: usize, radius: f64 },
    ExhaustiveIfBelow { max_total_candidates: usize },
}

pub struct ParameterGeneratorComponent;
#[async_trait]
impl PipelineComponent for ParameterGeneratorComponent {
    fn id(&self) -> ComponentId {
        ComponentId("ParameterGenerator".into())
    }
    fn name(&self) -> &'static str {
        "ParameterGenerator"
    }
    fn version(&self) -> ComponentVersion {
        ComponentVersion {
            semver: "0.1.0".into(),
        }
    }
    fn input_contract(&self) -> Vec<DataContract> {
        vec![DataContract::QualityReport]
    }
    fn output_contract(&self) -> Vec<DataContract> {
        vec![
            DataContract::ParameterSpace,
            DataContract::CandidateParameterSets,
        ]
    }
    fn failure_policy(&self) -> FailurePolicy {
        FailurePolicy::AbortRun
    }
    async fn execute(&self, ctx: &mut PipelineContext) -> Result<ComponentOutcome, PipelineError> {
        let budget = match ctx
            .bag
            .get("parameter_generation_budget")
            .and_then(|v| v.as_u64())
        {
            Some(value) => value as usize,
            None => 128,
        };
        for (strategy_id, space) in ctx.parameter_spaces.clone() {
            let seed = match ctx.run_config.as_ref().map(|c| c.seed) {
                Some(value) => value,
                None => 0,
            };
            let candidates = generate_budgeted(
                &space,
                GenerationBudget {
                    max_candidates: budget,
                },
                seed,
            )?;
            ctx.candidate_sets.insert(strategy_id, candidates);
        }
        let total: usize = ctx.candidate_sets.values().map(Vec::len).sum();
        Ok(ComponentOutcome {
            message: format!("{total} parameter candidates generated"),
        })
    }
}

pub struct ParameterSearchComponent;
#[async_trait]
impl PipelineComponent for ParameterSearchComponent {
    fn id(&self) -> ComponentId {
        ComponentId("ParameterSearch".into())
    }
    fn name(&self) -> &'static str {
        "ParameterSearch"
    }
    fn version(&self) -> ComponentVersion {
        ComponentVersion {
            semver: "0.1.0".into(),
        }
    }
    fn input_contract(&self) -> Vec<DataContract> {
        vec![DataContract::CandidateParameterSets]
    }
    fn output_contract(&self) -> Vec<DataContract> {
        vec![DataContract::CandidateParameterSets]
    }
    fn failure_policy(&self) -> FailurePolicy {
        FailurePolicy::AbortRun
    }
    async fn execute(&self, ctx: &mut PipelineContext) -> Result<ComponentOutcome, PipelineError> {
        if ctx.bag.contains_key("search_state") {
            return Ok(ComponentOutcome {
                message: "existing search state preserved from checkpoint".into(),
            });
        }
        let pending: usize = ctx.candidate_sets.values().map(Vec::len).sum();
        let mut initial = Vec::new();
        for candidates in ctx.candidate_sets.values() {
            initial.extend(candidates.iter().cloned());
        }
        let runtime_state = RuntimeSearchState::new(initial);
        let runtime_value = serde_json::to_value(&runtime_state).map_err(|e| {
            PipelineError::Search(SearchError::Message {
                code: "SEARCH_STATE_SERIALIZE".into(),
                message: e.to_string(),
                retryable: false,
            })
        })?;
        ctx.bag.insert("search_runtime_state".into(), runtime_value);
        ctx.bag.insert(
            "search_state".into(),
            serde_json::json!({
                "schema_version": 1,
                "phase": "SparseExploration",
                "pending": pending,
                "evaluated": 0,
                "failed": 0,
                "best_score_so_far": null
            }),
        );
        Ok(ComponentOutcome {
            message: format!("search state initialized with {pending} pending candidates"),
        })
    }
}
