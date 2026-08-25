use qs_core::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum OrderType {
    Market,
    Limit { price: f64 },
    Stop { price: f64 },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OrderIntent {
    pub timestamp: i64,
    pub direction: TradeDirection,
    pub order_type: OrderType,
    pub requested_size: f64,
    pub tags: BTreeMap<String, ScalarValue>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Fill {
    pub timestamp: i64,
    pub direction: TradeDirection,
    pub price: f64,
    pub size: f64,
    pub fees: f64,
    pub slippage: f64,
    pub partial: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PositionState {
    pub direction: TradeDirection,
    pub average_price: f64,
    pub size: f64,
    pub opened_at: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecutionConstraints {
    pub min_size: f64,
    pub max_size: f64,
    pub lot_step: f64,
    pub tick_size: f64,
    pub allow_partial_fills: bool,
}

impl Default for ExecutionConstraints {
    fn default() -> Self {
        Self {
            min_size: 0.01,
            max_size: 100.0,
            lot_step: 0.01,
            tick_size: 0.00001,
            allow_partial_fills: true,
        }
    }
}

/// Remove floating-point noise from a value snapped to a decimal grid
/// (e.g. `steps * lot_step`), so results match their intended decimal value
/// (12 * 0.1 -> exactly 1.2). No-op when the value is already clean or too
/// large/small for the 1e-9 relative guard to apply.
fn snap_to_grid(value: f64) -> f64 {
    let snapped = (value * 1e9).round() / 1e9;
    if (snapped - value).abs() <= value.abs().max(1.0) * 1e-9 {
        snapped
    } else {
        value
    }
}

pub fn normalize_size(size: f64, constraints: &ExecutionConstraints) -> Option<f64> {
    if !size.is_finite() || size < constraints.min_size {
        return None;
    }
    let clamped = size.min(constraints.max_size);
    let steps = (clamped / constraints.lot_step).floor();
    let normalized = snap_to_grid(steps * constraints.lot_step);
    if normalized < constraints.min_size {
        None
    } else {
        Some(normalized)
    }
}

pub fn round_to_tick(price: f64, constraints: &ExecutionConstraints) -> f64 {
    if constraints.tick_size <= 0.0 {
        return price;
    }
    snap_to_grid((price / constraints.tick_size).round() * constraints.tick_size)
}

pub fn market_fill(
    intent: &OrderIntent,
    bar: &MarketBar,
    constraints: &ExecutionConstraints,
    slippage: f64,
    fee_per_unit: f64,
) -> Option<Fill> {
    let size = normalize_size(intent.requested_size, constraints)?;
    let direction_mult = match &intent.direction {
        TradeDirection::Long => 1.0,
        TradeDirection::Short => -1.0,
    };
    let price = round_to_tick(bar.close + slippage * direction_mult, constraints);
    Some(Fill {
        timestamp: bar.timestamp,
        direction: intent.direction.clone(),
        price,
        size,
        fees: fee_per_unit.abs() * size,
        slippage: slippage.abs(),
        partial: false,
    })
}

pub fn limit_fill(
    intent: &OrderIntent,
    bar: &MarketBar,
    constraints: &ExecutionConstraints,
    fee_per_unit: f64,
) -> Option<Fill> {
    let price = match &intent.order_type {
        OrderType::Limit { price } => *price,
        _ => return None,
    };
    let fillable = match &intent.direction {
        TradeDirection::Long => bar.low <= price,
        TradeDirection::Short => bar.high >= price,
    };
    if !fillable {
        return None;
    }
    let size = normalize_size(intent.requested_size, constraints)?;
    Some(Fill {
        timestamp: bar.timestamp,
        direction: intent.direction.clone(),
        price: round_to_tick(price, constraints),
        size,
        fees: fee_per_unit.abs() * size,
        slippage: 0.0,
        partial: false,
    })
}

pub fn stop_fill(
    intent: &OrderIntent,
    bar: &MarketBar,
    constraints: &ExecutionConstraints,
    slippage: f64,
    fee_per_unit: f64,
) -> Option<Fill> {
    let price = match &intent.order_type {
        OrderType::Stop { price } => *price,
        _ => return None,
    };
    let triggered = match &intent.direction {
        TradeDirection::Long => bar.high >= price,
        TradeDirection::Short => bar.low <= price,
    };
    if !triggered {
        return None;
    }
    let size = normalize_size(intent.requested_size, constraints)?;
    let direction_mult = match &intent.direction {
        TradeDirection::Long => 1.0,
        TradeDirection::Short => -1.0,
    };
    Some(Fill {
        timestamp: bar.timestamp,
        direction: intent.direction.clone(),
        price: round_to_tick(price + slippage * direction_mult, constraints),
        size,
        fees: fee_per_unit.abs() * size,
        slippage: slippage.abs(),
        partial: false,
    })
}
