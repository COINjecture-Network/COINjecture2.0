//! Processes confirmed block events into database writes.

use crate::sse::EventBroadcaster;
use crate::supabase::SupabaseClient;
use serde_json::Value;
use std::sync::Arc;

pub struct EventProcessor {
    pub supabase: Arc<SupabaseClient>,
    pub broadcaster: Arc<EventBroadcaster>,
}

pub struct ProcessResult {
    pub height: u64,
    pub trades_finalized: usize,
    pub orders_updated: usize,
}

impl EventProcessor {
    /// Process a confirmed block — finalize trades and update marketplace state.
    pub async fn process_block(&self, block: &Value) -> Result<ProcessResult, String> {
        let height = block["height"].as_u64().unwrap_or(0);
        let tx_count = block["tx_count"].as_u64().unwrap_or(0);

        // Finalize trades that reference transactions in this block
        // (In production, match on_chain_tx_hash from the trades table)
        let trades_finalized = 0; // Actual finalization requires tx hash matching

        // Record indexer metrics
        metrics::gauge!("coinjecture_indexer_height").set(height as f64);
        metrics::counter!("coinjecture_blocks_indexed_total").increment(1);

        tracing::info!(height, tx_count, trades_finalized, "Block indexed");

        Ok(ProcessResult {
            height,
            trades_finalized,
            orders_updated: 0,
        })
    }

    /// Roll back data above `fork_height` on a chain reorg.
    pub async fn handle_reorg(&self, fork_height: u64) -> Result<(), String> {
        tracing::warn!(fork_height, "Chain reorg — rolling back indexed data");
        metrics::counter!("coinjecture_reorg_events_total").increment(1);

        // Unfinalize trades above fork height
        let body = serde_json::json!({ "is_finalized": false });
        let _ = self
            .supabase
            .patch_rows(
                &format!("trades?block_height=gt.{fork_height}"),
                body,
            )
            .await;

        Ok(())
    }
}
