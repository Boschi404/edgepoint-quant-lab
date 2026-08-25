use crate::{NormalizationConfig, Normalizer, RawDataset};
use qs_core::*;
use std::collections::BTreeMap;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct OhlcvColumnMapping {
    pub timestamp: String,
    pub open: String,
    pub high: String,
    pub low: String,
    pub close: String,
    pub volume: Option<String>,
    pub spread: Option<String>,
    pub instrument: Instrument,
    pub timeframe: Timeframe,
    pub dataset_id: DatasetId,
}

pub struct OhlcvJsonNormalizer {
    pub mapping: OhlcvColumnMapping,
}

impl Normalizer for OhlcvJsonNormalizer {
    fn normalize(
        &self,
        raw: RawDataset,
        config: NormalizationConfig,
    ) -> Result<MarketDataset, DataError> {
        let mut bars = Vec::with_capacity(raw.records.len());
        for (idx, row) in raw.records.iter().enumerate() {
            let object = row.fields.as_object().ok_or_else(|| {
                data_error(
                    "NORMALIZE_ROW",
                    format!("row {idx} is not an object"),
                    false,
                )
            })?;
            bars.push(MarketBar {
                timestamp: parse_i64(
                    object.get(&self.mapping.timestamp),
                    idx,
                    &self.mapping.timestamp,
                )?,
                open: parse_f64(object.get(&self.mapping.open), idx, &self.mapping.open)?,
                high: parse_f64(object.get(&self.mapping.high), idx, &self.mapping.high)?,
                low: parse_f64(object.get(&self.mapping.low), idx, &self.mapping.low)?,
                close: parse_f64(object.get(&self.mapping.close), idx, &self.mapping.close)?,
                volume: parse_optional_f64(&self.mapping.volume, object, idx)?,
                spread: parse_optional_f64(&self.mapping.spread, object, idx)?,
                extra: BTreeMap::new(),
            });
        }
        bars.sort_by_key(|b| b.timestamp);
        Ok(MarketDataset {
            dataset_id: self.mapping.dataset_id.clone(),
            schema_version: 1,
            instrument: self.mapping.instrument.clone(),
            timeframe: self.mapping.timeframe.clone(),
            timezone: config.timezone,
            bars,
            metadata: DatasetMetadata {
                source_uri: Some(raw.source.uri),
                checksum: None,
                created_at: chrono::Utc::now().timestamp_millis(),
                normalization_version: "0.1.0".into(),
                extra: BTreeMap::new(),
            },
            quality: None,
        })
    }
}

fn parse_i64(value: Option<&serde_json::Value>, row: usize, col: &str) -> Result<i64, DataError> {
    match value {
        Some(serde_json::Value::Number(n)) => n.as_i64().ok_or_else(|| {
            data_error(
                "NORMALIZE_I64",
                format!("row {row} col {col} is not i64"),
                false,
            )
        }),
        Some(serde_json::Value::String(s)) => s.parse::<i64>().map_err(|e| {
            data_error(
                "NORMALIZE_I64_PARSE",
                format!("row {row} col {col}: {e}"),
                false,
            )
        }),
        _ => Err(data_error(
            "NORMALIZE_MISSING_I64",
            format!("row {row} missing {col}"),
            false,
        )),
    }
}

fn parse_f64(value: Option<&serde_json::Value>, row: usize, col: &str) -> Result<f64, DataError> {
    match value {
        Some(serde_json::Value::Number(n)) => n.as_f64().ok_or_else(|| {
            data_error(
                "NORMALIZE_F64",
                format!("row {row} col {col} is not f64"),
                false,
            )
        }),
        Some(serde_json::Value::String(s)) => s.parse::<f64>().map_err(|e| {
            data_error(
                "NORMALIZE_F64_PARSE",
                format!("row {row} col {col}: {e}"),
                false,
            )
        }),
        _ => Err(data_error(
            "NORMALIZE_MISSING_F64",
            format!("row {row} missing {col}"),
            false,
        )),
    }
}

fn parse_optional_f64(
    name: &Option<String>,
    object: &serde_json::Map<String, serde_json::Value>,
    row: usize,
) -> Result<Option<f64>, DataError> {
    let Some(col) = name else {
        return Ok(None);
    };
    parse_f64(object.get(col), row, col).map(Some)
}

fn data_error(code: &str, message: String, retryable: bool) -> DataError {
    DataError::Message {
        code: code.into(),
        message,
        retryable,
    }
}
