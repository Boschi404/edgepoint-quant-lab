use qs_core::*;

pub fn metric_bundle(trades: &[Trade], equity: &[EquityPoint]) -> MetricBundle {
    MetricBundle {
        classical: classical_metrics(trades, equity),
        stability: stability_metrics(trades, equity, 30),
        regime: None,
        rolling: None,
        stress: None,
        custom: Default::default(),
    }
}

pub fn classical_metrics(trades: &[Trade], equity: &[EquityPoint]) -> ClassicalMetrics {
    let returns: Vec<f64> = trades.iter().map(|t| t.r_multiple).collect();
    let total_r: f64 = returns.iter().sum();
    let average_r = match mean(&returns) {
        Some(value) => value,
        None => 0.0,
    };
    let wins = returns.iter().filter(|v| **v > 0.0).count();
    let gross_win: f64 = returns.iter().filter(|v| **v > 0.0).sum();
    let gross_loss: f64 = returns.iter().filter(|v| **v < 0.0).map(|v| v.abs()).sum();
    let max_drawdown = equity.iter().map(|e| e.drawdown).fold(0.0_f64, f64::max);
    ClassicalMetrics {
        total_r,
        average_r,
        expectancy: average_r,
        winrate: if returns.is_empty() {
            0.0
        } else {
            wins as f64 / returns.len() as f64
        },
        profit_factor: safe_profit_factor(gross_win, gross_loss),
        max_drawdown,
        sharpe: sharpe_like(&returns),
        sortino: sortino_like(&returns),
        calmar: if max_drawdown == 0.0 {
            None
        } else {
            Some(total_r / max_drawdown)
        },
        z_score: z_score_runs(trades),
        lr_correlation: linear_regression_correlation(equity),
        max_consecutive_losses: max_consecutive_losses(trades),
        recovery_factor: if max_drawdown == 0.0 {
            None
        } else {
            Some(total_r / max_drawdown)
        },
        average_trade_duration_secs: average_duration(trades),
    }
}

pub fn stability_metrics(
    trades: &[Trade],
    equity: &[EquityPoint],
    window: usize,
) -> StabilityMetrics {
    let returns: Vec<f64> = trades.iter().map(|t| t.r_multiple).collect();
    StabilityMetrics {
        trade_variance: variance(&returns),
        trade_std: stddev(&returns),
        rolling_average_r: rolling_mean_points(trades, window),
        rolling_profit_factor: rolling_profit_factor_points(trades, window),
        edge_stability_ratio: edge_stability_ratio(trades, window),
        inter_regime_variance: None,
        crisis_window_performance: Vec::new(),
        pnl_autocorrelation: autocorrelation_lag1(&returns),
        ulcer_index: ulcer_index(equity),
        underwater_time_ratio: underwater_time_ratio(equity),
    }
}

fn mean(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        None
    } else {
        Some(values.iter().sum::<f64>() / values.len() as f64)
    }
}

fn variance(values: &[f64]) -> Option<f64> {
    if values.len() < 2 {
        return None;
    }
    let m = mean(values)?;
    Some(values.iter().map(|v| (v - m).powi(2)).sum::<f64>() / (values.len() - 1) as f64)
}

fn stddev(values: &[f64]) -> Option<f64> {
    variance(values).map(f64::sqrt)
}

fn sharpe_like(values: &[f64]) -> Option<f64> {
    let m = mean(values)?;
    let sd = stddev(values)?;
    if sd == 0.0 {
        None
    } else {
        Some(m / sd * (values.len() as f64).sqrt())
    }
}

fn sortino_like(values: &[f64]) -> Option<f64> {
    let m = mean(values)?;
    let downside: Vec<f64> = values.iter().copied().filter(|v| *v < 0.0).collect();
    let dd = stddev(&downside)?;
    if dd == 0.0 {
        None
    } else {
        Some(m / dd * (values.len() as f64).sqrt())
    }
}

fn safe_profit_factor(gross_win: f64, gross_loss: f64) -> f64 {
    if gross_loss == 0.0 {
        if gross_win > 0.0 {
            999.0
        } else {
            0.0
        }
    } else {
        gross_win / gross_loss
    }
}

fn max_consecutive_losses(trades: &[Trade]) -> u32 {
    let mut best = 0;
    let mut cur = 0;
    for trade in trades {
        if trade.r_multiple < 0.0 {
            cur += 1;
            best = best.max(cur);
        } else {
            cur = 0;
        }
    }
    best
}

fn average_duration(trades: &[Trade]) -> Option<f64> {
    if trades.is_empty() {
        None
    } else {
        Some(
            trades
                .iter()
                .map(|t| (t.exit_time - t.entry_time) as f64)
                .sum::<f64>()
                / trades.len() as f64,
        )
    }
}

fn rolling_mean_points(trades: &[Trade], window: usize) -> Vec<RollingPoint> {
    if window == 0 || trades.len() < window {
        return Vec::new();
    }
    trades
        .windows(window)
        .filter_map(|slice| {
            let end = slice.last()?;
            Some(RollingPoint {
                timestamp: end.exit_time,
                value: slice.iter().map(|t| t.r_multiple).sum::<f64>() / window as f64,
            })
        })
        .collect()
}

fn rolling_profit_factor_points(trades: &[Trade], window: usize) -> Vec<RollingPoint> {
    if window == 0 || trades.len() < window {
        return Vec::new();
    }
    trades
        .windows(window)
        .filter_map(|slice| {
            let end = slice.last()?;
            let win: f64 = slice
                .iter()
                .filter(|t| t.r_multiple > 0.0)
                .map(|t| t.r_multiple)
                .sum();
            let loss: f64 = slice
                .iter()
                .filter(|t| t.r_multiple < 0.0)
                .map(|t| t.r_multiple.abs())
                .sum();
            let value = safe_profit_factor(win, loss);
            Some(RollingPoint {
                timestamp: end.exit_time,
                value,
            })
        })
        .collect()
}

fn edge_stability_ratio(trades: &[Trade], window: usize) -> Option<f64> {
    let points = rolling_mean_points(trades, window);
    if points.is_empty() {
        return None;
    }
    let positive = points.iter().filter(|p| p.value > 0.0).count();
    Some(positive as f64 / points.len() as f64)
}

fn autocorrelation_lag1(values: &[f64]) -> Option<f64> {
    if values.len() < 3 {
        return None;
    }
    let x = &values[..values.len() - 1];
    let y = &values[1..];
    pearson(x, y)
}

fn linear_regression_correlation(equity: &[EquityPoint]) -> Option<f64> {
    if equity.len() < 3 {
        return None;
    }
    let x: Vec<f64> = (0..equity.len()).map(|i| i as f64).collect();
    let y: Vec<f64> = equity.iter().map(|p| p.equity).collect();
    pearson(&x, &y)
}

fn pearson(x: &[f64], y: &[f64]) -> Option<f64> {
    if x.len() != y.len() || x.len() < 2 {
        return None;
    }
    let mx = mean(x)?;
    let my = mean(y)?;
    let num: f64 = x.iter().zip(y).map(|(a, b)| (a - mx) * (b - my)).sum();
    let den_x: f64 = x.iter().map(|a| (a - mx).powi(2)).sum();
    let den_y: f64 = y.iter().map(|b| (b - my).powi(2)).sum();
    let den = (den_x * den_y).sqrt();
    if den == 0.0 {
        None
    } else {
        Some(num / den)
    }
}

fn ulcer_index(equity: &[EquityPoint]) -> Option<f64> {
    if equity.is_empty() {
        return None;
    }
    Some((equity.iter().map(|p| p.drawdown.powi(2)).sum::<f64>() / equity.len() as f64).sqrt())
}

fn underwater_time_ratio(equity: &[EquityPoint]) -> Option<f64> {
    if equity.is_empty() {
        return None;
    }
    Some(equity.iter().filter(|p| p.underwater).count() as f64 / equity.len() as f64)
}

fn z_score_runs(trades: &[Trade]) -> Option<f64> {
    if trades.len() < 2 {
        return None;
    }
    let wins = trades.iter().filter(|t| t.r_multiple > 0.0).count() as f64;
    let losses = trades.iter().filter(|t| t.r_multiple <= 0.0).count() as f64;
    if wins == 0.0 || losses == 0.0 {
        return None;
    }
    let mut runs = 1.0;
    for pair in trades.windows(2) {
        let a = pair[0].r_multiple > 0.0;
        let b = pair[1].r_multiple > 0.0;
        if a != b {
            runs += 1.0;
        }
    }
    let n = wins + losses;
    let expected_runs = ((2.0 * wins * losses) / n) + 1.0;
    let variance_runs = (2.0 * wins * losses * (2.0 * wins * losses - n)) / (n.powi(2) * (n - 1.0));
    if variance_runs <= 0.0 {
        None
    } else {
        Some((runs - expected_runs) / variance_runs.sqrt())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_metrics_do_not_panic() {
        let m = classical_metrics(&[], &[]);
        assert_eq!(m.total_r, 0.0);
    }
}
