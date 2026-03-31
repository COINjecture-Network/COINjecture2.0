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
                        // Reorg detection
                        let parent = block["parent_hash"]
                            .as_str()
                            .or_else(|| block["header"]["prev_hash"].as_str())
                            .unwrap_or("");
                        if !sync.last_indexed_hash.is_empty() && parent != sync.last_indexed_hash {
                            let fork_height = match self.processor.find_fork_height(parent).await {
                                Ok(height) => height,
                                Err(e) => {
                                    tracing::error!(error = %e, "Fork lookup failed");
                                    break;
                                }
                            };

                            if let Err(e) = self.processor.handle_reorg(fork_height).await {
                                tracing::error!(error = %e, "Reorg handling failed");
                                break;
                            }

                            sync.last_indexed_height = fork_height;
                            sync.last_finalized_height = fork_height;
                            sync.last_indexed_hash =
                                match self.processor.get_block_hash_at_height(fork_height).await {
                                    Ok(hash) => hash,
                                    Err(e) => {
                                        tracing::error!(error = %e, "Failed to load fork hash");
                                        break;
                                    }
                                };
                            sync.last_sync_at = Utc::now();

                            tracing::warn!(
                                fork_height,
                                current_height = height,
                                "Reorg handled; will resume from fork point"
                            );
                            break;
                        }

                        match self.processor.process_block(&block).await {
                            Ok(_) => {
                                sync.last_indexed_height = height;
                                sync.last_finalized_height = height;
                                sync.last_indexed_hash = block["hash"]
                                    .as_str()
                                    .or_else(|| block["block_hash"].as_str())
                                    .or_else(|| block["header"]["hash"].as_str())
                                    .unwrap_or("")
                                    .to_string();
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
