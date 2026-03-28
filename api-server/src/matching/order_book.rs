//! Single-instrument order book with price-time priority.
//!
//! Buy side: highest price first, then earliest time.
//! Sell side: lowest price first, then earliest time.

use super::types::*;
use serde::Serialize;
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use uuid::Uuid;

// ── Price level key ─────────────────────────────────────────────────────────

/// BTreeMap key that controls price ordering per side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PriceLevel {
    price: Decimal,
    side: Side,
}

impl Ord for PriceLevel {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.side {
            Side::Buy => other.price.cmp(&self.price),   // descending
            Side::Sell => self.price.cmp(&other.price),   // ascending
        }
    }
}

impl PartialOrd for PriceLevel {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// ── Order book ──────────────────────────────────────────────────────────────

pub struct OrderBook {
    pair_id: String,
    bids: BTreeMap<PriceLevel, Vec<InternalOrder>>,
    asks: BTreeMap<PriceLevel, Vec<InternalOrder>>,
    order_index: HashMap<OrderId, (Side, Decimal)>,
}

impl OrderBook {
    pub fn new(pair_id: String) -> Self {
        Self {
            pair_id,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            order_index: HashMap::new(),
        }
    }

    /// Submit an order and execute matching.
    pub fn submit_order(&mut self, mut order: InternalOrder) -> MatchResult {
        let mut trades = Vec::new();

        match order.order_type {
            OrderType::Market => {
                self.match_against_book(&mut order, &mut trades);
                MatchResult {
                    order_id: order.id,
                    status: if order.is_filled() {
                        MatchStatus::Filled
                    } else {
                        MatchStatus::Cancelled
                    },
                    remaining_quantity: order.remaining(),
                    trades,
                }
            }
            OrderType::Limit => {
                if order.price.is_some() {
                    self.match_against_book(&mut order, &mut trades);
                }

                if order.is_filled() {
                    return MatchResult {
                        order_id: order.id,
                        status: MatchStatus::Filled,
                        trades,
                        remaining_quantity: Decimal::ZERO,
                    };
                }

                match order.time_in_force {
                    TimeInForce::FOK => MatchResult {
                        order_id: order.id,
                        status: MatchStatus::Cancelled,
                        trades: Vec::new(),
                        remaining_quantity: order.quantity,
                    },
                    TimeInForce::IOC => MatchResult {
                        order_id: order.id,
                        status: if trades.is_empty() {
                            MatchStatus::Cancelled
                        } else {
                            MatchStatus::PartiallyFilled
                        },
                        remaining_quantity: order.remaining(),
                        trades,
                    },
                    TimeInForce::GTC => {
                        let status = if trades.is_empty() {
                            MatchStatus::Resting
                        } else {
                            MatchStatus::PartiallyFilled
                        };
                        let remaining = order.remaining();
                        self.insert_resting_order(order.clone());
                        MatchResult {
                            order_id: order.id,
                            status,
                            trades,
                            remaining_quantity: remaining,
                        }
                    }
                }
            }
        }
    }

    /// Cancel an order by ID.
    pub fn cancel_order(&mut self, order_id: &OrderId) -> Option<InternalOrder> {
        let (side, price) = self.order_index.remove(order_id)?;
        let book = match side {
            Side::Buy => &mut self.bids,
            Side::Sell => &mut self.asks,
        };
        let level = PriceLevel { price, side };
        if let Some(orders) = book.get_mut(&level) {
            if let Some(pos) = orders.iter().position(|o| o.id == *order_id) {
                let removed = orders.remove(pos);
                if orders.is_empty() {
                    book.remove(&level);
                }
                return Some(removed);
            }
        }
        None
    }

    // ── Matching ────────────────────────────────────────────────────────

    fn match_against_book(
        &mut self,
        order: &mut InternalOrder,
        trades: &mut Vec<MatchedTrade>,
    ) {
        let opposite = match order.side {
            Side::Buy => &mut self.asks,
            Side::Sell => &mut self.bids,
        };

        let mut empty_levels = Vec::new();

        for (level, resting_orders) in opposite.iter_mut() {
            if order.is_filled() {
                break;
            }

            // Price check
            if let Some(limit_price) = order.price {
                let can_match = match order.side {
                    Side::Buy => limit_price >= level.price,
                    Side::Sell => limit_price <= level.price,
                };
                if !can_match {
                    break;
                }
            }

            let mut filled_indices = Vec::new();

            for (i, resting) in resting_orders.iter_mut().enumerate() {
                if order.is_filled() {
                    break;
                }

                let fill_qty = order.remaining().min(resting.remaining());
                let fill_price = level.price;

                order.filled_quantity += fill_qty;
                resting.filled_quantity += fill_qty;

                let (buyer_wallet, seller_wallet, buy_id, sell_id) = match order.side {
                    Side::Buy => (
                        order.wallet_address.clone(),
                        resting.wallet_address.clone(),
                        order.id,
                        resting.id,
                    ),
                    Side::Sell => (
                        resting.wallet_address.clone(),
                        order.wallet_address.clone(),
                        resting.id,
                        order.id,
                    ),
                };

                trades.push(MatchedTrade {
                    id: Uuid::new_v4(),
                    pair_id: order.pair_id.clone(),
                    buy_order_id: buy_id,
                    sell_order_id: sell_id,
                    buyer_wallet,
                    seller_wallet,
                    price: fill_price,
                    quantity: fill_qty,
                    timestamp: chrono::Utc::now(),
                });

                if resting.is_filled() {
                    filled_indices.push(i);
                    self.order_index.remove(&resting.id);
                }
            }

            for i in filled_indices.into_iter().rev() {
                resting_orders.remove(i);
            }

            if resting_orders.is_empty() {
                empty_levels.push(*level);
            }
        }

        for level in empty_levels {
            let opposite = match order.side {
                Side::Buy => &mut self.asks,
                Side::Sell => &mut self.bids,
            };
            opposite.remove(&level);
        }
    }

    fn insert_resting_order(&mut self, order: InternalOrder) {
        let price = order.price.expect("resting orders must have a price");
        let side = order.side;
        let level = PriceLevel { price, side };
        self.order_index.insert(order.id, (side, price));
        match side {
            Side::Buy => &mut self.bids,
            Side::Sell => &mut self.asks,
        }
        .entry(level)
        .or_default()
        .push(order);
    }

    // ── Queries ─────────────────────────────────────────────────────────

    pub fn best_bid(&self) -> Option<Decimal> {
        self.bids.keys().next().map(|l| l.price)
    }

    pub fn best_ask(&self) -> Option<Decimal> {
        self.asks.keys().next().map(|l| l.price)
    }

    pub fn spread(&self) -> Option<Decimal> {
        match (self.best_ask(), self.best_bid()) {
            (Some(ask), Some(bid)) => Some(ask - bid),
            _ => None,
        }
    }

    pub fn depth(&self, max_levels: usize) -> BookDepth {
        let bids = self
            .bids
            .iter()
            .take(max_levels)
            .map(|(l, orders)| DepthLevel {
                price: l.price,
                quantity: orders.iter().map(|o| o.remaining()).sum(),
                order_count: orders.len(),
            })
            .collect();

        let asks = self
            .asks
            .iter()
            .take(max_levels)
            .map(|(l, orders)| DepthLevel {
                price: l.price,
                quantity: orders.iter().map(|o| o.remaining()).sum(),
                order_count: orders.len(),
            })
            .collect();

        BookDepth {
            pair_id: self.pair_id.clone(),
            bids,
            asks,
        }
    }

    pub fn order_count(&self) -> usize {
        self.order_index.len()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct BookDepth {
    pub pair_id: String,
    pub bids: Vec<DepthLevel>,
    pub asks: Vec<DepthLevel>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DepthLevel {
    pub price: Decimal,
    pub quantity: Decimal,
    pub order_count: usize,
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn dec(s: &str) -> Decimal {
        s.parse().unwrap()
    }

    fn limit_buy(price: &str, qty: &str) -> InternalOrder {
        InternalOrder {
            id: Uuid::new_v4(),
            user_id: "buyer".into(),
            wallet_address: "wallet_buy".into(),
            pair_id: "BEANS/USDC".into(),
            side: Side::Buy,
            order_type: OrderType::Limit,
            price: Some(dec(price)),
            quantity: dec(qty),
            filled_quantity: Decimal::ZERO,
            time_in_force: TimeInForce::GTC,
            created_at: Utc::now(),
        }
    }

    fn limit_sell(price: &str, qty: &str) -> InternalOrder {
        InternalOrder {
            id: Uuid::new_v4(),
            user_id: "seller".into(),
            wallet_address: "wallet_sell".into(),
            pair_id: "BEANS/USDC".into(),
            side: Side::Sell,
            order_type: OrderType::Limit,
            price: Some(dec(price)),
            quantity: dec(qty),
            filled_quantity: Decimal::ZERO,
            time_in_force: TimeInForce::GTC,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn test_limit_buy_rests_on_empty_book() {
        let mut book = OrderBook::new("BEANS/USDC".into());
        let result = book.submit_order(limit_buy("1.50", "100"));
        assert!(matches!(result.status, MatchStatus::Resting));
        assert!(result.trades.is_empty());
        assert_eq!(book.order_count(), 1);
        assert_eq!(book.best_bid(), Some(dec("1.50")));
    }

    #[test]
    fn test_crossing_produces_trade() {
        let mut book = OrderBook::new("BEANS/USDC".into());
        book.submit_order(limit_sell("1.50", "100"));
        let result = book.submit_order(limit_buy("1.50", "50"));
        assert!(matches!(result.status, MatchStatus::Filled));
        assert_eq!(result.trades.len(), 1);
        assert_eq!(result.trades[0].price, dec("1.50"));
        assert_eq!(result.trades[0].quantity, dec("50"));
    }

    #[test]
    fn test_price_improvement() {
        let mut book = OrderBook::new("BEANS/USDC".into());
        book.submit_order(limit_sell("1.40", "100"));
        let result = book.submit_order(limit_buy("1.50", "50"));
        assert_eq!(result.trades[0].price, dec("1.40"));
    }

    #[test]
    fn test_partial_fill() {
        let mut book = OrderBook::new("BEANS/USDC".into());
        book.submit_order(limit_sell("1.50", "50"));
        let result = book.submit_order(limit_buy("1.50", "100"));
        assert!(matches!(result.status, MatchStatus::PartiallyFilled));
        assert_eq!(result.trades[0].quantity, dec("50"));
        assert_eq!(result.remaining_quantity, dec("50"));
        assert_eq!(book.best_bid(), Some(dec("1.50")));
    }

    #[test]
    fn test_price_time_priority() {
        let mut book = OrderBook::new("BEANS/USDC".into());
        let sell1 = limit_sell("1.50", "50");
        let sell1_id = sell1.id;
        book.submit_order(sell1);
        book.submit_order(limit_sell("1.50", "50"));

        let result = book.submit_order(limit_buy("1.50", "50"));
        assert_eq!(result.trades[0].sell_order_id, sell1_id);
    }

    #[test]
    fn test_market_order() {
        let mut book = OrderBook::new("BEANS/USDC".into());
        book.submit_order(limit_sell("1.50", "100"));

        let mut market = limit_buy("0", "50");
        market.order_type = OrderType::Market;
        market.price = None;

        let result = book.submit_order(market);
        assert!(matches!(result.status, MatchStatus::Filled));
    }

    #[test]
    fn test_market_unfilled_cancelled() {
        let mut book = OrderBook::new("BEANS/USDC".into());
        let mut market = limit_buy("0", "50");
        market.order_type = OrderType::Market;
        market.price = None;

        let result = book.submit_order(market);
        assert!(matches!(result.status, MatchStatus::Cancelled));
        assert_eq!(book.order_count(), 0);
    }

    #[test]
    fn test_cancel_order() {
        let mut book = OrderBook::new("BEANS/USDC".into());
        let order = limit_buy("1.50", "100");
        let id = order.id;
        book.submit_order(order);
        assert_eq!(book.order_count(), 1);
        assert!(book.cancel_order(&id).is_some());
        assert_eq!(book.order_count(), 0);
    }

    #[test]
    fn test_book_depth() {
        let mut book = OrderBook::new("BEANS/USDC".into());
        book.submit_order(limit_buy("1.50", "100"));
        book.submit_order(limit_buy("1.49", "200"));
        book.submit_order(limit_sell("1.55", "50"));

        let depth = book.depth(10);
        assert_eq!(depth.bids.len(), 2);
        assert_eq!(depth.asks.len(), 1);
        assert_eq!(depth.bids[0].price, dec("1.50"));
        assert_eq!(depth.asks[0].price, dec("1.55"));
    }

    #[test]
    fn test_spread() {
        let mut book = OrderBook::new("BEANS/USDC".into());
        book.submit_order(limit_buy("1.50", "100"));
        book.submit_order(limit_sell("1.55", "50"));
        assert_eq!(book.spread(), Some(dec("0.05")));
    }

    #[test]
    fn test_ioc_cancels_unfilled() {
        let mut book = OrderBook::new("BEANS/USDC".into());
        book.submit_order(limit_sell("1.50", "50"));

        let mut ioc = limit_buy("1.50", "100");
        ioc.time_in_force = TimeInForce::IOC;
        let result = book.submit_order(ioc);
        assert!(matches!(result.status, MatchStatus::PartiallyFilled));
        assert_eq!(result.trades[0].quantity, dec("50"));
        assert_eq!(book.order_count(), 0);
    }

    #[test]
    fn test_fok_rejects_partial() {
        let mut book = OrderBook::new("BEANS/USDC".into());
        book.submit_order(limit_sell("1.50", "50"));

        let mut fok = limit_buy("1.50", "100");
        fok.time_in_force = TimeInForce::FOK;
        let result = book.submit_order(fok);
        assert!(matches!(result.status, MatchStatus::Cancelled));
        assert!(result.trades.is_empty());
    }
}
