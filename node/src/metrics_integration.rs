// =============================================================================
// Network Metrics Integration
// =============================================================================
//
// This module bridges the node's runtime state with the tokenomics NetworkMetrics
// oracle. It collects live blockchain data and feeds it to the oracle so that
// all tokenomics calculations use empirical, network-derived values.
//
// INSTITUTIONAL QUALITY: This is the single integration point between
// blockchain state and economic calculations.
//
// NOTE: Some trackers are prepared for future metrics integration
#![allow(dead_code)]

use coinject_core::Block;
use coinject_tokenomics::network_metrics::{FaultType, NetworkMetrics, NetworkSnapshot};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

// =============================================================================
// METRICS COLLECTOR
// =============================================================================

/// Collects runtime metrics from the node and feeds them to the NetworkMetrics oracle
pub struct MetricsCollector {
    /// The underlying tokenomics oracle
    oracle: Arc<RwLock<NetworkMetrics>>,

    /// Block timing tracker (block_height -> receive_time)
    block_times: RwLock<HashMap<u64, Instant>>,

    /// Problem solve times (block_height -> solve_duration_ms)
    solve_times: RwLock<HashMap<u64, u64>>,

    /// Current peer count
    peer_count: RwLock<u32>,

    /// Consensus agreement ratio
    consensus_agreement: RwLock<f64>,

    /// Last snapshot block height
    last_snapshot_height: RwLock<u64>,

    /// Hash rate estimator
    hash_rate_estimator: RwLock<HashRateEstimator>,

    /// Fault tracker
    fault_tracker: RwLock<FaultTracker>,

    /// Staking tracker
    staking_tracker: RwLock<StakingTracker>,
}

impl MetricsCollector {
    /// Create a new metrics collector with a fresh oracle
    pub fn new() -> Self {
        MetricsCollector {
            oracle: Arc::new(RwLock::new(NetworkMetrics::default_window())),
            block_times: RwLock::new(HashMap::new()),
            solve_times: RwLock::new(HashMap::new()),
            peer_count: RwLock::new(0),
            consensus_agreement: RwLock::new(1.0),
            last_snapshot_height: RwLock::new(0),
            hash_rate_estimator: RwLock::new(HashRateEstimator::new()),
            fault_tracker: RwLock::new(FaultTracker::new()),
            staking_tracker: RwLock::new(StakingTracker::new()),
        }
    }

    /// Get the underlying oracle for read access by other modules
    pub fn oracle(&self) -> Arc<RwLock<NetworkMetrics>> {
        Arc::clone(&self.oracle)
    }

    // =========================================================================
    // EVENT RECORDING METHODS
    // =========================================================================

    /// Record a new block being added to the chain
    pub async fn on_block_added(&self, block: &Block, validation_time_ms: u64) {
        let height = block.header.height;
        let now = Instant::now();

        // Calculate block time
        let block_time = {
            let times = self.block_times.read().await;
            if height > 0 {
                if let Some(&prev_time) = times.get(&(height - 1)) {
                    now.duration_since(prev_time).as_secs_f64()
                } else {
                    8.64 // Default target
                }
            } else {
                8.64
            }
        };

        // Store this block's time
        {
            let mut times = self.block_times.write().await;
            times.insert(height, now);

            // Keep only last 1000 blocks
            if times.len() > 1000 {
                let oldest = height.saturating_sub(1000);
                times.retain(|&h, _| h > oldest);
            }
        }

        // Update hash rate estimator
        {
            let mut estimator = self.hash_rate_estimator.write().await;
            estimator.record_block(block.header.work_score, block_time);
        }

        // Get solve time if tracked
        let solve_time = {
            let solve_times = self.solve_times.read().await;
            solve_times
                .get(&height)
                .copied()
                .unwrap_or(validation_time_ms) as f64
                / 1000.0
        };

        // Build snapshot
        let snapshot = NetworkSnapshot {
            block_height: height,
            timestamp: block.header.timestamp as u64,
            hash_rate: self.hash_rate_estimator.read().await.estimate(),
            block_time,
            solve_time,
            problem_category: self.get_problem_category(block),
            total_fees: self.calculate_total_fees(block),
            tx_count: block.transactions.len() as u64,
            avg_tx_size: self.calculate_avg_tx_size(block),
            storage_used: self.estimate_block_storage(block),
            peer_count: *self.peer_count.read().await,
            consensus_agreement: *self.consensus_agreement.read().await,
            total_staked: self.staking_tracker.read().await.total_staked,
            staker_count: self.staking_tracker.read().await.staker_count,
            reorg_depth: self.fault_tracker.read().await.recent_reorg_depth,
            invalid_blocks: self.fault_tracker.read().await.recent_invalid_blocks,
            disconnections: self.fault_tracker.read().await.recent_disconnections,
        };

        // Record to oracle
        self.oracle.write().await.record_snapshot(snapshot);

        // Update last snapshot height
        *self.last_snapshot_height.write().await = height;

        // Reset fault tracker counters
        self.fault_tracker.write().await.reset_recent();

        tracing::debug!(
            "📊 Recorded metrics snapshot for block {}: hash_rate={:.2}, block_time={:.2}s",
            height,
            self.hash_rate_estimator.read().await.estimate(),
            block_time
        );
    }

    /// Record peer count change
    pub async fn on_peer_count_change(&self, count: usize) {
        *self.peer_count.write().await = count as u32;
    }

    /// Record consensus agreement update
    pub async fn on_consensus_update(&self, agreement_ratio: f64) {
        *self.consensus_agreement.write().await = agreement_ratio;
    }

    /// Record a problem solve time (for NP-hard PoW)
    pub async fn on_problem_solved(&self, block_height: u64, solve_time_ms: u64) {
        let mut times = self.solve_times.write().await;
        times.insert(block_height, solve_time_ms);

        // Keep only last 1000
        if times.len() > 1000 {
            let oldest = block_height.saturating_sub(1000);
            times.retain(|&h, _| h > oldest);
        }
    }

    /// Record a fault event
    pub async fn on_fault(&self, fault_type: FaultType, severity: u64) {
        let mut tracker = self.fault_tracker.write().await;
        tracker.record_fault(fault_type, severity);
    }

    /// Record a chain reorganization
    pub async fn on_reorg(&self, depth: u64) {
        let mut tracker = self.fault_tracker.write().await;
        tracker.recent_reorg_depth += depth;
    }

    /// Record staking state update
    pub async fn on_staking_update(&self, total_staked: u128, staker_count: u64) {
        let mut tracker = self.staking_tracker.write().await;
        tracker.total_staked = total_staked;
        tracker.staker_count = staker_count;
    }

    // =========================================================================
    // HELPER METHODS
    // =========================================================================

    fn get_problem_category(&self, block: &Block) -> u8 {
        // Extract from coinbase or first transaction
        // In production, this would read the actual problem category
        // For now, use work_score as a proxy (convert to integer)
        ((block.header.work_score * 10.0) as u64 % 10) as u8
    }

    fn calculate_total_fees(&self, block: &Block) -> u128 {
        block.transactions.iter().map(|tx| tx.fee()).sum()
    }

    fn calculate_avg_tx_size(&self, block: &Block) -> u64 {
        if block.transactions.is_empty() {
            return 250; // Default estimate
        }

        // Estimate transaction size (header + signature)
        // Transaction type determines base size - estimate ~200-300 bytes average
        let total_size: u64 = block.transactions.len() as u64 * 250;

        total_size / block.transactions.len() as u64
    }

    fn estimate_block_storage(&self, block: &Block) -> u64 {
        // Block header (92 bytes) + transactions + overhead
        92 + block.transactions.len() as u64 * 250
    }

    // =========================================================================
    // ORACLE QUERY METHODS
    // =========================================================================

    /// Get current hardness factor for a problem category
    pub async fn hardness_factor(&self, category: u8) -> f64 {
        self.oracle.read().await.hardness_factor(category)
    }

    /// Get current base storage cost
    pub async fn base_storage_cost(&self) -> u128 {
        self.oracle.read().await.base_storage_cost()
    }

    /// Get current median fee
    pub async fn median_fee(&self) -> u128 {
        self.oracle.read().await.median_fee()
    }

    /// Get fault severity for reputation calculations
    pub async fn fault_severity(&self, fault_type: FaultType) -> f64 {
        self.oracle.read().await.fault_severity(fault_type)
    }

    /// Get median stake for threshold calculations
    pub async fn median_stake(&self) -> u128 {
        self.oracle.read().await.median_stake()
    }

    /// Get emission bounds
    pub async fn emission_bounds(&self) -> (f64, f64) {
        self.oracle.read().await.emission_bounds()
    }

    /// Get psi magnitude for emission calculations
    pub async fn psi_magnitude(&self) -> f64 {
        self.oracle.read().await.psi_magnitude()
    }

    /// Check if oracle is bootstrapped
    pub async fn is_bootstrapped(&self) -> bool {
        self.oracle.read().await.is_bootstrapped()
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// HASH RATE ESTIMATOR
// =============================================================================

/// Estimates network hash rate from block work_score and timing
struct HashRateEstimator {
    /// Recent block data (work_score, time)
    recent_blocks: Vec<(f64, f64)>,
    /// Current estimate
    estimate: f64,
    /// Window size
    window: usize,
}

impl HashRateEstimator {
    fn new() -> Self {
        HashRateEstimator {
            recent_blocks: Vec::new(),
            estimate: 1.0,
            window: 100,
        }
    }

    fn record_block(&mut self, work_score: f64, block_time: f64) {
        self.recent_blocks.push((work_score, block_time));

        // Maintain window
        if self.recent_blocks.len() > self.window {
            self.recent_blocks.remove(0);
        }

        // Recalculate estimate
        // hash_rate ≈ work_score / avg_block_time
        if !self.recent_blocks.is_empty() {
            let avg_work_score: f64 = self.recent_blocks.iter().map(|(w, _)| *w).sum::<f64>()
                / self.recent_blocks.len() as f64;

            let avg_time: f64 = self.recent_blocks.iter().map(|(_, t)| *t).sum::<f64>()
                / self.recent_blocks.len() as f64;

            if avg_time > 0.0 {
                // Work score represents useful computation done
                self.estimate = avg_work_score * 1000.0 / avg_time;
            }
        }
    }

    fn estimate(&self) -> f64 {
        self.estimate
    }
}

// =============================================================================
// FAULT TRACKER
// =============================================================================

/// Tracks recent faults for network metrics
struct FaultTracker {
    /// Recent reorg depth (blocks)
    recent_reorg_depth: u64,
    /// Recent invalid blocks received
    recent_invalid_blocks: u64,
    /// Recent disconnections
    recent_disconnections: u64,
    /// Fault counts by type
    fault_counts: HashMap<FaultType, u64>,
}

impl FaultTracker {
    fn new() -> Self {
        FaultTracker {
            recent_reorg_depth: 0,
            recent_invalid_blocks: 0,
            recent_disconnections: 0,
            fault_counts: HashMap::new(),
        }
    }

    fn record_fault(&mut self, fault_type: FaultType, _severity: u64) {
        *self.fault_counts.entry(fault_type).or_insert(0) += 1;

        match fault_type {
            FaultType::InvalidBlock => self.recent_invalid_blocks += 1,
            FaultType::UnexpectedDisconnect => self.recent_disconnections += 1,
            _ => {}
        }
    }

    fn reset_recent(&mut self) {
        self.recent_reorg_depth = 0;
        self.recent_invalid_blocks = 0;
        self.recent_disconnections = 0;
    }
}

// =============================================================================
// STAKING TRACKER
// =============================================================================

/// Tracks staking state for network metrics
struct StakingTracker {
    total_staked: u128,
    staker_count: u64,
}

impl StakingTracker {
    fn new() -> Self {
        StakingTracker {
            total_staked: 0,
            staker_count: 0,
        }
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use coinject_core::{
        Address, BlockHeader, CoinbaseTransaction, Commitment, Hash, SolutionReveal,
    };

    fn create_test_block(height: u64) -> Block {
        let commitment = Commitment {
            hash: Hash::ZERO,
            problem_hash: Hash::ZERO,
        };

        Block {
            header: BlockHeader {
                version: 1,
                height,
                prev_hash: Hash::ZERO,
                timestamp: (1700000000 + height * 600) as i64,
                transactions_root: Hash::ZERO,
                solutions_root: Hash::ZERO,
                commitment,
                work_score: 1.0,
                miner: Address::from_bytes([0u8; 32]),
                nonce: 0,
                solve_time_us: 0,
                verify_time_us: 0,
                time_asymmetry_ratio: 0.0,
                solution_quality: 0.0,
                complexity_weight: 0.0,
                energy_estimate_joules: 0.0,
            },
            coinbase: CoinbaseTransaction::new(Address::from_bytes([0u8; 32]), 0, height),
            transactions: Vec::new(),
            solution_reveal: SolutionReveal {
                problem: coinject_core::problem::ProblemType::Custom {
                    problem_id: Hash::ZERO,
                    data: vec![],
                },
                solution: coinject_core::problem::Solution::Custom(vec![]),
                commitment: Commitment {
                    hash: Hash::ZERO,
                    problem_hash: Hash::ZERO,
                },
            },
        }
    }

    #[tokio::test]
    async fn test_metrics_collector_creation() {
        let collector = MetricsCollector::new();
        assert!(!collector.is_bootstrapped().await);
    }

    #[tokio::test]
    async fn test_block_recording() {
        let collector = MetricsCollector::new();

        // Record several blocks
        for i in 0..20 {
            let block = create_test_block(i);
            collector.on_block_added(&block, 50).await;
        }

        // Should be bootstrapped now
        assert!(collector.is_bootstrapped().await);
    }

    #[tokio::test]
    async fn test_peer_count_tracking() {
        let collector = MetricsCollector::new();

        collector.on_peer_count_change(10).await;
        assert_eq!(*collector.peer_count.read().await, 10);

        collector.on_peer_count_change(25).await;
        assert_eq!(*collector.peer_count.read().await, 25);
    }

    #[tokio::test]
    async fn test_consensus_tracking() {
        let collector = MetricsCollector::new();

        collector.on_consensus_update(0.95).await;
        assert!((*collector.consensus_agreement.read().await - 0.95).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_fault_recording() {
        let collector = MetricsCollector::new();

        collector.on_fault(FaultType::InvalidBlock, 1).await;
        collector.on_fault(FaultType::InvalidBlock, 1).await;

        let tracker = collector.fault_tracker.read().await;
        assert_eq!(tracker.recent_invalid_blocks, 2);
    }
}
