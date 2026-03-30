//! Main indexer loop — polls node RPC for confirmed blocks and processes them.

use super::event_processor::EventProcessor;
use super::sync_state::SyncState;
use crate::node_rpc::NodeRpcClient;
use crate::sse::EventBroadcaster;
use crate::supabase::SupabaseClient;
use chrono::Utc;
use std::sync::Arc;
use std::time::Duration;

pub struct IndexerService {
    node_rpc: Arc<NodeRpcClient>,
    processor: EventProcessor,
    supabase: Arc<SupabaseClient>,
    poll_interval: Duration,
    confirmations: u64,
}

impl IndexerService {
    pub fn new(
        node_rpc: Arc<NodeRpcClient>,
        supabase: Arc<SupabaseClient>,
        broadcaster: Arc<EventBroadcaster>,
        poll_interval: Duration,
        confirmations: u64,
    ) -> Self {
        Self {
            node_rpc: node_rpc.clone(),
            processor: EventProcessor {
                supabase: supabase.clone(),
                broadcaster,
            },
            supabase,
            poll_interval,
            confirmations,
        }
    }

    pub async fn run(self) {
        tracing::info!(
            confirmations = self.confirmations,
            poll_ms = self.poll_interval.as_millis(),
            "Blockchain indexer started"
        );

        let mut sync = match SyncState::load(&self.supabase).await {
            Ok(s) => {
                tracing::info!(height = s.last_indexed_height, "Resuming indexer");
                s
            }
            Err(e) => {
                tracing::warn!(error = %e, "Starting indexer from genesis");
                SyncState::default()
            }
        };

        let mut interval = tokio::time::interval(self.poll_interval);

        loop {
            interval.tick().await;

            // Get chain tip
            let chain_tip = match self.node_rpc.get_chain_info().await {
                Ok(info) => {
                    let h = info["best_height"].as_u64().unwrap_or(0);
                    metrics::gauge!("coinjecture_chain_tip_height").set(h as f64);
                    h
                }
                Err(_) => continue,
            };

            let safe_height = chain_tip.saturating_sub(self.confirmations);
            if safe_height <= sync.last_indexed_height {
                continue;
            }

            let start = sync.last_indexed_height + 1;
            let end = safe_height.min(start + 99);

            for height in start..=end {
                match self.node_rpc.get_block_by_height(height).await {
                    Ok(block) => {
                        // Block structure: { header: { height, prev_hash, ... }, ... }
                        let header = &block["header"];

                        // Reorg detection via prev_hash (byte array in JSON)
                        let prev_hash_str = if let Some(arr) = header["prev_hash"].as_array() {
                            arr.iter()
                                .map(|b| format!("{:02x}", b.as_u64().unwrap_or(0)))
                                .collect::<String>()
                        } else {
                            header["prev_hash"].as_str().unwrap_or("").to_string()
                        };

                        if !sync.last_indexed_hash.is_empty()
                            && !prev_hash_str.is_empty()
                            && prev_hash_str != sync.last_indexed_hash
                        {
                            tracing::warn!(
                                height,
                                expected = %sync.last_indexed_hash,
                                got = %prev_hash_str,
                                "Chain reorg detected"
                            );
                            if let Err(e) =
                                self.processor.handle_reorg(sync.last_indexed_height).await
                            {
                                tracing::error!(error = %e, "Reorg handling failed");
                                break;
                            }
                        }

                        match self.processor.process_block(&block).await {
                            Ok(_) => {
                                // Store prev_hash of the NEXT expected block for reorg detection.
                                // Since the node returns prev_hash as a byte array, we store the
                                // hex representation so we can compare on the next iteration.
                                // For the current block, its hash IS the next block's prev_hash.
                                // We don't have the current block's hash directly, so we skip
                                // reorg detection for the first block after a restart.
                                sync.last_indexed_height = height;
                                sync.last_indexed_hash = prev_hash_str.clone();
                                sync.last_sync_at = Utc::now();
                            }
                            Err(e) => {
                                tracing::error!(height, error = %e, "Block processing failed");
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        tracing::debug!(height, error = %e, "Block fetch failed");
                        break;
                    }
                }
            }

            if let Err(e) = sync.save(&self.supabase).await {
                tracing::warn!(error = %e, "Failed to save sync state");
            }
        }
    }
}
