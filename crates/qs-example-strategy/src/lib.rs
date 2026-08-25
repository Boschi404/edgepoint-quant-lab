use qs_core::*;
use qs_strategy_api::*;
use std::collections::BTreeMap;

/// Minimal static plugin used for integration tests and wiring validation.
/// It is deliberately simple and not intended as a profitable trading strategy.
pub struct MovingAverageToyStrategy;

impl StrategyPlugin for MovingAverageToyStrategy {
    fn metadata(&self) -> StrategyMetadata {
        StrategyMetadata {
            strategy_id: StrategyId("moving_average_toy".into()),
            version: "0.1.0".into(),
            name: "Moving Average Toy Strategy".into(),
            plugin_checksum: None,
            contract_version: 1,
        }
    }

    fn parameter_space(&self) -> ParameterSpace {
        ParameterSpace {
            strategy_id: self.metadata().strategy_id,
            parameters: vec![
                ParameterDefinition { name: "lookback".into(), kind: ParameterKind::IntRange { min: 5, max: 50, step: 5 }, default: Some(ParameterValue::Int(20)) },
            ],
            constraints: vec![],
            neighborhood: NeighborhoodDefinition { metric: "normalized_mixed".into(), weights: BTreeMap::new() },
        }
    }

    fn validate_parameters(&self, params: &ParameterSet) -> Result<(), StrategyError> {
        match params.values.get("lookback") {
            Some(ParameterValue::Int(v)) if *v >= 2 => Ok(()),
            _ => Err(StrategyError::Message { code: "INVALID_LOOKBACK".into(), message: "lookback must be an integer >= 2".into(), retryable: false }),
        }
    }

    fn run(&self, input: StrategyRunInput) -> Result<StrategyRunOutput, StrategyError> {
        self.validate_parameters(&input.parameters)?;
        let lookback = match input.parameters.values.get("lookback") { Some(ParameterValue::Int(v)) => *v as usize, _ => 20 };
        let mut signals = Vec::new();
        let bars = &input.dataset.bars;
        if bars.len() <= lookback { return Ok(StrategyRunOutput { signals, diagnostics: BTreeMap::new() }); }
        let mut last_above = false;
        for idx in lookback..bars.len() {
            let avg = bars[idx - lookback..idx].iter().map(|b| b.close).sum::<f64>() / lookback as f64;
            let above = bars[idx].close > avg;
            if above != last_above {
                signals.push(SignalEvent { timestamp: bars[idx].timestamp, side: if above { TradeDirection::Long } else { TradeDirection::Short }, strength: None, tags: BTreeMap::new() });
                last_above = above;
            }
        }
        Ok(StrategyRunOutput { signals, diagnostics: BTreeMap::new() })
    }
}
