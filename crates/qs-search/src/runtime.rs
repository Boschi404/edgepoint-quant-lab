use crate::{build_intensification_plan_from_candidates, robust_score, SearchPlanConfig};
use qs_core::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum RuntimeSearchPhase {
    SparseExploration,
    Intensification,
    ControlledCompletion,
    Finished,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RuntimeSearchConfig {
    pub batch_size: usize,
    pub max_total_evaluations: usize,
    pub plan: SearchPlanConfig,
}

impl Default for RuntimeSearchConfig {
    fn default() -> Self {
        Self { batch_size: 16, max_total_evaluations: 256, plan: SearchPlanConfig::default() }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RuntimeSearchState {
    pub schema_version: u32,
    pub phase: RuntimeSearchPhase,
    pub pending: VecDeque<ParameterSet>,
    pub evaluated: BTreeMap<ParameterSetId, ScoredCandidate>,
    pub failed: BTreeMap<ParameterSetId, String>,
    pub best_score_so_far: Option<f64>,
    pub generated_intensification: bool,
    pub generated_completion: bool,
}

impl RuntimeSearchState {
    pub fn new(initial: Vec<ParameterSet>) -> Self {
        Self {
            schema_version: 1,
            phase: RuntimeSearchPhase::SparseExploration,
            pending: initial.into(),
            evaluated: BTreeMap::new(),
            failed: BTreeMap::new(),
            best_score_so_far: None,
            generated_intensification: false,
            generated_completion: false,
        }
    }

    pub fn next_batch(&mut self, batch_size: usize) -> Vec<ParameterSet> {
        let mut out = Vec::new();
        let size = batch_size.max(1);
        for _ in 0..size {
            let Some(candidate) = self.pending.pop_front() else { break; };
            out.push(candidate);
        }
        out
    }

    pub fn record_evaluation(&mut self, result: &EvaluationResult) {
        let score = robust_score(result);
        if score.is_finite() {
            self.best_score_so_far = match self.best_score_so_far {
                Some(best) if best >= score => Some(best),
                _ => Some(score),
            };
        }
        self.evaluated.insert(result.parameter_set_id.clone(), ScoredCandidate { parameter_set_id: result.parameter_set_id.clone(), strategy_id: result.strategy_id.clone(), score });
    }

    pub fn record_failure(&mut self, candidate: &ParameterSet, message: String) {
        self.failed.insert(candidate.id.clone(), message);
    }

    pub fn is_finished(&self) -> bool { self.phase == RuntimeSearchPhase::Finished }

    pub fn progress(&self) -> RuntimeSearchProgress {
        let evaluated = self.evaluated.len();
        let failed = self.failed.len();
        let pending = self.pending.len();
        let total = evaluated + failed + pending;
        RuntimeSearchProgress { phase: self.phase.clone(), pending, evaluated, failed, total, best_score_so_far: self.best_score_so_far }
    }

    pub fn maybe_advance_phase(
        &mut self,
        space: &ParameterSpace,
        all_candidates: &[ParameterSet],
        results: &[EvaluationResult],
        config: &RuntimeSearchConfig,
    ) -> Result<(), SearchError> {
        if !self.pending.is_empty() { return Ok(()); }
        if self.evaluated.len() + self.failed.len() >= config.max_total_evaluations {
            self.phase = RuntimeSearchPhase::Finished;
            return Ok(());
        }

        match self.phase {
            RuntimeSearchPhase::SparseExploration if !self.generated_intensification => {
                let plan = build_intensification_plan_from_candidates(space, results, all_candidates, &config.plan)?;
                self.enqueue_unique(plan.intensification_candidates);
                self.generated_intensification = true;
                self.phase = if self.pending.is_empty() { RuntimeSearchPhase::ControlledCompletion } else { RuntimeSearchPhase::Intensification };
            }
            RuntimeSearchPhase::Intensification | RuntimeSearchPhase::SparseExploration if !self.generated_completion => {
                let plan = build_intensification_plan_from_candidates(space, results, all_candidates, &config.plan)?;
                self.enqueue_unique(plan.completion_candidates);
                self.generated_completion = true;
                self.phase = if self.pending.is_empty() { RuntimeSearchPhase::Finished } else { RuntimeSearchPhase::ControlledCompletion };
            }
            RuntimeSearchPhase::ControlledCompletion | RuntimeSearchPhase::Intensification | RuntimeSearchPhase::SparseExploration => {
                self.phase = RuntimeSearchPhase::Finished;
            }
            RuntimeSearchPhase::Finished => {}
        }
        Ok(())
    }

    fn enqueue_unique(&mut self, candidates: Vec<ParameterSet>) {
        let known = self.known_ids();
        let mut newly_added = BTreeSet::new();
        for candidate in candidates {
            if known.contains(&candidate.id) || !newly_added.insert(candidate.id.clone()) { continue; }
            self.pending.push_back(candidate);
        }
    }

    fn known_ids(&self) -> BTreeSet<ParameterSetId> {
        let mut ids = self.pending.iter().map(|candidate| candidate.id.clone()).collect::<BTreeSet<_>>();
        ids.extend(self.evaluated.keys().cloned());
        ids.extend(self.failed.keys().cloned());
        ids
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RuntimeSearchProgress {
    pub phase: RuntimeSearchPhase,
    pub pending: usize,
    pub evaluated: usize,
    pub failed: usize,
    pub total: usize,
    pub best_score_so_far: Option<f64>,
}
