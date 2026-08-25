pub mod adapters;
pub mod normalizers;

use adapters::csv_adapter::CsvRawDataAdapter;
use async_trait::async_trait;
use normalizers::{OhlcvColumnMapping, OhlcvJsonNormalizer};
use qs_core::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DataSource { pub uri: String, pub format_hint: Option<String> }
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RawSchema { pub fields: Vec<String> }
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RawRecord { pub fields: serde_json::Value }
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RawDataset { pub source: DataSource, pub schema: RawSchema, pub records: Vec<RawRecord> }
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NormalizationConfig { pub timezone: String, pub timestamp_unit: String }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfiguredDataset {
    pub dataset_id: DatasetId,
    pub source: DataSource,
    pub normalization: NormalizationConfig,
    pub mapping: OhlcvColumnMapping,
}

pub trait RawDataAdapter: Send + Sync {
    fn adapter_id(&self) -> String;
    fn detect(&self, source: &DataSource) -> Result<bool, DataError>;
    fn read_schema(&self, source: &DataSource) -> Result<RawSchema, DataError>;
    fn read_records(&self, source: &DataSource) -> Result<Box<dyn Iterator<Item = Result<RawRecord, DataError>>>, DataError>;
}

pub trait Normalizer: Send + Sync {
    fn normalize(&self, raw: RawDataset, config: NormalizationConfig) -> Result<MarketDataset, DataError>;
}

pub struct DataIngestionComponent;

#[async_trait]
impl PipelineComponent for DataIngestionComponent {
    fn id(&self) -> ComponentId { ComponentId("DataIngestion".into()) }
    fn name(&self) -> &'static str { "DataIngestion" }
    fn version(&self) -> ComponentVersion { ComponentVersion { semver: "0.1.0".into() } }
    fn input_contract(&self) -> Vec<DataContract> { vec![] }
    fn output_contract(&self) -> Vec<DataContract> { vec![DataContract::RawDatasetReference] }
    fn failure_policy(&self) -> FailurePolicy { FailurePolicy::AbortRun }
    async fn execute(&self, ctx: &mut PipelineContext) -> Result<ComponentOutcome, PipelineError> {
        let Some(value) = ctx.bag.get("dataset_configs").cloned() else {
            ctx.bag.insert("raw_datasets".into(), serde_json::json!([]));
            return Ok(ComponentOutcome { message: "no dataset configs supplied; raw dataset list is empty".into() });
        };
        let configs: Vec<ConfiguredDataset> = serde_json::from_value(value).map_err(|e| PipelineError::Data(DataError::Message { code: "DATASET_CONFIG_PARSE".into(), message: e.to_string(), retryable: false }))?;
        let csv = CsvRawDataAdapter;
        let mut raw_datasets = Vec::new();
        for config in configs {
            if !csv.detect(&config.source)? {
                return Err(PipelineError::Data(DataError::Message { code: "NO_ADAPTER".into(), message: format!("no adapter for {}", config.source.uri), retryable: false }));
            }
            let schema = csv.read_schema(&config.source)?;
            let mut records = Vec::new();
            for record in csv.read_records(&config.source)? { records.push(record?); }
            raw_datasets.push(RawDataset { source: config.source, schema, records });
        }
        ctx.bag.insert("raw_datasets".into(), serde_json::to_value(raw_datasets).map_err(|e| PipelineError::Data(DataError::Message { code: "RAW_DATASETS_SERIALIZE".into(), message: e.to_string(), retryable: false }))?);
        Ok(ComponentOutcome { message: "raw datasets ingested".into() })
    }
}

pub struct DataNormalizerComponent;

#[async_trait]
impl PipelineComponent for DataNormalizerComponent {
    fn id(&self) -> ComponentId { ComponentId("DataNormalizer".into()) }
    fn name(&self) -> &'static str { "DataNormalizer" }
    fn version(&self) -> ComponentVersion { ComponentVersion { semver: "0.1.0".into() } }
    fn input_contract(&self) -> Vec<DataContract> { vec![DataContract::RawDatasetReference] }
    fn output_contract(&self) -> Vec<DataContract> { vec![DataContract::NormalizedDataset] }
    fn failure_policy(&self) -> FailurePolicy { FailurePolicy::AbortRun }
    async fn execute(&self, ctx: &mut PipelineContext) -> Result<ComponentOutcome, PipelineError> {
        let configs_value = match ctx.bag.get("dataset_configs").cloned() { Some(value) => value, None => serde_json::json!([]) };
        let raw_value = match ctx.bag.get("raw_datasets").cloned() { Some(value) => value, None => serde_json::json!([]) };
        let configs: Vec<ConfiguredDataset> = serde_json::from_value(configs_value).map_err(|e| PipelineError::Data(DataError::Message { code: "DATASET_CONFIG_PARSE".into(), message: e.to_string(), retryable: false }))?;
        let raws: Vec<RawDataset> = serde_json::from_value(raw_value).map_err(|e| PipelineError::Data(DataError::Message { code: "RAW_DATASETS_PARSE".into(), message: e.to_string(), retryable: false }))?;
        for (config, raw) in configs.into_iter().zip(raws.into_iter()) {
            let normalizer = OhlcvJsonNormalizer { mapping: config.mapping };
            let dataset = normalizer.normalize(raw, config.normalization)?;
            ctx.datasets.insert(dataset.dataset_id.clone(), dataset);
        }
        Ok(ComponentOutcome { message: format!("{} datasets normalized", ctx.datasets.len()) })
    }
}

pub struct DataQualityGateComponent { pub policy: DataQualityPolicy }

#[async_trait]
impl PipelineComponent for DataQualityGateComponent {
    fn id(&self) -> ComponentId { ComponentId("DataQualityGate".into()) }
    fn name(&self) -> &'static str { "DataQualityGate" }
    fn version(&self) -> ComponentVersion { ComponentVersion { semver: "0.1.0".into() } }
    fn input_contract(&self) -> Vec<DataContract> { vec![DataContract::NormalizedDataset] }
    fn output_contract(&self) -> Vec<DataContract> { vec![DataContract::QualityReport] }
    fn failure_policy(&self) -> FailurePolicy { FailurePolicy::AbortRun }
    async fn execute(&self, ctx: &mut PipelineContext) -> Result<ComponentOutcome, PipelineError> {
        for dataset in ctx.datasets.values_mut() {
            let report = run_quality_checks(dataset, &self.policy)?;
            dataset.quality = Some(report);
        }
        Ok(ComponentOutcome { message: "quality checks completed".into() })
    }
}

pub fn run_quality_checks(dataset: &MarketDataset, policy: &DataQualityPolicy) -> Result<DataQualityReport, DataError> {
    let mut checks = Vec::new();
    let mut failed = false;
    let mut warning = false;
    let mut prev_timestamp = None;
    let mut gap_rows = Vec::new();
    let mut outlier_rows = Vec::new();
    let expected_step_ms = (dataset.timeframe.seconds as i64).saturating_mul(1000);

    for (idx, bar) in dataset.bars.iter().enumerate() {
        if let Some(previous) = prev_timestamp {
            if bar.timestamp <= previous {
                failed = true;
                checks.push(DataQualityCheckResult { check_name: "timestamp_monotonic".into(), passed: false, severity: QualitySeverity::Error, message: "non-monotonic timestamp".into(), affected_rows: vec![idx] });
            }
            if expected_step_ms > 0 && bar.timestamp - previous > expected_step_ms.saturating_mul(2) {
                warning = true;
                gap_rows.push(idx);
            }
        }
        if !(bar.high >= bar.low && bar.high >= bar.open && bar.high >= bar.close && bar.low <= bar.open && bar.low <= bar.close) {
            failed = true;
            checks.push(DataQualityCheckResult { check_name: "ohlc_consistency".into(), passed: false, severity: QualitySeverity::Error, message: "OHLC invariant violated".into(), affected_rows: vec![idx] });
        }
        if bar.open <= 0.0 || bar.high <= 0.0 || bar.low <= 0.0 || bar.close <= 0.0 {
            warning = true;
            outlier_rows.push(idx);
        }
        if let Some(spread) = bar.spread {
            if spread < 0.0 {
                failed = true;
                checks.push(DataQualityCheckResult { check_name: "negative_spread".into(), passed: false, severity: QualitySeverity::Error, message: "negative spread".into(), affected_rows: vec![idx] });
            }
        }
        prev_timestamp = Some(bar.timestamp);
    }

    if !gap_rows.is_empty() {
        checks.push(DataQualityCheckResult { check_name: "gap_detection".into(), passed: false, severity: QualitySeverity::Warning, message: "detected timestamp gaps larger than configured timeframe tolerance".into(), affected_rows: gap_rows });
    }
    if !outlier_rows.is_empty() {
        checks.push(DataQualityCheckResult { check_name: "outlier_detection".into(), passed: false, severity: QualitySeverity::Warning, message: "detected non-positive OHLC values".into(), affected_rows: outlier_rows });
    }
    if checks.is_empty() {
        checks.push(DataQualityCheckResult { check_name: "basic_checks".into(), passed: true, severity: QualitySeverity::Info, message: "basic checks passed".into(), affected_rows: vec![] });
    }
    if failed && matches!(policy, DataQualityPolicy::Block) {
        return Err(DataError::Message { code: "DATA_QUALITY_BLOCKED".into(), message: "dataset failed quality gate".into(), retryable: false });
    }
    let status = if failed { DataQualityStatus::Failed } else if warning { DataQualityStatus::PassedWithWarnings } else { DataQualityStatus::Passed };
    Ok(DataQualityReport { dataset_id: dataset.dataset_id.clone(), schema_version: 1, status, checks, fixes_applied: vec![], generated_at: chrono::Utc::now().timestamp_millis() })
}
