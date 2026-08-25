use qs_core::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WalkForwardFold {
    pub fold_index: usize,
    pub start_time: i64,
    pub end_time: i64,
    pub trade_count: usize,
    pub total_r: f64,
    pub average_r: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WalkForwardReport {
    pub schema_version: u32,
    pub strategy_id: StrategyId,
    pub parameter_set_id: ParameterSetId,
    pub folds: Vec<WalkForwardFold>,
    pub consistency_ratio: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MonteCarloReport {
    pub schema_version: u32,
    pub strategy_id: StrategyId,
    pub parameter_set_id: ParameterSetId,
    pub simulations: usize,
    pub p05_total_r: f64,
    pub p50_total_r: f64,
    pub p95_total_r: f64,
    pub probability_negative: f64,
}

pub fn walk_forward_report(result: &EvaluationResult, folds: usize) -> WalkForwardReport {
    let folds = folds.max(1);
    let mut out = Vec::new();
    if result.trades.is_empty() {
        return WalkForwardReport { schema_version: 1, strategy_id: result.strategy_id.clone(), parameter_set_id: result.parameter_set_id.clone(), folds: out, consistency_ratio: 0.0 };
    }
    let chunk = result.trades.len().div_ceil(folds);
    for (idx, slice) in result.trades.chunks(chunk).enumerate() {
        let total_r = slice.iter().map(|trade| trade.r_multiple).sum::<f64>();
        let average_r = total_r / slice.len() as f64;
        let start_time = match slice.first() { Some(trade) => trade.entry_time, None => 0 };
        let end_time = match slice.last() { Some(trade) => trade.exit_time, None => 0 };
        out.push(WalkForwardFold { fold_index: idx, start_time, end_time, trade_count: slice.len(), total_r, average_r });
    }
    let positive = out.iter().filter(|fold| fold.total_r > 0.0).count();
    let consistency_ratio = if out.is_empty() { 0.0 } else { positive as f64 / out.len() as f64 };
    WalkForwardReport { schema_version: 1, strategy_id: result.strategy_id.clone(), parameter_set_id: result.parameter_set_id.clone(), folds: out, consistency_ratio }
}

pub fn monte_carlo_report(result: &EvaluationResult, simulations: usize, seed: u64) -> MonteCarloReport {
    let returns = result.trades.iter().map(|trade| trade.r_multiple).collect::<Vec<_>>();
    if returns.is_empty() || simulations == 0 {
        return MonteCarloReport { schema_version: 1, strategy_id: result.strategy_id.clone(), parameter_set_id: result.parameter_set_id.clone(), simulations: 0, p05_total_r: 0.0, p50_total_r: 0.0, p95_total_r: 0.0, probability_negative: 0.0 };
    }
    let mut state = if seed == 0 { 0xA5A5_1234_9876_FEDC } else { seed };
    let mut totals = Vec::with_capacity(simulations);
    for _ in 0..simulations {
        let mut total = 0.0;
        for _ in 0..returns.len() {
            state = lcg_next(state);
            let idx = (state as usize) % returns.len();
            total += returns[idx];
        }
        totals.push(total);
    }
    totals.sort_by(|a, b| match a.partial_cmp(b) { Some(ordering) => ordering, None => std::cmp::Ordering::Equal });
    let negative = totals.iter().filter(|value| **value < 0.0).count();
    MonteCarloReport {
        schema_version: 1,
        strategy_id: result.strategy_id.clone(),
        parameter_set_id: result.parameter_set_id.clone(),
        simulations,
        p05_total_r: percentile(&totals, 0.05),
        p50_total_r: percentile(&totals, 0.50),
        p95_total_r: percentile(&totals, 0.95),
        probability_negative: negative as f64 / totals.len() as f64,
    }
}

fn percentile(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() { return 0.0; }
    let idx = ((sorted.len() - 1) as f64 * q.clamp(0.0, 1.0)).round() as usize;
    sorted[idx]
}

fn lcg_next(state: u64) -> u64 { state.wrapping_mul(2862933555777941757).wrapping_add(3037000493) }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SensitivityReport {
    pub schema_version: u32,
    pub strategy_id: StrategyId,
    pub parameter_set_id: ParameterSetId,
    pub local_trade_std: Option<f64>,
    pub fragility_score: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegimeBucketReport {
    pub bucket: String,
    pub trade_count: usize,
    pub total_r: f64,
    pub average_r: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegimeReport {
    pub schema_version: u32,
    pub strategy_id: StrategyId,
    pub parameter_set_id: ParameterSetId,
    pub buckets: Vec<RegimeBucketReport>,
    pub inter_bucket_variance: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecutionStressReport {
    pub schema_version: u32,
    pub strategy_id: StrategyId,
    pub parameter_set_id: ParameterSetId,
    pub base_total_r: f64,
    pub stressed_total_r: f64,
    pub stress_loss: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParameterDecayReport {
    pub schema_version: u32,
    pub strategy_id: StrategyId,
    pub parameter_set_id: ParameterSetId,
    pub first_half_total_r: f64,
    pub second_half_total_r: f64,
    pub decay_ratio: Option<f64>,
}

pub fn sensitivity_report(result: &EvaluationResult) -> SensitivityReport {
    let returns = result.trades.iter().map(|trade| trade.r_multiple).collect::<Vec<_>>();
    let std = sample_std(&returns);
    let mean_abs = if returns.is_empty() { 0.0 } else { returns.iter().map(|value| value.abs()).sum::<f64>() / returns.len() as f64 };
    let fragility_score = match std {
        Some(value) if mean_abs > 0.0 => (value / mean_abs).min(10.0),
        Some(value) => value.min(10.0),
        None => 0.0,
    };
    SensitivityReport { schema_version: 1, strategy_id: result.strategy_id.clone(), parameter_set_id: result.parameter_set_id.clone(), local_trade_std: std, fragility_score }
}

pub fn regime_report(result: &EvaluationResult) -> RegimeReport {
    let mut positive = Vec::new();
    let mut negative = Vec::new();
    for trade in &result.trades {
        if trade.r_multiple >= 0.0 { positive.push(trade.r_multiple); } else { negative.push(trade.r_multiple); }
    }
    let buckets = vec![bucket_report("positive_trades", &positive), bucket_report("negative_trades", &negative)];
    let averages = buckets.iter().map(|bucket| bucket.average_r).collect::<Vec<_>>();
    RegimeReport { schema_version: 1, strategy_id: result.strategy_id.clone(), parameter_set_id: result.parameter_set_id.clone(), buckets, inter_bucket_variance: sample_variance(&averages) }
}

pub fn execution_stress_report(result: &EvaluationResult, slippage_r_per_trade: f64) -> ExecutionStressReport {
    let base_total_r = result.metrics.classical.total_r;
    let stressed_total_r = base_total_r - slippage_r_per_trade.abs() * result.trades.len() as f64;
    ExecutionStressReport { schema_version: 1, strategy_id: result.strategy_id.clone(), parameter_set_id: result.parameter_set_id.clone(), base_total_r, stressed_total_r, stress_loss: base_total_r - stressed_total_r }
}

pub fn parameter_decay_report(result: &EvaluationResult) -> ParameterDecayReport {
    let mid = result.trades.len() / 2;
    let first_half_total_r = result.trades[..mid].iter().map(|trade| trade.r_multiple).sum::<f64>();
    let second_half_total_r = result.trades[mid..].iter().map(|trade| trade.r_multiple).sum::<f64>();
    let decay_ratio = if first_half_total_r.abs() < f64::EPSILON { None } else { Some(second_half_total_r / first_half_total_r) };
    ParameterDecayReport { schema_version: 1, strategy_id: result.strategy_id.clone(), parameter_set_id: result.parameter_set_id.clone(), first_half_total_r, second_half_total_r, decay_ratio }
}

fn bucket_report(name: &str, values: &[f64]) -> RegimeBucketReport {
    let total_r = values.iter().sum::<f64>();
    let average_r = if values.is_empty() { 0.0 } else { total_r / values.len() as f64 };
    RegimeBucketReport { bucket: name.into(), trade_count: values.len(), total_r, average_r }
}

fn sample_variance(values: &[f64]) -> Option<f64> {
    if values.len() < 2 { return None; }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    Some(values.iter().map(|value| (value - mean).powi(2)).sum::<f64>() / (values.len() - 1) as f64)
}

fn sample_std(values: &[f64]) -> Option<f64> { sample_variance(values).map(f64::sqrt) }
