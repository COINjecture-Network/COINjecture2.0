//! Trade outbox — fans out matched trades to persistence (Supabase) and
//! real-time streaming (SSE broadcaster).

use crate::sse::{EventBroadcaster, MarketplaceEvent};
use crate::supabase::SupabaseClient;
use std::sync::Arc;
use tokio::sync::mpsc;

use super::types::MatchedTrade;

pub struct TradeOutbox {
    trade_rx: mpsc::UnboundedReceiver<Vec<MatchedTrade>>,
    supabase: Option<Arc<SupabaseClient>>,
    broadcaster: Arc<EventBroadcaster>,
}

impl TradeOutbox {
    pub fn new(
        trade_rx: mpsc::UnboundedReceiver<Vec<MatchedTrade>>,
        supabase: Option<Arc<SupabaseClient>>,
        broadcaster: Arc<EventBroadcaster>,
    ) -> Self {
        Self {
            trade_rx,
            supabase,
            broadcaster,
        }
    }

    pub async fn run(mut self) {
        tracing::info!("Trade outbox started");

        while let Some(trades) = self.trade_rx.recv().await {
            for trade in &trades {
                // 1. Publish to SSE (fire-and-forget)
                self.broadcaster
                    .publish_marketplace(MarketplaceEvent::Trade {
                        pair: trade.pair_id.clone(),
                        price: trade.price.to_string(),
                        quantity: trade.quantity.to_string(),
                        timestamp: trade.timestamp.to_rfc3339(),
                    });

                // 2. Persist to Supabase (with logging on failure)
                if let Some(ref supabase) = self.supabase {
                    let trade_data = serde_json::json!({
                        "id": trade.id.to_string(),
                        "pair_id": trade.pair_id,
                        "buy_order_id": trade.buy_order_id.to_string(),
                        "sell_order_id": trade.sell_order_id.to_string(),
                        "buyer_wallet": trade.buyer_wallet,
                        "seller_wallet": trade.seller_wallet,
                        "price": trade.price.to_string(),
                        "quantity": trade.quantity.to_string(),
                        "executed_at": trade.timestamp.to_rfc3339(),
                    });

                    match supabase.insert_row("trades", trade_data).await {
                        Ok(_) => {
                            tracing::debug!(
                                trade_id = %trade.id,
                                pair = %trade.pair_id,
                                price = %trade.price,
                                qty = %trade.quantity,
                                "Trade persisted"
                            );
                        }
                        Err(e) => {
                            tracing::error!(
                                trade_id = %trade.id,
                                error = %e,
                                "Failed to persist trade"
                            );
                        }
                    }
                }
            }
        }

        tracing::info!("Trade outbox stopped");
    }
}
