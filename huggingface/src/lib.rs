// Hugging Face Dataset Integration - INSTITUTIONAL GRADE v3.1
// Comprehensive metrics collection for academic research and transparency
// Real-time sync of marketplace problem and solution data to Hugging Face
//
// Phase 1C: Dual-Feed Streaming Architecture
// - Feed A: head_unconfirmed - Real-time blocks (may contain future orphans)
// - Feed B: canonical_confirmed - Only k-confirmed blocks
// - Feed C: reorg_events - Forensic log of chain reorganizations

pub mod client;
pub mod energy;
pub mod metrics;
pub mod serialize;
pub mod streamer;

pub use client::{HuggingFaceClient, HuggingFaceConfig, DatasetRecord};
pub use energy::{EnergyMeasurement, EnergyMeasurementMethod, EnergyConfig};
pub use metrics::{MetricsCollector, NetworkContext, HardwareContext};
pub use serialize::{serialize_problem, serialize_solution};
pub use streamer::{
    DualFeedStreamer, StreamerConfig, StreamerState, StreamerStateSummary,
    UnconfirmedBlockRecord, ConfirmedBlockRecord, ReorgEventRecord, StreamerError,
};

use coinject_state::ProblemSubmission;
use coinject_core::Block;
use std::collections::VecDeque;
use std::time::Duration;

/// Pending block awaiting k-confirmations before publishing
#[derive(Debug, Clone)]
struct PendingBlock {
    block: Block,
    is_mined: bool,
    network_ctx: Option<NetworkContext>,
    _received_at_height: u64,
}

/// Main Hugging Face sync service
pub struct HuggingFaceSync {
    client: tokio::sync::Mutex<HuggingFaceClient>,
    metrics_collector: tokio::sync::Mutex<MetricsCollector>,
    config: SyncConfig,
    /// Pending blocks buffer - blocks waiting for k-confirmations
    /// Key: block height, Value: pending block data
    pending_blocks: tokio::sync::Mutex<VecDeque<PendingBlock>>,
    /// Current best known chain height (for confirmation counting)
    current_height: tokio::sync::Mutex<u64>,
}

/// Configuration for Hugging Face sync
#[derive(Debug, Clone)]
pub struct SyncConfig {
    pub enabled: bool,
    pub include_submitter_address: bool,
    pub include_solver_address: bool,
    pub batch_size: usize,
    pub batch_interval: Duration,
    /// Minimum confirmations before publishing a block to Hugging Face
    /// This prevents publishing blocks that may be reorged away
    /// Default: 20 for testnet (conservative), can be lowered for mainnet
    pub min_confirmations: u64,
}

impl Default for SyncConfig {
    fn default() -> Self {
        SyncConfig {
            enabled: true,
            include_submitter_address: true,
            include_solver_address: true,
            batch_size: 10,
            batch_interval: Duration::from_secs(5),
            min_confirmations: 20, // Conservative default for testnet
        }
    }
}

impl HuggingFaceSync {
    /// Create new Hugging Face sync service
    pub fn new(
        hf_config: HuggingFaceConfig,
        energy_config: energy::EnergyConfig,
        sync_config: SyncConfig,
    ) -> Result<Self, SyncError> {
        let client = HuggingFaceClient::new(hf_config)?;
        let metrics_collector = MetricsCollector::new(energy_config);

        eprintln!("📊 Hugging Face: Initialized with k={} confirmation guard", sync_config.min_confirmations);

        Ok(HuggingFaceSync {
            client: tokio::sync::Mutex::new(client),
            metrics_collector: tokio::sync::Mutex::new(metrics_collector),
            config: sync_config,
            pending_blocks: tokio::sync::Mutex::new(VecDeque::new()),
            current_height: tokio::sync::Mutex::new(0),
        })
    }

    /// Push problem submission to Hugging Face
    pub async fn push_problem_submission(
        &self,
        submission: &ProblemSubmission,
        block_height: u64,
    ) -> Result<(), SyncError> {
        if !self.config.enabled {
            return Ok(());
        }

        let collector = self.metrics_collector.lock().await;
        let record = collector.collect_problem_record(
            submission,
            block_height,
            &self.config,
        )?;
        drop(collector);

        let mut client = self.client.lock().await;
        client.push_record(record).await?;
        Ok(())
    }

    /// Push solution submission to Hugging Face
    pub async fn push_solution_submission(
        &self,
        submission: &ProblemSubmission,
        block_height: u64,
        solve_time: Duration,
        verify_time: Duration,
        solve_memory: usize,
        verify_memory: usize,
    ) -> Result<(), SyncError> {
        if !self.config.enabled {
            return Ok(());
        }

        // Measure energy during verification
        let mut collector = self.metrics_collector.lock().await;
        let (solve_energy, verify_energy) = collector.measure_energy(solve_time, verify_time)?;

        let record = collector.collect_solution_record(
            submission,
            block_height,
            solve_time,
            verify_time,
            solve_memory,
            verify_memory,
            solve_energy,
            verify_energy,
            &self.config,
        )?;
        drop(collector);

        let mut client = self.client.lock().await;
        client.push_record(record).await?;
        Ok(())
    }

    /// Push consensus block to Hugging Face (for mined or validated blocks)
    /// Blocks are held in a pending buffer until they have k confirmations
    pub async fn push_consensus_block(
        &self,
        block: &Block,
        is_mined: bool,
    ) -> Result<(), SyncError> {
        self.push_consensus_block_with_context(block, is_mined, None).await
    }

    /// Push consensus block with network context - INSTITUTIONAL GRADE v3.0
    /// Blocks are held in a pending buffer until they have k confirmations
    /// This prevents publishing blocks that may be reorged away
    pub async fn push_consensus_block_with_context(
        &self,
        block: &Block,
        is_mined: bool,
        network_ctx: Option<NetworkContext>,
    ) -> Result<(), SyncError> {
        if !self.config.enabled {
            eprintln!("⚠️  Hugging Face sync is disabled");
            return Ok(());
        }

        let block_height = block.header.height;
        let k = self.config.min_confirmations;

        // Update current chain height
        {
            let mut height = self.current_height.lock().await;
            if block_height > *height {
                *height = block_height;
            }
        }

        // Add block to pending buffer
        {
            let mut pending = self.pending_blocks.lock().await;

            // Check if this block is already in the pending buffer (avoid duplicates)
            let block_hash = block.hash();
            let already_pending = pending.iter().any(|pb|
                pb.block.header.height == block_height &&
                pb.block.hash() == block_hash
            );

            if !already_pending {
                pending.push_back(PendingBlock {
                    block: block.clone(),
                    is_mined,
                    network_ctx: network_ctx.clone(),
                    _received_at_height: block_height,
                });
                eprintln!("📦 Hugging Face: Block {} added to pending buffer (k={} confirmations required, {} pending)",
                    block_height, k, pending.len());
            }
        }

        // Process any blocks that now have k confirmations
        self.process_confirmed_blocks().await
    }

    /// Process blocks that have achieved k confirmations and publish them
    async fn process_confirmed_blocks(&self) -> Result<(), SyncError> {
        let k = self.config.min_confirmations;
        let current_height = *self.current_height.lock().await;

        // Collect blocks to publish (those with k+ confirmations)
        let blocks_to_publish: Vec<PendingBlock> = {
            let mut pending = self.pending_blocks.lock().await;
            let mut to_publish = Vec::new();

            // Drain blocks that have k confirmations
            while let Some(front) = pending.front() {
                let confirmations = current_height.saturating_sub(front.block.header.height);
                if confirmations >= k {
                    if let Some(pb) = pending.pop_front() {
                        to_publish.push(pb);
                    }
                } else {
                    // Blocks are in order, so if this one isn't confirmed, neither are the rest
                    break;
                }
            }
            to_publish
        };

        // Publish confirmed blocks
        for pb in blocks_to_publish {
            let confirmations = current_height.saturating_sub(pb.block.header.height);
            eprintln!("✅ Hugging Face: Publishing block {} with {} confirmations (k={})",
                pb.block.header.height, confirmations, k);

            let collector = self.metrics_collector.lock().await;
            let record = match collector.collect_consensus_block_record_with_context(
                &pb.block, pb.is_mined, pb.network_ctx.as_ref()
            ) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("❌ Hugging Face: Failed to collect record for confirmed block {}: {}",
                        pb.block.header.height, e);
                    continue; // Skip this block but continue with others
                }
            };
            drop(collector);

            let mut client = self.client.lock().await;
            if let Err(e) = client.push_record(record).await {
                eprintln!("❌ Hugging Face: Failed to push confirmed block {}: {}",
                    pb.block.header.height, e);
                // Continue with other blocks
            }
        }

        Ok(())
    }

    /// Handle a reorg by clearing pending blocks at or above the reorg height
    /// Call this when a chain reorganization is detected
    pub async fn handle_reorg(&self, reorg_height: u64) {
        let mut pending = self.pending_blocks.lock().await;
        let before_count = pending.len();

        // Remove all pending blocks at or above reorg height
        pending.retain(|pb| pb.block.header.height < reorg_height);

        let removed = before_count - pending.len();
        if removed > 0 {
            eprintln!("⚠️  Hugging Face: Reorg detected at height {}. Removed {} pending blocks from HF buffer",
                reorg_height, removed);
        }

        // Update current height if needed
        let mut height = self.current_height.lock().await;
        if *height >= reorg_height {
            *height = reorg_height.saturating_sub(1);
        }
    }

    /// Get the number of pending blocks awaiting confirmation
    pub async fn pending_count(&self) -> usize {
        self.pending_blocks.lock().await.len()
    }
    
    /// Set the node's PeerId for attribution in metrics
    pub async fn set_node_id(&self, peer_id: String) {
        let mut collector = self.metrics_collector.lock().await;
        collector.set_node_id(peer_id);
    }

    /// Force flush any buffered records
    pub async fn flush(&self) -> Result<(), SyncError> {
        if !self.config.enabled {
            return Ok(());
        }

        let mut client = self.client.lock().await;
        client.force_flush().await?;
        Ok(())
    }
}

/// Sync errors
#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("Hugging Face client error: {0}")]
    Client(#[from] client::ClientError),
    #[error("Metrics collection error: {0}")]
    Metrics(#[from] metrics::MetricsError),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serialize::SerializationError),
    #[error("Energy measurement error: {0}")]
    Energy(#[from] energy::EnergyError),
}

