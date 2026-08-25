pub mod registry;
pub use registry::*;

use qs_core::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StrategyMetadata {
    pub strategy_id: StrategyId,
    pub version: String,
    pub name: String,
    pub plugin_checksum: Option<String>,
    pub contract_version: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SignalEvent {
    pub timestamp: i64,
    pub side: TradeDirection,
    pub strength: Option<f64>,
    pub tags: BTreeMap<String, ScalarValue>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StrategyRunInput { pub dataset: MarketDataset, pub parameters: ParameterSet, pub seed: u64 }
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StrategyRunOutput { pub signals: Vec<SignalEvent>, pub diagnostics: BTreeMap<String, ScalarValue> }

pub trait StrategyPlugin: Send + Sync {
    fn metadata(&self) -> StrategyMetadata;
    fn parameter_space(&self) -> ParameterSpace;
    fn validate_parameters(&self, params: &ParameterSet) -> Result<(), StrategyError>;
    fn run(&self, input: StrategyRunInput) -> Result<StrategyRunOutput, StrategyError>;
}
