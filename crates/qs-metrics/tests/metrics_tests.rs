use qs_core::*;
use qs_metrics::classical_metrics;

#[test]
fn empty_metrics_are_finite() {
    let metrics = classical_metrics(&[], &[]);
    assert!(metrics.profit_factor.is_finite());
    assert_eq!(metrics.total_r, 0.0);
}
