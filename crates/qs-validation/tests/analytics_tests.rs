use qs_core::*;
use qs_validation::{monte_carlo_report, parameter_decay_report, walk_forward_report};
use std::collections::BTreeMap;

fn result_with_returns(values: &[f64]) -> EvaluationResult {
    let instrument = Instrument { symbol: "X".into(), venue: None, asset_class: None };
    let trades = values.iter().enumerate().map(|(idx, value)| Trade {
        trade_id: format!("t{idx}"),
        strategy_id: StrategyId("s".into()),
        parameter_set_id: ParameterSetId("p".into()),
        instrument: instrument.clone(),
        direction: TradeDirection::Long,
        entry_time: idx as i64,
        exit_time: idx as i64 + 1,
        entry_price: 1.0,
        exit_price: 1.0 + value,
        size: 1.0,
        pnl: *value,
        r_multiple: *value,
        fees: 0.0,
        slippage: 0.0,
        tags: BTreeMap::new(),
    }).collect::<Vec<_>>();
    EvaluationResult {
        run_id: RunId("r".into()),
        strategy_id: StrategyId("s".into()),
        parameter_set_id: ParameterSetId("p".into()),
        dataset_id: DatasetId("d".into()),
        trades,
        equity_curve: Vec::new(),
        metrics: MetricBundle::default(),
        diagnostics: EvaluationDiagnostics::default(),
    }
}

#[test]
fn walk_forward_creates_folds() {
    let report = walk_forward_report(&result_with_returns(&[1.0, -0.5, 2.0, 0.5]), 2);
    assert_eq!(report.folds.len(), 2);
}

#[test]
fn monte_carlo_is_deterministic_for_seed() {
    let result = result_with_returns(&[1.0, -0.5, 2.0, 0.5]);
    let a = monte_carlo_report(&result, 25, 42);
    let b = monte_carlo_report(&result, 25, 42);
    assert_eq!(a.p50_total_r, b.p50_total_r);
}

#[test]
fn decay_report_handles_halves() {
    let report = parameter_decay_report(&result_with_returns(&[1.0, 1.0, -1.0, -1.0]));
    assert_eq!(report.first_half_total_r, 2.0);
    assert_eq!(report.second_half_total_r, -2.0);
}
