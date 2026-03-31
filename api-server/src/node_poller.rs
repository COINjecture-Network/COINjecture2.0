//! Background task that polls the node's JSON-RPC for new blocks and mempool
//! data, then publishes changes to the EventBroadcaster for SSE streaming.

use crate::node_rpc::NodeRpcClient;
use crate::sse::{BlockEvent, EventBroadcaster, MempoolEvent};
use std::sync::Arc;
use std::time::Duration;

pub struct NodePoller {
    node_rpc: Arc<NodeRpcClient>,
    broadcaster: Arc<EventBroadcaster>,
    poll_interval: Duration,
}

impl NodePoller {
    pub fn new(
        node_rpc: Arc<NodeRpcClient>,
        broadcaster: Arc<EventBroadcaster>,
        poll_interval: Duration,
    ) -> Self {
        Self {
            node_rpc,
            broadcaster,
            poll_interval,
        }
    }

    /// Run the polling loop. Intended to be spawned as a background task.
    pub async fn run(&self) {
        let mut last_height: u64 = 0;
        let mut interval = tokio::time::interval(self.poll_interval);

        tracing::info!(
            interval_ms = self.poll_interval.as_millis(),
            "Node poller started"
        );

        loop {
            interval.tick().await;

            // Poll for new blocks
            match self.node_rpc.get_latest_block().await {
                Ok(block_data) => {
                    let height = block_data["height"].as_u64().unwrap_or(0);
                    if height > last_height {
                        last_height = height;
                        let event = BlockEvent {
                            height,
                            hash: block_data["hash"]
                                .as_str()
                                .unwrap_or("")
                                .to_string(),
                            timestamp: block_data["timestamp"]
                                .as_str()
                                .unwrap_or("")
                                .to_string(),
                            tx_count: block_data["tx_count"]
                                .as_u64()
                                .unwrap_or(0) as usize,
                            miner: block_data["miner"]
                                .as_str()
                                .unwrap_or("")
                                .to_string(),
                            work_score: block_data["work_score"]
                                .as_f64()
                                .unwrap_or(0.0),
                        };
                        self.broadcaster.publish_block(event).await;
                    }
                }
                Err(e) => {
                    tracing::debug!(error = %e, "Node block poll failed");
                }
            }

            // Poll chain info for mempool approximation
            // NOTE: No dedicated mempool RPC method exists yet.
            // TODO: Add mempool_getInfo to the RPC crate for accurate data.
            if let Ok(info) = self.node_rpc.get_chain_info().await {
                let event = MempoolEvent {
                    pending_count: info["pending_transactions"]
                        .as_u64()
                        .unwrap_or(0) as usize,
                    total_size_bytes: 0,
                    oldest_tx_age_seconds: 0,
                };
                self.broadcaster.publish_mempool(event).await;
            }
        }
    }
}
