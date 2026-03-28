//! The matching engine — single-threaded, in-memory, price-time priority.
//!
//! Concurrency happens AROUND the core (API ingestion via channels, persistence
//! via the outbox), never INSIDE the matching loop.

use super::order_book::{BookDepth, OrderBook};
use super::types::*;
use serde::Serialize;
use std::collections::HashMap;
use tokio::sync::{mpsc, oneshot};

// ── Commands ────────────────────────────────────────────────────────────────

pub enum EngineCommand {
    SubmitOrder {
        order: InternalOrder,
        response: oneshot::Sender<MatchResult>,
    },
    CancelOrder {
        order_id: OrderId,
        pair_id: String,
        response: oneshot::Sender<Option<InternalOrder>>,
    },
    GetDepth {
        pair_id: String,
        max_levels: usize,
        response: oneshot::Sender<Option<BookDepth>>,
    },
    GetStats {
        response: oneshot::Sender<EngineStats>,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct EngineStats {
    pub instruments: usize,
    pub total_resting_orders: usize,
    pub total_trades_executed: u64,
    pub orders_per_instrument: HashMap<String, usize>,
}

// ── Engine ──────────────────────────────────────────────────────────────────

pub struct MatchingEngine {
    books: HashMap<String, OrderBook>,
    command_rx: mpsc::Receiver<EngineCommand>,
    trade_tx: mpsc::UnboundedSender<Vec<MatchedTrade>>,
    total_trades: u64,
}

impl MatchingEngine {
    /// Create a new engine. Returns (engine, command_sender, trade_receiver).
    pub fn new(
        known_pairs: Vec<String>,
    ) -> (
        Self,
        mpsc::Sender<EngineCommand>,
        mpsc::UnboundedReceiver<Vec<MatchedTrade>>,
    ) {
        let (cmd_tx, cmd_rx) = mpsc::channel(10_000);
        let (trade_tx, trade_rx) = mpsc::unbounded_channel();

        let mut books = HashMap::new();
        for pair in known_pairs {
            books.insert(pair.clone(), OrderBook::new(pair));
        }

        (
            Self {
                books,
                command_rx: cmd_rx,
                trade_tx,
                total_trades: 0,
            },
            cmd_tx,
            trade_rx,
        )
    }

    /// Run the engine loop. Spawn on a dedicated tokio task.
    pub async fn run(mut self) {
        tracing::info!(instruments = self.books.len(), "Matching engine started");

        while let Some(cmd) = self.command_rx.recv().await {
            match cmd {
                EngineCommand::SubmitOrder { order, response } => {
                    let pair_id = order.pair_id.clone();
                    let book = self
                        .books
                        .entry(pair_id)
                        .or_insert_with_key(|k| OrderBook::new(k.clone()));

                    let result = book.submit_order(order);

                    if !result.trades.is_empty() {
                        self.total_trades += result.trades.len() as u64;
                        let _ = self.trade_tx.send(result.trades.clone());
                    }

                    let _ = response.send(result);
                }
                EngineCommand::CancelOrder {
                    order_id,
                    pair_id,
                    response,
                } => {
                    let cancelled = self
                        .books
                        .get_mut(&pair_id)
                        .and_then(|b| b.cancel_order(&order_id));
                    let _ = response.send(cancelled);
                }
                EngineCommand::GetDepth {
                    pair_id,
                    max_levels,
                    response,
                } => {
                    let depth = self.books.get(&pair_id).map(|b| b.depth(max_levels));
                    let _ = response.send(depth);
                }
                EngineCommand::GetStats { response } => {
                    let stats = EngineStats {
                        instruments: self.books.len(),
                        total_resting_orders: self.books.values().map(|b| b.order_count()).sum(),
                        total_trades_executed: self.total_trades,
                        orders_per_instrument: self
                            .books
                            .iter()
                            .map(|(k, v)| (k.clone(), v.order_count()))
                            .collect(),
                    };
                    let _ = response.send(stats);
                }
            }
        }

        tracing::info!("Matching engine stopped");
    }
}

// ── Handle (for API layer) ──────────────────────────────────────────────────

/// Cloneable handle for sending commands to the matching engine.
#[derive(Clone)]
pub struct EngineHandle {
    cmd_tx: mpsc::Sender<EngineCommand>,
}

impl EngineHandle {
    pub fn new(cmd_tx: mpsc::Sender<EngineCommand>) -> Self {
        Self { cmd_tx }
    }

    pub async fn submit_order(&self, order: InternalOrder) -> Result<MatchResult, EngineError> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(EngineCommand::SubmitOrder {
                order,
                response: tx,
            })
            .await
            .map_err(|_| EngineError::Unavailable)?;
        rx.await.map_err(|_| EngineError::Unavailable)
    }

    pub async fn cancel_order(
        &self,
        order_id: OrderId,
        pair_id: String,
    ) -> Result<Option<InternalOrder>, EngineError> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(EngineCommand::CancelOrder {
                order_id,
                pair_id,
                response: tx,
            })
            .await
            .map_err(|_| EngineError::Unavailable)?;
        rx.await.map_err(|_| EngineError::Unavailable)
    }

    pub async fn get_depth(
        &self,
        pair_id: String,
        max_levels: usize,
    ) -> Result<Option<BookDepth>, EngineError> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(EngineCommand::GetDepth {
                pair_id,
                max_levels,
                response: tx,
            })
            .await
            .map_err(|_| EngineError::Unavailable)?;
        rx.await.map_err(|_| EngineError::Unavailable)
    }

    pub async fn get_stats(&self) -> Result<EngineStats, EngineError> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(EngineCommand::GetStats { response: tx })
            .await
            .map_err(|_| EngineError::Unavailable)?;
        rx.await.map_err(|_| EngineError::Unavailable)
    }
}

#[derive(Debug)]
pub enum EngineError {
    Unavailable,
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable => write!(f, "Matching engine unavailable"),
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn dec(s: &str) -> Decimal {
        s.parse().unwrap()
    }

    fn test_order(side: Side, price: &str, qty: &str) -> InternalOrder {
        InternalOrder {
            id: uuid::Uuid::new_v4(),
            user_id: "user".into(),
            wallet_address: if side == Side::Buy {
                "buyer".into()
            } else {
                "seller".into()
            },
            pair_id: "BEANS/USDC".into(),
            side,
            order_type: OrderType::Limit,
            price: Some(dec(price)),
            quantity: dec(qty),
            filled_quantity: Decimal::ZERO,
            time_in_force: TimeInForce::GTC,
            created_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_engine_submit_and_match() {
        let (engine, tx, mut trade_rx) = MatchingEngine::new(vec!["BEANS/USDC".into()]);
        let handle = EngineHandle::new(tx);
        tokio::spawn(engine.run());

        let sell = test_order(Side::Sell, "1.50", "100");
        let result = handle.submit_order(sell).await.unwrap();
        assert!(matches!(result.status, MatchStatus::Resting));

        let buy = test_order(Side::Buy, "1.50", "50");
        let result = handle.submit_order(buy).await.unwrap();
        assert!(matches!(result.status, MatchStatus::Filled));
        assert_eq!(result.trades.len(), 1);

        let trades = trade_rx.recv().await.unwrap();
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].quantity, dec("50"));
    }

    #[tokio::test]
    async fn test_engine_stats() {
        let (engine, tx, _trade_rx) = MatchingEngine::new(vec!["BEANS/USDC".into()]);
        let handle = EngineHandle::new(tx);
        tokio::spawn(engine.run());

        let stats = handle.get_stats().await.unwrap();
        assert_eq!(stats.instruments, 1);
        assert_eq!(stats.total_resting_orders, 0);
    }

    #[tokio::test]
    async fn test_engine_cancel() {
        let (engine, tx, _trade_rx) = MatchingEngine::new(vec!["BEANS/USDC".into()]);
        let handle = EngineHandle::new(tx);
        tokio::spawn(engine.run());

        let order = test_order(Side::Buy, "1.50", "100");
        let oid = order.id;
        handle.submit_order(order).await.unwrap();

        let cancelled = handle
            .cancel_order(oid, "BEANS/USDC".into())
            .await
            .unwrap();
        assert!(cancelled.is_some());
    }

    #[tokio::test]
    async fn test_engine_depth() {
        let (engine, tx, _trade_rx) = MatchingEngine::new(vec!["BEANS/USDC".into()]);
        let handle = EngineHandle::new(tx);
        tokio::spawn(engine.run());

        handle
            .submit_order(test_order(Side::Buy, "1.50", "100"))
            .await
            .unwrap();
        handle
            .submit_order(test_order(Side::Sell, "1.55", "50"))
            .await
            .unwrap();

        let depth = handle
            .get_depth("BEANS/USDC".into(), 10)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(depth.bids.len(), 1);
        assert_eq!(depth.asks.len(), 1);
    }
}
