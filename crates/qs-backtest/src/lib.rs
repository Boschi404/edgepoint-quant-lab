pub mod execution;
pub use execution::*;

use async_trait::async_trait;
use qs_core::*;
use qs_metrics::metric_bundle;
use qs_search::{RuntimeSearchConfig, RuntimeSearchState};
use qs_storage::{atomic_write_json, JsonlResultStore};
use qs_strategy_api::{SignalEvent, StaticStrategyRegistry, StrategyPlugin, StrategyRunInput};
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Clone, Debug)]
pub struct ExecutionModel {
    pub commission_per_trade: f64,
    pub slippage_per_trade: f64,
    pub fixed_size: f64,
    pub initial_equity: f64,
    pub constraints: ExecutionConstraints,
}

impl Default for ExecutionModel {
    fn default() -> Self {
        Self {
            commission_per_trade: 0.0,
            slippage_per_trade: 0.0,
            fixed_size: 1.0,
            initial_equity: 100_000.0,
            constraints: ExecutionConstraints::default(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct BacktestEngine {
    pub execution: ExecutionModel,
}

impl BacktestEngine {
    pub fn new(execution: ExecutionModel) -> Self {
        Self { execution }
    }

    pub fn evaluate_plugin(
        &self,
        run_id: RunId,
        plugin: &dyn StrategyPlugin,
        dataset: &MarketDataset,
        params: &ParameterSet,
        seed: u64,
    ) -> Result<EvaluationResult, PipelineError> {
        plugin.validate_parameters(params)?;
        let output = plugin.run(StrategyRunInput {
            dataset: dataset.clone(),
            parameters: params.clone(),
            seed,
        })?;
        Ok(self.evaluate_signals(
            run_id,
            plugin.metadata().strategy_id,
            dataset,
            params,
            output.signals,
        ))
    }

    pub fn evaluate_signals(
        &self,
        run_id: RunId,
        strategy_id: StrategyId,
        dataset: &MarketDataset,
        params: &ParameterSet,
        signals: Vec<SignalEvent>,
    ) -> EvaluationResult {
        let trades = self.signals_to_trades(strategy_id.clone(), dataset, params, signals);
        let equity_curve = self.equity_curve(&trades, dataset);
        let metrics = metric_bundle(&trades, &equity_curve);
        EvaluationResult {
            run_id,
            strategy_id,
            parameter_set_id: params.id.clone(),
            dataset_id: dataset.dataset_id.clone(),
            trades,
            equity_curve,
            metrics,
            diagnostics: EvaluationDiagnostics::default(),
        }
    }

    fn signals_to_trades(
        &self,
        strategy_id: StrategyId,
        dataset: &MarketDataset,
        params: &ParameterSet,
        signals: Vec<SignalEvent>,
    ) -> Vec<Trade> {
        let mut signals = signals;
        signals.sort_by_key(|s| s.timestamp);
        let mut trades = Vec::new();
        let mut open: Option<SignalEvent> = None;
        for signal in signals {
            match &open {
                None => open = Some(signal),
                Some(entry)
                    if std::mem::discriminant(&entry.side)
                        == std::mem::discriminant(&signal.side) => {}
                Some(_) => {
                    let entry = match open.take() {
                        Some(value) => value,
                        None => continue,
                    };
                    if let Some(trade) = self.close_trade(
                        strategy_id.clone(),
                        dataset,
                        params,
                        &entry,
                        &signal,
                        trades.len(),
                    ) {
                        trades.push(trade);
                    }
                    open = Some(signal);
                }
            }
        }
        trades
    }

    fn close_trade(
        &self,
        strategy_id: StrategyId,
        dataset: &MarketDataset,
        params: &ParameterSet,
        entry: &SignalEvent,
        exit: &SignalEvent,
        idx: usize,
    ) -> Option<Trade> {
        let entry_bar = nearest_bar(dataset, entry.timestamp)?;
        let exit_bar = nearest_bar(dataset, exit.timestamp)?;
        let fee_per_unit = if self.execution.fixed_size.abs() < f64::EPSILON {
            0.0
        } else {
            self.execution.commission_per_trade / self.execution.fixed_size.abs()
        };
        let entry_intent = OrderIntent {
            timestamp: entry.timestamp,
            direction: entry.side.clone(),
            order_type: OrderType::Market,
            requested_size: self.execution.fixed_size,
            tags: BTreeMap::new(),
        };
        let exit_direction = opposite_direction(&entry.side);
        let exit_intent = OrderIntent {
            timestamp: exit.timestamp,
            direction: exit_direction,
            order_type: OrderType::Market,
            requested_size: self.execution.fixed_size,
            tags: BTreeMap::new(),
        };
        let entry_fill = market_fill(
            &entry_intent,
            entry_bar,
            &self.execution.constraints,
            self.execution.slippage_per_trade,
            fee_per_unit,
        )?;
        let exit_fill = market_fill(
            &exit_intent,
            exit_bar,
            &self.execution.constraints,
            self.execution.slippage_per_trade,
            fee_per_unit,
        )?;
        let size = entry_fill.size.min(exit_fill.size);
        let direction_mult = match &entry.side {
            TradeDirection::Long => 1.0,
            TradeDirection::Short => -1.0,
        };
        let gross = (exit_fill.price - entry_fill.price) * direction_mult * size;
        let fees = entry_fill.fees + exit_fill.fees;
        let pnl = gross - fees;
        Some(Trade {
            trade_id: format!("{}_{}_{}", strategy_id.0, params.id.0, idx),
            strategy_id,
            parameter_set_id: params.id.clone(),
            instrument: dataset.instrument.clone(),
            direction: entry.side.clone(),
            entry_time: entry_fill.timestamp,
            exit_time: exit_fill.timestamp,
            entry_price: entry_fill.price,
            exit_price: exit_fill.price,
            size,
            pnl,
            r_multiple: pnl,
            fees,
            slippage: entry_fill.slippage + exit_fill.slippage,
            tags: BTreeMap::new(),
        })
    }

    fn equity_curve(&self, trades: &[Trade], dataset: &MarketDataset) -> Vec<EquityPoint> {
        let mut points = Vec::new();
        let mut equity = self.execution.initial_equity;
        let mut peak = equity;
        let mut trade_idx = 0usize;
        for bar in &dataset.bars {
            while trade_idx < trades.len() && trades[trade_idx].exit_time <= bar.timestamp {
                equity += trades[trade_idx].pnl;
                peak = peak.max(equity);
                trade_idx += 1;
            }
            let drawdown = if peak <= 0.0 {
                0.0
            } else {
                (peak - equity) / peak
            };
            points.push(EquityPoint {
                timestamp: bar.timestamp,
                equity,
                drawdown,
                underwater: equity < peak,
            });
        }
        points
    }
}

fn opposite_direction(direction: &TradeDirection) -> TradeDirection {
    match direction {
        TradeDirection::Long => TradeDirection::Short,
        TradeDirection::Short => TradeDirection::Long,
    }
}

fn persist_result_if_configured(
    ctx: &PipelineContext,
    run_id: &RunId,
    result: &EvaluationResult,
) -> Result<(), PipelineError> {
    let Some(root) = ctx.bag.get("storage_root").and_then(|v| v.as_str()) else {
        return Ok(());
    };
    let store = JsonlResultStore::new(root);
    store.append_evaluation(run_id, result)?;
    for trade in &result.trades {
        store.append_trade(run_id, trade)?;
    }
    for point in &result.equity_curve {
        store.append_equity(run_id, point)?;
    }
    let edge_stability_ratio = match result.metrics.stability.edge_stability_ratio {
        Some(value) => finite_json(value),
        None => None,
    };
    store.append_metric(
        run_id,
        &serde_json::json!({
            "run_id": run_id.0.clone(),
            "strategy_id": result.strategy_id.0.clone(),
            "parameter_set_id": result.parameter_set_id.0.clone(),
            "total_r": finite_json(result.metrics.classical.total_r),
            "expectancy": finite_json(result.metrics.classical.expectancy),
            "profit_factor": finite_json(result.metrics.classical.profit_factor),
            "max_drawdown": finite_json(result.metrics.classical.max_drawdown),
            "edge_stability_ratio": edge_stability_ratio
        }),
    )?;
    Ok(())
}

fn already_evaluated_ids(ctx: &PipelineContext) -> std::collections::BTreeSet<ParameterSetId> {
    ctx.partial_results
        .iter()
        .map(|result| result.parameter_set_id.clone())
        .collect()
}

fn update_search_state(
    ctx: &mut PipelineContext,
    pending: usize,
    evaluated: usize,
    failed: usize,
    best_score: Option<f64>,
) {
    ctx.bag.insert(
        "search_state".into(),
        serde_json::json!({
            "schema_version": 1,
            "phase": "SparseExploration",
            "pending": pending,
            "evaluated": evaluated,
            "failed": failed,
            "best_score_so_far": best_score
        }),
    );
}

fn sync_runtime_search_state(
    ctx: &mut PipelineContext,
    runtime: &RuntimeSearchState,
) -> Result<(), PipelineError> {
    let progress = runtime.progress();
    ctx.bag.insert(
        "search_state".into(),
        serde_json::json!({
            "schema_version": 1,
            "phase": format!("{:?}", progress.phase),
            "pending": progress.pending,
            "evaluated": progress.evaluated,
            "failed": progress.failed,
            "best_score_so_far": progress.best_score_so_far
        }),
    );
    let runtime_value = serde_json::to_value(runtime).map_err(|e| {
        PipelineError::Search(SearchError::Message {
            code: "SEARCH_RUNTIME_SERIALIZE".into(),
            message: e.to_string(),
            retryable: false,
        })
    })?;
    ctx.bag.insert("search_runtime_state".into(), runtime_value);
    Ok(())
}

fn load_runtime_search_state(
    ctx: &PipelineContext,
) -> Result<Option<RuntimeSearchState>, PipelineError> {
    let Some(value) = ctx.bag.get("search_runtime_state").cloned() else {
        return Ok(None);
    };
    let state = serde_json::from_value(value).map_err(|e| {
        PipelineError::Search(SearchError::Message {
            code: "SEARCH_RUNTIME_PARSE".into(),
            message: e.to_string(),
            retryable: false,
        })
    })?;
    Ok(Some(state))
}

fn persist_search_state_if_configured(
    ctx: &PipelineContext,
    run_id: &RunId,
) -> Result<(), PipelineError> {
    let Some(root) = ctx.bag.get("storage_root").and_then(|v| v.as_str()) else {
        return Ok(());
    };
    let Some(search_state) = ctx.bag.get("search_state") else {
        return Ok(());
    };
    let path = PathBuf::from(root)
        .join("checkpoints")
        .join(&run_id.0)
        .join("search_state.latest.json");
    atomic_write_json(&path, search_state).map_err(PipelineError::Checkpoint)
}

fn finite_json(value: f64) -> Option<f64> {
    if value.is_finite() {
        Some(value)
    } else {
        None
    }
}

fn nearest_bar(dataset: &MarketDataset, timestamp: i64) -> Option<&MarketBar> {
    dataset
        .bars
        .iter()
        .find(|bar| bar.timestamp >= timestamp)
        .or_else(|| dataset.bars.last())
}

async fn execute_runtime_search(
    ctx: &mut PipelineContext,
    registry: &StaticStrategyRegistry,
    engine: &BacktestEngine,
    dataset: &MarketDataset,
    run_id: &RunId,
    seed: u64,
    mut runtime: RuntimeSearchState,
) -> Result<ComponentOutcome, PipelineError> {
    let config = RuntimeSearchConfig::default();
    let all_candidates = ctx
        .candidate_sets
        .values()
        .flat_map(|items| items.iter().cloned())
        .collect::<Vec<_>>();
    let mut failed = runtime.failed.len();
    loop {
        if runtime.is_finished() {
            break;
        }
        let batch = runtime.next_batch(config.batch_size);
        if batch.is_empty() {
            let Some(space) = ctx.parameter_spaces.values().next().cloned() else {
                break;
            };
            runtime.maybe_advance_phase(&space, &all_candidates, &ctx.partial_results, &config)?;
            sync_runtime_search_state(ctx, &runtime)?;
            persist_search_state_if_configured(ctx, run_id)?;
            if runtime.is_finished() {
                break;
            }
            continue;
        }
        for candidate in batch {
            ctx.cancellation.check_cancelled()?;
            ctx.pause.wait_if_paused().await?;
            let plugin = registry.get(&candidate.strategy_id).ok_or_else(|| {
                PipelineError::Strategy(StrategyError::Message {
                    code: "STRATEGY_NOT_FOUND".into(),
                    message: candidate.strategy_id.0.clone(),
                    retryable: false,
                })
            })?;
            match engine.evaluate_plugin(run_id.clone(), plugin.as_ref(), dataset, &candidate, seed)
            {
                Ok(result) => {
                    persist_result_if_configured(ctx, run_id, &result)?;
                    runtime.record_evaluation(&result);
                    ctx.partial_results.push(result);
                }
                Err(err) => {
                    failed += 1;
                    runtime.record_failure(&candidate, err.to_string());
                    sync_runtime_search_state(ctx, &runtime)?;
                    persist_search_state_if_configured(ctx, run_id)?;
                    return Err(err);
                }
            }
            sync_runtime_search_state(ctx, &runtime)?;
            persist_search_state_if_configured(ctx, run_id)?;
            let progress = runtime.progress();
            if let Some(sink) = &ctx.progress {
                sink.publish(ProgressEvent {
                    schema_version: 1,
                    run_id: run_id.clone(),
                    stage: "SearchScheduler".into(),
                    status: RunStatus::Running,
                    worker_id: None,
                    current: progress.evaluated as u64,
                    total: Some(progress.total as u64),
                    percent: if progress.total == 0 {
                        Some(100.0)
                    } else {
                        Some(progress.evaluated as f64 * 100.0 / progress.total as f64)
                    },
                    best_score_so_far: progress.best_score_so_far,
                    message: format!("phase {:?}, failed {failed}", progress.phase),
                    error: None,
                    timestamp: chrono::Utc::now().timestamp_millis(),
                });
            }
        }
        let Some(space) = ctx.parameter_spaces.values().next().cloned() else {
            continue;
        };
        runtime.maybe_advance_phase(&space, &all_candidates, &ctx.partial_results, &config)?;
        sync_runtime_search_state(ctx, &runtime)?;
        persist_search_state_if_configured(ctx, run_id)?;
    }
    Ok(ComponentOutcome {
        message: format!("{} evaluations available", ctx.partial_results.len()),
    })
}

pub struct StrategyRunnerComponent {
    pub registry: StaticStrategyRegistry,
    pub execution: ExecutionModel,
}

impl StrategyRunnerComponent {
    pub fn new(registry: StaticStrategyRegistry, execution: ExecutionModel) -> Self {
        Self {
            registry,
            execution,
        }
    }
}

#[async_trait]
impl PipelineComponent for StrategyRunnerComponent {
    fn id(&self) -> ComponentId {
        ComponentId("StrategyRunner".into())
    }
    fn name(&self) -> &'static str {
        "StrategyRunner"
    }
    fn version(&self) -> ComponentVersion {
        ComponentVersion {
            semver: "0.1.0".into(),
        }
    }
    fn input_contract(&self) -> Vec<DataContract> {
        vec![
            DataContract::CandidateParameterSets,
            DataContract::NormalizedDataset,
        ]
    }
    fn output_contract(&self) -> Vec<DataContract> {
        vec![DataContract::EvaluationResults]
    }
    fn failure_policy(&self) -> FailurePolicy {
        FailurePolicy::AbortRun
    }
    async fn execute(&self, ctx: &mut PipelineContext) -> Result<ComponentOutcome, PipelineError> {
        let run_id = ctx.run_id.clone().ok_or_else(|| PipelineError::Invariant {
            message: "run_id missing".into(),
        })?;
        let seed = match ctx.run_config.as_ref().map(|c| c.seed) {
            Some(value) => value,
            None => 0,
        };
        let dataset = ctx.datasets.values().next().cloned().ok_or_else(|| {
            PipelineError::Data(DataError::Message {
                code: "NO_DATASET".into(),
                message: "no normalized dataset available".into(),
                retryable: false,
            })
        })?;
        let engine = BacktestEngine::new(self.execution.clone());
        if let Some(runtime) = load_runtime_search_state(ctx)? {
            return execute_runtime_search(
                ctx,
                &self.registry,
                &engine,
                &dataset,
                &run_id,
                seed,
                runtime,
            )
            .await;
        }
        let total: usize = ctx.candidate_sets.values().map(Vec::len).sum();
        let mut evaluated_ids = already_evaluated_ids(ctx);
        let mut current = evaluated_ids.len() as u64;
        let mut failed = 0usize;
        let mut best_score = ctx
            .partial_results
            .iter()
            .map(|result| result.metrics.classical.expectancy)
            .filter(|value| value.is_finite())
            .max_by(|a, b| match a.partial_cmp(b) {
                Some(ordering) => ordering,
                None => std::cmp::Ordering::Equal,
            });
        update_search_state(
            ctx,
            total.saturating_sub(current as usize),
            current as usize,
            failed,
            best_score,
        );
        persist_search_state_if_configured(ctx, &run_id)?;
        for (strategy_id, candidates) in ctx.candidate_sets.clone() {
            let plugin = self.registry.get(&strategy_id).ok_or_else(|| {
                PipelineError::Strategy(StrategyError::Message {
                    code: "STRATEGY_NOT_FOUND".into(),
                    message: strategy_id.0.clone(),
                    retryable: false,
                })
            })?;
            for candidate in candidates {
                ctx.cancellation.check_cancelled()?;
                ctx.pause.wait_if_paused().await?;
                if evaluated_ids.contains(&candidate.id) {
                    continue;
                }
                current += 1;
                match engine.evaluate_plugin(
                    run_id.clone(),
                    plugin.as_ref(),
                    &dataset,
                    &candidate,
                    seed,
                ) {
                    Ok(result) => {
                        persist_result_if_configured(ctx, &run_id, &result)?;
                        let score = result.metrics.classical.expectancy;
                        if score.is_finite() {
                            best_score = match best_score {
                                Some(best) if best >= score => Some(best),
                                _ => Some(score),
                            };
                        }
                        evaluated_ids.insert(candidate.id.clone());
                        ctx.partial_results.push(result);
                        update_search_state(
                            ctx,
                            total.saturating_sub(current as usize),
                            current as usize,
                            failed,
                            best_score,
                        );
                        persist_search_state_if_configured(ctx, &run_id)?;
                        if let Some(sink) = &ctx.progress {
                            sink.publish(ProgressEvent {
                                schema_version: 1,
                                run_id: run_id.clone(),
                                stage: "StrategyRunner".into(),
                                status: RunStatus::Running,
                                worker_id: None,
                                current,
                                total: Some(total as u64),
                                percent: if total == 0 {
                                    Some(100.0)
                                } else {
                                    Some(current as f64 * 100.0 / total as f64)
                                },
                                best_score_so_far: best_score,
                                message: "evaluated candidate".into(),
                                error: None,
                                timestamp: chrono::Utc::now().timestamp_millis(),
                            });
                        }
                    }
                    Err(err) => {
                        failed += 1;
                        update_search_state(
                            ctx,
                            total.saturating_sub(current as usize),
                            current as usize,
                            failed,
                            best_score,
                        );
                        persist_search_state_if_configured(ctx, &run_id)?;
                        return Err(err);
                    }
                }
            }
        }
        Ok(ComponentOutcome {
            message: format!("{} evaluations available", ctx.partial_results.len()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nearest_bar_handles_empty_dataset() {
        let dataset = MarketDataset {
            dataset_id: DatasetId("d".into()),
            schema_version: 1,
            instrument: Instrument {
                symbol: "X".into(),
                venue: None,
                asset_class: None,
            },
            timeframe: Timeframe {
                name: "1m".into(),
                seconds: 60,
            },
            timezone: "UTC".into(),
            bars: vec![],
            metadata: DatasetMetadata {
                source_uri: None,
                checksum: None,
                created_at: 0,
                normalization_version: "test".into(),
                extra: BTreeMap::new(),
            },
            quality: None,
        };
        assert!(nearest_bar(&dataset, 0).is_none());
    }
}
