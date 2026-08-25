use crate::{DefaultNeighborhood, GenerationBudget, ParameterNeighborhood};
use qs_core::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchPlanConfig {
    pub top_n: usize,
    pub intensification_radius: f64,
    pub intensification_budget_per_top: usize,
    pub completion_budget: usize,
}

impl Default for SearchPlanConfig {
    fn default() -> Self {
        Self { top_n: 5, intensification_radius: 0.20, intensification_budget_per_top: 8, completion_budget: 32 }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchPlan {
    pub schema_version: u32,
    pub strategy_id: StrategyId,
    pub selected_top: Vec<ParameterSetId>,
    pub intensification_candidates: Vec<ParameterSet>,
    pub completion_candidates: Vec<ParameterSet>,
}

pub fn build_intensification_plan(
    space: &ParameterSpace,
    evaluated: &[EvaluationResult],
    config: &SearchPlanConfig,
) -> Result<SearchPlan, SearchError> {
    let mut ranked = evaluated.iter().filter(|result| result.strategy_id == space.strategy_id).collect::<Vec<_>>();
    ranked.sort_by(|a, b| {
        let av = robust_score(a);
        let bv = robust_score(b);
        match bv.partial_cmp(&av) { Some(ordering) => ordering, None => std::cmp::Ordering::Equal }
    });

    let top = ranked.into_iter().take(config.top_n).collect::<Vec<_>>();
    let selected_top = top.iter().map(|result| result.parameter_set_id.clone()).collect::<Vec<_>>();
    let lookup = collect_parameter_sets(evaluated);
    let evaluated_ids = evaluated.iter().map(|result| result.parameter_set_id.clone()).collect::<BTreeSet<_>>();
    let neighborhood = DefaultNeighborhood;
    let mut generated_ids = BTreeSet::new();
    let mut intensification_candidates = Vec::new();

    for result in top {
        let Some(center) = lookup.get(&result.parameter_set_id) else { continue; };
        for candidate in neighborhood.neighbors(center, config.intensification_radius, config.intensification_budget_per_top, space)? {
            if evaluated_ids.contains(&candidate.id) || !generated_ids.insert(candidate.id.clone()) { continue; }
            intensification_candidates.push(candidate);
        }
    }

    let mut completion_candidates = Vec::new();
    let baseline = crate::generate_budgeted(space, GenerationBudget { max_candidates: config.completion_budget.saturating_mul(4).max(config.completion_budget) }, 17)?;
    for candidate in baseline {
        if completion_candidates.len() >= config.completion_budget { break; }
        if evaluated_ids.contains(&candidate.id) || generated_ids.contains(&candidate.id) { continue; }
        completion_candidates.push(candidate);
    }

    Ok(SearchPlan { schema_version: 1, strategy_id: space.strategy_id.clone(), selected_top, intensification_candidates, completion_candidates })
}

pub fn robust_score(result: &EvaluationResult) -> f64 {
    let c = &result.metrics.classical;
    let s = &result.metrics.stability;
    let pf = if c.profit_factor.is_finite() { c.profit_factor.min(5.0) / 5.0 } else { 0.0 };
    let stability = match s.edge_stability_ratio { Some(value) if value.is_finite() => value, _ => 0.0 };
    let dd_penalty = c.max_drawdown.max(0.0).min(1.0);
    let trade_penalty = if result.trades.len() < 30 { 0.20 } else { 0.0 };
    c.expectancy.tanh() * 0.40 + pf * 0.25 + stability * 0.25 - dd_penalty * 0.30 - trade_penalty
}

fn collect_parameter_sets(evaluated: &[EvaluationResult]) -> BTreeMap<ParameterSetId, ParameterSet> {
    let mut out = BTreeMap::new();
    for result in evaluated {
        out.entry(result.parameter_set_id.clone()).or_insert_with(|| ParameterSet {
            id: result.parameter_set_id.clone(),
            strategy_id: result.strategy_id.clone(),
            values: BTreeMap::new(),
            source: ParameterSetSource::ResumeRecovered,
            parent_ids: Vec::new(),
        });
    }
    out
}

pub fn build_intensification_plan_from_candidates(
    space: &ParameterSpace,
    evaluated: &[EvaluationResult],
    candidates: &[ParameterSet],
    config: &SearchPlanConfig,
) -> Result<SearchPlan, SearchError> {
    let mut ranked = evaluated.iter().filter(|result| result.strategy_id == space.strategy_id).collect::<Vec<_>>();
    ranked.sort_by(|a, b| {
        let av = robust_score(a);
        let bv = robust_score(b);
        match bv.partial_cmp(&av) { Some(ordering) => ordering, None => std::cmp::Ordering::Equal }
    });
    let selected_top = ranked.iter().take(config.top_n).map(|result| result.parameter_set_id.clone()).collect::<Vec<_>>();
    let candidate_lookup = candidates.iter().map(|candidate| (candidate.id.clone(), candidate.clone())).collect::<BTreeMap<_, _>>();
    let evaluated_ids = evaluated.iter().map(|result| result.parameter_set_id.clone()).collect::<BTreeSet<_>>();
    let neighborhood = DefaultNeighborhood;
    let mut generated_ids = BTreeSet::new();
    let mut intensification_candidates = Vec::new();
    for parameter_set_id in &selected_top {
        let Some(center) = candidate_lookup.get(parameter_set_id) else { continue; };
        for candidate in neighborhood.neighbors(center, config.intensification_radius, config.intensification_budget_per_top, space)? {
            if evaluated_ids.contains(&candidate.id) || !generated_ids.insert(candidate.id.clone()) { continue; }
            intensification_candidates.push(candidate);
        }
    }
    let completion_candidates = candidates.iter()
        .filter(|candidate| !evaluated_ids.contains(&candidate.id) && !generated_ids.contains(&candidate.id))
        .take(config.completion_budget)
        .cloned()
        .collect::<Vec<_>>();
    Ok(SearchPlan { schema_version: 1, strategy_id: space.strategy_id.clone(), selected_top, intensification_candidates, completion_candidates })
}
