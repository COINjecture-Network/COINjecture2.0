//! Main indexer loop — polls node RPC for confirmed blocks and processes them.

use super::event_processor::EventProcessor;
use super::sync_state::SyncState;
use crate::node_rpc::NodeRpcClient;
use crate::sse::EventBroadcaster;
use crate::supabase::SupabaseClient;
use crate::wallet_activity;
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
            if sync.last_indexed_height > chain_tip.saturating_add(10) {
                tracing::warn!(
                    sync_height = sync.last_indexed_height,
                    chain_tip,
                    "Indexer sync ahead of chain tip — resetting to genesis"
                );
                sync = SyncState::default();
                if let Err(e) = sync.save(&self.supabase).await {
                    tracing::warn!(error = %e, "Failed to save reset sync state");
                }
            }
            if safe_height <= sync.last_indexed_height {
                continue;
            }

            let start = sync.last_indexed_height + 1;
            let end = safe_height.min(start + 99);

            for height in start..=end {
                match self.node_rpc.get_block_by_height(height).await {
                    Ok(block) => {
                        // Reorg detection (normalize byte-array hashes from RPC JSON)
                        let parent = wallet_activity::parent_hash_from_json(&block);
                        if !sync.last_indexed_hash.is_empty() && parent != sync.last_indexed_hash {
                            let fork_height = match self.processor.find_fork_height(&parent).await {
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
                                sync.last_indexed_hash =
                                    wallet_activity::block_hash_from_json(&block);
                                sync.last_sync_at = Utc::now();
                            }
                            Err(e) => {
                                tracing::error!(height, error = %e, "Block processing failed");
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!(height, error = %e, "Block fetch failed");
                        if height > chain_tip || sync.last_indexed_height > chain_tip {
                            tracing::warn!(
                                chain_tip,
                                sync_height = sync.last_indexed_height,
                                "Resetting indexer after missing block / chain reset"
                            );
                            sync = SyncState::default();
                            let _ = sync.save(&self.supabase).await;
                        }
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
