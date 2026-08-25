use qs_core::*;
use qs_data::run_quality_checks;
use std::collections::BTreeMap;

fn dataset(bars: Vec<MarketBar>) -> MarketDataset {
    MarketDataset {
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
        bars,
        metadata: DatasetMetadata {
            source_uri: None,
            checksum: None,
            created_at: 0,
            normalization_version: "test".into(),
            extra: BTreeMap::new(),
        },
        quality: None,
    }
}

fn bar(ts: i64, close: f64) -> MarketBar {
    MarketBar {
        timestamp: ts,
        open: close,
        high: close + 1.0,
        low: close - 1.0,
        close,
        volume: None,
        spread: Some(0.01),
        extra: BTreeMap::new(),
    }
}

#[test]
fn quality_gate_passes_clean_data() {
    let report = match run_quality_checks(
        &dataset(vec![bar(1_000, 10.0), bar(61_000, 11.0)]),
        &DataQualityPolicy::Block,
    ) {
        Ok(value) => value,
        Err(err) => panic!("quality: {err}"),
    };
    assert!(matches!(report.status, DataQualityStatus::Passed));
}

#[test]
fn quality_gate_blocks_non_monotonic_data() {
    let err = run_quality_checks(
        &dataset(vec![bar(61_000, 10.0), bar(1_000, 11.0)]),
        &DataQualityPolicy::Block,
    )
    .err();
    assert!(err.is_some());
}
