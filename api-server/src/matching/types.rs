//! Internal types for the matching engine — prices use `rust_decimal::Decimal`
//! to avoid floating-point errors in financial arithmetic.

use chrono::{DateTime, Utc};
pub use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type OrderId = Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderType {
    Limit,
    Market,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeInForce {
    GTC,
    IOC,
    FOK,
}

/// An order as it exists in the matching engine's memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InternalOrder {
    pub id: OrderId,
    pub user_id: String,
    pub wallet_address: String,
    pub pair_id: String,
    pub side: Side,
    pub order_type: OrderType,
    pub price: Option<Decimal>,
    pub quantity: Decimal,
    pub filled_quantity: Decimal,
    pub time_in_force: TimeInForce,
    pub created_at: DateTime<Utc>,
}

impl InternalOrder {
    pub fn remaining(&self) -> Decimal {
        self.quantity - self.filled_quantity
    }

    pub fn is_filled(&self) -> bool {
        self.filled_quantity >= self.quantity
    }
}

/// A trade produced by the matching engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchedTrade {
    pub id: Uuid,
    pub pair_id: String,
    pub buy_order_id: OrderId,
    pub sell_order_id: OrderId,
    pub buyer_wallet: String,
    pub seller_wallet: String,
    pub price: Decimal,
    pub quantity: Decimal,
    pub timestamp: DateTime<Utc>,
}

/// Result of submitting an order.
#[derive(Debug, Clone, Serialize)]
pub struct MatchResult {
    pub order_id: OrderId,
    pub status: MatchStatus,
    pub trades: Vec<MatchedTrade>,
    pub remaining_quantity: Decimal,
}

#[derive(Debug, Clone, Serialize)]
pub enum MatchStatus {
    Resting,
    PartiallyFilled,
    Filled,
    Cancelled,
}
