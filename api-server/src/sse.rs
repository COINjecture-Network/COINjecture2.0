//! Central SSE (Server-Sent Events) broadcaster for real-time blockchain data.
//!
//! The `EventBroadcaster` holds broadcast channels for block, mempool, and
//! marketplace events. The `NodePoller` publishes to these channels, and
//! SSE endpoint handlers subscribe to produce streams for connected clients.

use axum::response::sse::Event;
use serde::Serialize;
use std::convert::Infallible;
use tokio::sync::broadcast;
use tokio::sync::RwLock;

// ── Event types ─────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize)]
pub struct BlockEvent {
    pub height: u64,
    pub hash: String,
    pub timestamp: String,
    pub tx_count: usize,
    pub miner: String,
    pub work_score: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct MempoolEvent {
    pub pending_count: usize,
    pub total_size_bytes: usize,
    pub oldest_tx_age_seconds: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type")]
pub enum MarketplaceEvent {
    NewOrder {
        id: String,
        pair: String,
        side: String,
        price: String,
        quantity: String,
    },
    Trade {
        pair: String,
        price: String,
        quantity: String,
        timestamp: String,
    },
    TaskPosted {
        id: String,
        title: String,
        bounty: String,
        problem_class: String,
    },
}

// ── Broadcaster ─────────────────────────────────────────────────────────────

pub struct EventBroadcaster {
    block_tx: broadcast::Sender<BlockEvent>,
    mempool_tx: broadcast::Sender<MempoolEvent>,
    marketplace_tx: broadcast::Sender<MarketplaceEvent>,
    /// Cached latest block (for /chain/latest-block instant responses).
    pub latest_block: RwLock<Option<BlockEvent>>,
    /// Cached latest mempool snapshot.
    pub latest_mempool: RwLock<Option<MempoolEvent>>,
}

impl EventBroadcaster {
    pub fn new(capacity: usize) -> Self {
        Self {
            block_tx: broadcast::channel(capacity).0,
            mempool_tx: broadcast::channel(capacity).0,
            marketplace_tx: broadcast::channel(capacity).0,
            latest_block: RwLock::new(None),
            latest_mempool: RwLock::new(None),
        }
    }

    pub async fn publish_block(&self, event: BlockEvent) {
        *self.latest_block.write().await = Some(event.clone());
        let _ = self.block_tx.send(event);
    }

    pub async fn publish_mempool(&self, event: MempoolEvent) {
        *self.latest_mempool.write().await = Some(event.clone());
        let _ = self.mempool_tx.send(event);
    }

    pub fn publish_marketplace(&self, event: MarketplaceEvent) {
        let _ = self.marketplace_tx.send(event);
    }

    /// Create an SSE stream for block events.
    pub fn subscribe_blocks(
        &self,
    ) -> impl tokio_stream::Stream<Item = Result<Event, Infallible>> + Send + 'static {
        let mut rx = self.block_tx.subscribe();
        async_stream::stream! {
            yield Ok(Event::default().event("connected").data("block_stream"));
            loop {
                match rx.recv().await {
                    Ok(block) => {
                        let data = serde_json::to_string(&block).unwrap_or_default();
                        yield Ok(Event::default()
                            .event("block")
                            .id(block.height.to_string())
                            .data(data));
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(skipped = n, "SSE block client lagged");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }

    /// Create an SSE stream for mempool events.
    pub fn subscribe_mempool(
        &self,
    ) -> impl tokio_stream::Stream<Item = Result<Event, Infallible>> + Send + 'static {
        let mut rx = self.mempool_tx.subscribe();
        async_stream::stream! {
            yield Ok(Event::default().event("connected").data("mempool_stream"));
            loop {
                match rx.recv().await {
                    Ok(mp) => {
                        let data = serde_json::to_string(&mp).unwrap_or_default();
                        yield Ok(Event::default().event("mempool").data(data));
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }

    /// Create an SSE stream for marketplace events.
    pub fn subscribe_marketplace(
        &self,
    ) -> impl tokio_stream::Stream<Item = Result<Event, Infallible>> + Send + 'static {
        let mut rx = self.marketplace_tx.subscribe();
        async_stream::stream! {
            yield Ok(Event::default().event("connected").data("marketplace_stream"));
            loop {
                match rx.recv().await {
                    Ok(evt) => {
                        let event_name = match &evt {
                            MarketplaceEvent::NewOrder { .. } => "new_order",
                            MarketplaceEvent::Trade { .. } => "trade",
                            MarketplaceEvent::TaskPosted { .. } => "task_posted",
                        };
                        let data = serde_json::to_string(&evt).unwrap_or_default();
                        yield Ok(Event::default().event(event_name).data(data));
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }
}
