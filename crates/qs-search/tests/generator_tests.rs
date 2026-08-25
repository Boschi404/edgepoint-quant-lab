use qs_core::*;
use qs_search::{generate_grid, DefaultNeighborhood, GenerationBudget, ParameterNeighborhood};
use std::collections::BTreeMap;

#[test]
fn grid_generation_respects_budget() {
    let space = ParameterSpace {
        strategy_id: StrategyId("s".into()),
        parameters: vec![
            ParameterDefinition { name: "a".into(), kind: ParameterKind::IntRange { min: 1, max: 10, step: 1 }, default: None },
            ParameterDefinition { name: "b".into(), kind: ParameterKind::Bool, default: None },
        ],
        constraints: vec![],
        neighborhood: NeighborhoodDefinition { metric: "normalized_mixed".into(), weights: BTreeMap::new() },
    };
    let candidates = match generate_grid(&space, GenerationBudget { max_candidates: 5 }) { Ok(value) => value, Err(err) => panic!("grid should generate: {err}") };
    assert_eq!(candidates.len(), 5);
}

#[test]
fn neighborhood_distance_is_zero_for_same_candidate() {
    let space = ParameterSpace {
        strategy_id: StrategyId("s".into()),
        parameters: vec![ParameterDefinition { name: "a".into(), kind: ParameterKind::IntRange { min: 0, max: 10, step: 1 }, default: None }],
        constraints: vec![],
        neighborhood: NeighborhoodDefinition { metric: "normalized_mixed".into(), weights: BTreeMap::new() },
    };
    let mut candidates = match generate_grid(&space, GenerationBudget { max_candidates: 1 }) { Ok(value) => value, Err(err) => panic!("grid: {err}") };
    let candidate = candidates.remove(0);
    let distance = match DefaultNeighborhood.distance(&candidate, &candidate, &space) { Ok(value) => value, Err(err) => panic!("distance: {err}") };
    assert_eq!(distance, 0.0);
}

#[test]
fn budgeted_generation_uses_sparse_source_when_space_exceeds_budget() {
    let space = ParameterSpace {
        strategy_id: StrategyId("s".into()),
        parameters: vec![
            ParameterDefinition { name: "a".into(), kind: ParameterKind::IntRange { min: 1, max: 100, step: 1 }, default: None },
            ParameterDefinition { name: "b".into(), kind: ParameterKind::IntRange { min: 1, max: 100, step: 1 }, default: None },
        ],
        constraints: vec![],
        neighborhood: NeighborhoodDefinition { metric: "normalized_mixed".into(), weights: BTreeMap::new() },
    };
    let candidates = match qs_search::generate_budgeted(&space, GenerationBudget { max_candidates: 10 }, 42) {
        Ok(value) => value,
        Err(err) => panic!("budgeted generation: {err}"),
    };
    assert_eq!(candidates.len(), 10);
    assert!(matches!(candidates[0].source, ParameterSetSource::RandomSparse));
}

#[test]
fn intensification_plan_selects_top_candidate() {
    let space = ParameterSpace {
        strategy_id: StrategyId("s".into()),
        parameters: vec![ParameterDefinition { name: "a".into(), kind: ParameterKind::IntRange { min: 1, max: 5, step: 1 }, default: None }],
        constraints: vec![],
        neighborhood: NeighborhoodDefinition { metric: "normalized_mixed".into(), weights: BTreeMap::new() },
    };
    let candidates = match generate_grid(&space, GenerationBudget { max_candidates: 5 }) { Ok(value) => value, Err(err) => panic!("grid: {err}") };
    let evaluated = candidates.iter().take(2).enumerate().map(|(idx, candidate)| EvaluationResult {
        run_id: RunId("r".into()),
        strategy_id: StrategyId("s".into()),
        parameter_set_id: candidate.id.clone(),
        dataset_id: DatasetId("d".into()),
        trades: Vec::new(),
        equity_curve: Vec::new(),
        metrics: MetricBundle { classical: ClassicalMetrics { expectancy: idx as f64, profit_factor: 1.0, ..ClassicalMetrics::default() }, ..MetricBundle::default() },
        diagnostics: EvaluationDiagnostics::default(),
    }).collect::<Vec<_>>();
    let plan = match qs_search::build_intensification_plan_from_candidates(&space, &evaluated, &candidates, &qs_search::SearchPlanConfig::default()) {
        Ok(value) => value,
        Err(err) => panic!("plan: {err}"),
    };
    assert_eq!(plan.selected_top[0], candidates[1].id);
}
