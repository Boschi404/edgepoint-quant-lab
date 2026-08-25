use crate::ids::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ScalarValue { Int(i64), Float(f64), Bool(bool), Text(String) }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Instrument { pub symbol: String, pub venue: Option<String>, pub asset_class: Option<String> }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Timeframe { pub name: String, pub seconds: u64 }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DatasetMetadata {
    pub source_uri: Option<String>,
    pub checksum: Option<String>,
    pub created_at: i64,
    pub normalization_version: String,
    pub extra: BTreeMap<String, ScalarValue>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MarketDataset {
    pub dataset_id: DatasetId,
    pub schema_version: u32,
    pub instrument: Instrument,
    pub timeframe: Timeframe,
    pub timezone: String,
    pub bars: Vec<MarketBar>,
    pub metadata: DatasetMetadata,
    pub quality: Option<DataQualityReport>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MarketBar {
    pub timestamp: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: Option<f64>,
    pub spread: Option<f64>,
    pub extra: BTreeMap<String, ScalarValue>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DataQualityReport {
    pub dataset_id: DatasetId,
    pub schema_version: u32,
    pub status: DataQualityStatus,
    pub checks: Vec<DataQualityCheckResult>,
    pub fixes_applied: Vec<DataFixLog>,
    pub generated_at: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DataQualityStatus { Passed, PassedWithWarnings, Failed }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DataQualityCheckResult {
    pub check_name: String,
    pub passed: bool,
    pub severity: QualitySeverity,
    pub message: String,
    pub affected_rows: Vec<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum QualitySeverity { Info, Warning, Error }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DataFixLog { pub fix_name: String, pub message: String, pub affected_rows: Vec<usize> }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DataQualityPolicy { Block, Warn, AutoFixAndLog }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParameterSet {
    pub id: ParameterSetId,
    pub strategy_id: StrategyId,
    pub values: BTreeMap<String, ParameterValue>,
    pub source: ParameterSetSource,
    pub parent_ids: Vec<ParameterSetId>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ParameterValue { Int(i64), Float(f64), Bool(bool), Enum(String), Text(String) }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ParameterSetSource { Grid, RandomSparse, LatinHypercube, BayesianSuggestion, NeighborhoodExpansion, ManualSeed, ResumeRecovered }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParameterSpace {
    pub strategy_id: StrategyId,
    pub parameters: Vec<ParameterDefinition>,
    pub constraints: Vec<ParameterConstraint>,
    pub neighborhood: NeighborhoodDefinition,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParameterDefinition { pub name: String, pub kind: ParameterKind, pub default: Option<ParameterValue> }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ParameterKind {
    IntRange { min: i64, max: i64, step: i64 },
    FloatRange { min: f64, max: f64, step: Option<f64>, scale: NumericScale },
    Bool,
    Enum { values: Vec<String> },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NumericScale { Linear, Log }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParameterConstraint { pub expression: String }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NeighborhoodDefinition { pub metric: String, pub weights: BTreeMap<String, f64> }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TradeDirection { Long, Short }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Trade {
    pub trade_id: String,
    pub strategy_id: StrategyId,
    pub parameter_set_id: ParameterSetId,
    pub instrument: Instrument,
    pub direction: TradeDirection,
    pub entry_time: i64,
    pub exit_time: i64,
    pub entry_price: f64,
    pub exit_price: f64,
    pub size: f64,
    pub pnl: f64,
    pub r_multiple: f64,
    pub fees: f64,
    pub slippage: f64,
    pub tags: BTreeMap<String, ScalarValue>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EquityPoint { pub timestamp: i64, pub equity: f64, pub drawdown: f64, pub underwater: bool }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EvaluationResult {
    pub run_id: RunId,
    pub strategy_id: StrategyId,
    pub parameter_set_id: ParameterSetId,
    pub dataset_id: DatasetId,
    pub trades: Vec<Trade>,
    pub equity_curve: Vec<EquityPoint>,
    pub metrics: MetricBundle,
    pub diagnostics: EvaluationDiagnostics,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct EvaluationDiagnostics { pub warnings: Vec<String>, pub extra: BTreeMap<String, ScalarValue> }

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct MetricBundle {
    pub classical: ClassicalMetrics,
    pub stability: StabilityMetrics,
    pub regime: Option<RegimeMetrics>,
    pub rolling: Option<RollingMetrics>,
    pub stress: Option<StressMetrics>,
    pub custom: BTreeMap<String, f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ClassicalMetrics {
    pub total_r: f64,
    pub average_r: f64,
    pub expectancy: f64,
    pub winrate: f64,
    pub profit_factor: f64,
    pub max_drawdown: f64,
    pub sharpe: Option<f64>,
    pub sortino: Option<f64>,
    pub calmar: Option<f64>,
    pub z_score: Option<f64>,
    pub lr_correlation: Option<f64>,
    pub max_consecutive_losses: u32,
    pub recovery_factor: Option<f64>,
    pub average_trade_duration_secs: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct StabilityMetrics {
    pub trade_variance: Option<f64>,
    pub trade_std: Option<f64>,
    pub rolling_average_r: Vec<RollingPoint>,
    pub rolling_profit_factor: Vec<RollingPoint>,
    pub edge_stability_ratio: Option<f64>,
    pub inter_regime_variance: Option<f64>,
    pub crisis_window_performance: Vec<CrisisWindowMetric>,
    pub pnl_autocorrelation: Option<f64>,
    pub ulcer_index: Option<f64>,
    pub underwater_time_ratio: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RollingPoint { pub timestamp: i64, pub value: f64 }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CrisisWindowMetric { pub window_id: String, pub start: i64, pub end: i64, pub total_r: f64, pub max_drawdown: f64 }

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct RegimeMetrics { pub per_regime: BTreeMap<String, MetricBundleLite> }

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct RollingMetrics { pub points: BTreeMap<String, Vec<RollingPoint>> }

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct StressMetrics { pub scenarios: BTreeMap<String, MetricBundleLite> }

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct MetricBundleLite { pub total_r: f64, pub profit_factor: f64, pub max_drawdown: f64, pub trade_count: u64 }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DataContract {
    RawDatasetReference,
    NormalizedDataset,
    QualityReport,
    ParameterSpace,
    CandidateParameterSets,
    EvaluationResults,
    ValidationResults,
    RankingResults,
    LiveExportArtifacts,
    ReportArtifacts,
}
