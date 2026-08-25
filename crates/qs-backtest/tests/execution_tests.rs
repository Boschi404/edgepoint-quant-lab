use qs_backtest::{limit_fill, market_fill, normalize_size, round_to_tick, ExecutionConstraints, OrderIntent, OrderType};
use qs_core::*;
use std::collections::BTreeMap;

fn bar() -> MarketBar {
    MarketBar { timestamp: 1, open: 100.0, high: 101.0, low: 99.0, close: 100.5, volume: None, spread: None, extra: BTreeMap::new() }
}

#[test]
fn size_normalization_respects_lot_step() {
    let constraints = ExecutionConstraints { min_size: 0.1, max_size: 10.0, lot_step: 0.1, tick_size: 0.01, allow_partial_fills: true };
    assert_eq!(normalize_size(1.26, &constraints), Some(1.2));
}

#[test]
fn tick_rounding_is_deterministic() {
    let constraints = ExecutionConstraints { min_size: 0.1, max_size: 10.0, lot_step: 0.1, tick_size: 0.25, allow_partial_fills: true };
    assert_eq!(round_to_tick(100.37, &constraints), 100.25);
}

#[test]
fn market_order_fills_on_close_with_slippage() {
    let constraints = ExecutionConstraints::default();
    let intent = OrderIntent { timestamp: 1, direction: TradeDirection::Long, order_type: OrderType::Market, requested_size: 1.0, tags: BTreeMap::new() };
    let fill = match market_fill(&intent, &bar(), &constraints, 0.1, 0.0) { Some(value) => value, None => panic!("expected fill") };
    assert!(fill.price > 100.5);
}

#[test]
fn limit_order_respects_bar_range() {
    let constraints = ExecutionConstraints::default();
    let intent = OrderIntent { timestamp: 1, direction: TradeDirection::Long, order_type: OrderType::Limit { price: 99.5 }, requested_size: 1.0, tags: BTreeMap::new() };
    assert!(limit_fill(&intent, &bar(), &constraints, 0.0).is_some());
}
