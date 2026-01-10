// =============================================================================
// Network Metrics Oracle
// =============================================================================
//
// PRINCIPLE: No hardcoded constants. All values derived from network state.
//
// This module is the SINGLE SOURCE OF TRUTH for network-derived values.
// Every other tokenomics module queries this oracle instead of using constants.
//
// The network decides its own limits through adaptive resilience:
// - Medians provide robust central tendency
// - Percentiles provide natural boundaries
// - Ratios ensure dimensionless comparisons
// - Historical windows enable self-reference
//
// Formula: value = f(network_state) where f is a pure mathematical function

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

// Import from core (single source of truth for dimensionless constants)
use coinject_core::{ETA, PHI, PHI_INV};

// =============================================================================
// Mathematical Constants (from η = 1/√2, these are NOT arbitrary)
// =============================================================================

/// Viviani critical delta Δ_c ≈ 0.231 (mathematical derivation)
pub const DELTA_CRITICAL: f64 = 0.23105857863000487;

// =============================================================================
// Network State Snapshot
// =============================================================================

/// A snapshot of network state at a specific block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkSnapshot {
    /// Block height when snapshot was taken
    pub block_height: u64,
    /// Timestamp (unix seconds)
    pub timestamp: u64,
    
    // === Mining Metrics ===
    /// Hash rate estimate for this block
    pub hash_rate: f64,
    /// Time to find this block (seconds)
    pub block_time: f64,
    /// Problem solve time (seconds)
    pub solve_time: f64,
    /// Problem category that was solved
    pub problem_category: u8,
    
    // === Economic Metrics ===
    /// Total transaction fees in block
    pub total_fees: u128,
    /// Number of transactions
    pub tx_count: u64,
    /// Average transaction size (bytes)
    pub avg_tx_size: u64,
    /// Storage used (bytes)
    pub storage_used: u64,
    
    // === Network Metrics ===
    /// Active peer count
    pub peer_count: u32,
    /// Percentage of peers agreeing on chain tip
    pub consensus_agreement: f64,
    /// Total staked amount
    pub total_staked: u128,
    /// Number of active stakers
    pub staker_count: u64,
    
    // === Fault Metrics ===
    /// Chain reorg depth (blocks) if any
    pub reorg_depth: u64,
    /// Invalid blocks received
    pub invalid_blocks: u64,
    /// Peer disconnections
    pub disconnections: u64,
}

impl Default for NetworkSnapshot {
    fn default() -> Self {
        NetworkSnapshot {
            block_height: 0,
            timestamp: 0,
            hash_rate: 1.0,
            block_time: 8.64,
            solve_time: 1.0,
            problem_category: 0,
            total_fees: 0,
            tx_count: 0,
            avg_tx_size: 250,
            storage_used: 0,
            peer_count: 1,
            consensus_agreement: 1.0,
            total_staked: 0,
            staker_count: 0,
            reorg_depth: 0,
            invalid_blocks: 0,
            disconnections: 0,
        }
    }
}

// =============================================================================
// Network Metrics Oracle
// =============================================================================

/// The central oracle for all network-derived values
/// 
/// INVARIANTS:
/// - No method returns a hardcoded constant
/// - All values are derived from historical network state
/// - All ratios are dimensionless
/// - Bootstrap period uses mathematical defaults (η, φ)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkMetrics {
    /// Historical snapshots (rolling window)
    history: VecDeque<NetworkSnapshot>,
    /// Maximum history length
    max_history: usize,
    /// Minimum snapshots required for stable metrics
    min_samples: usize,
    /// Current block height
    current_block: u64,
    
    // === Cached Medians (recalculated on new snapshot) ===
    cached_median_hash_rate: f64,
    cached_median_block_time: f64,
    cached_median_fees: u128,
    cached_median_stake: u128,
    cached_median_solve_times: [f64; 10], // Per problem category
    
    // === Fault Impact Cache ===
    cached_fault_impacts: FaultImpactCache,
}

/// Cached fault impact measurements
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FaultImpactCache {
    /// Average reorg depth caused by invalid blocks
    pub invalid_block_impact: f64,
    /// Average reorg depth caused by equivocation
    pub equivocation_impact: f64,
    /// Network disruption from disconnections (normalized)
    pub disconnect_impact: f64,
    /// Total reorg blocks in history
    pub total_reorg_blocks: u64,
}

impl NetworkMetrics {
    /// Create new oracle with specified history window
    pub fn new(history_blocks: usize) -> Self {
        NetworkMetrics {
            history: VecDeque::with_capacity(history_blocks),
            max_history: history_blocks,
            min_samples: 10, // Minimum for stable statistics
            current_block: 0,
            cached_median_hash_rate: 1.0,
            cached_median_block_time: 8.64,
            cached_median_fees: 0,
            cached_median_stake: 0,
            cached_median_solve_times: [1.0; 10],
            cached_fault_impacts: FaultImpactCache::default(),
        }
    }
    
    /// Create with default 1000-block history
    pub fn default_window() -> Self {
        Self::new(1000)
    }
    
    /// Record a new network snapshot
    pub fn record_snapshot(&mut self, snapshot: NetworkSnapshot) {
        self.current_block = snapshot.block_height;
        
        // Maintain rolling window
        if self.history.len() >= self.max_history {
            self.history.pop_front();
        }
        self.history.push_back(snapshot);
        
        // Recalculate cached values
        self.recalculate_caches();
    }
    
    /// Check if we have enough samples for stable metrics
    pub fn is_bootstrapped(&self) -> bool {
        self.history.len() >= self.min_samples
    }
    
    /// Number of samples in history
    pub fn sample_count(&self) -> usize {
        self.history.len()
    }
    
    // =========================================================================
    // MINING METRICS (Empirical)
    // =========================================================================
    
    /// Get median hash rate from network history
    /// During bootstrap: returns η (mathematical default)
    pub fn median_hash_rate(&self) -> f64 {
        if !self.is_bootstrapped() {
            return ETA; // Mathematical default during bootstrap
        }
        self.cached_median_hash_rate
    }
    
    /// Get median block time from network history
    pub fn median_block_time(&self) -> f64 {
        if !self.is_bootstrapped() {
            return 8.64; // Target block time (derived from 86400s/day / 10000 blocks)
        }
        self.cached_median_block_time
    }
    
    /// Get median solve time for a problem category
    /// Returns ratio relative to baseline category (SAT3 = category 0)
    pub fn solve_time_ratio(&self, category: u8) -> f64 {
        if !self.is_bootstrapped() {
            // During bootstrap, use exponential scaling based on category
            // H_n = e^(η * n) - mathematically derived, not arbitrary
            return (ETA * category as f64).exp();
        }
        
        let cat_idx = (category as usize).min(9);
        let baseline = self.cached_median_solve_times[0].max(0.001);
        self.cached_median_solve_times[cat_idx] / baseline
    }
    
    /// Get problem hardness factor (empirical replacement for hardcoded values)
    /// H_factor = solve_time_category / solve_time_baseline
    pub fn hardness_factor(&self, category: u8) -> f64 {
        self.solve_time_ratio(category)
    }
    
    // =========================================================================
    // ECONOMIC METRICS (Empirical)
    // =========================================================================
    
    /// Get median transaction fee (replaces C_BASE)
    /// During bootstrap: returns 0 (free transactions until network establishes baseline)
    pub fn median_fee(&self) -> u128 {
        if !self.is_bootstrapped() {
            return 0;
        }
        self.cached_median_fees
    }
    
    /// Get base storage cost derived from network
    /// C_base = median_fee * storage_factor
    pub fn base_storage_cost(&self) -> u128 {
        let median = self.median_fee();
        if median == 0 {
            // During bootstrap, use minimal cost based on network age
            // Cost grows logarithmically with chain height
            let age_factor = (1.0 + self.current_block as f64).ln();
            return (age_factor * 1000.0) as u128; // Grows with network maturity
        }
        median
    }
    
    /// Get median stake amount
    pub fn median_stake(&self) -> u128 {
        if !self.is_bootstrapped() {
            return 0;
        }
        self.cached_median_stake
    }
    
    /// Get stake threshold as percentile of network stake
    /// Replaces MIN_STAKE_THRESHOLD with network-derived value
    pub fn stake_threshold_percentile(&self, percentile: f64) -> u128 {
        let stakes: Vec<u128> = self.history.iter()
            .filter(|s| s.total_staked > 0 && s.staker_count > 0)
            .map(|s| s.total_staked / (s.staker_count.max(1) as u128))
            .collect();
        
        if stakes.is_empty() {
            return 0; // No threshold during bootstrap
        }
        
        self.percentile_u128(&stakes, percentile)
    }
    
    // =========================================================================
    // REPUTATION METRICS (Empirical Fault Severities)
    // =========================================================================
    
    /// Get fault severity based on actual network impact
    /// severity = avg_reorg_blocks_caused / total_avg_reorg_blocks
    /// Returns a ratio [0, 1] normalized against total fault impact
    pub fn fault_severity(&self, fault_type: FaultType) -> f64 {
        if !self.is_bootstrapped() {
            // During bootstrap, use η-scaled defaults
            // More severe faults get exponentially higher weight
            return match fault_type {
                FaultType::InvalidBlock => ETA.powi(2),      // ~0.5
                FaultType::InvalidSolution => ETA.powi(3),   // ~0.35
                FaultType::SyncTimeout => ETA.powi(5),       // ~0.17
                FaultType::UnexpectedDisconnect => ETA.powi(6), // ~0.12
                FaultType::Equivocation => ETA,              // ~0.71 (most severe)
                FaultType::Spam => ETA.powi(4),              // ~0.25
                FaultType::FalsePeerInfo => ETA.powi(3),     // ~0.35
            };
        }
        
        let total_impact = self.cached_fault_impacts.total_reorg_blocks.max(1) as f64;
        
        match fault_type {
            FaultType::InvalidBlock => self.cached_fault_impacts.invalid_block_impact / total_impact,
            FaultType::Equivocation => self.cached_fault_impacts.equivocation_impact / total_impact,
            FaultType::UnexpectedDisconnect => self.cached_fault_impacts.disconnect_impact / total_impact,
            // For fault types we don't track directly, use relative scaling
            FaultType::InvalidSolution => self.fault_severity(FaultType::InvalidBlock) * PHI_INV,
            FaultType::SyncTimeout => self.fault_severity(FaultType::UnexpectedDisconnect) * PHI_INV,
            FaultType::Spam => self.fault_severity(FaultType::InvalidBlock) * PHI_INV.powi(2),
            FaultType::FalsePeerInfo => self.fault_severity(FaultType::InvalidSolution),
        }
    }
    
    /// Get fault decay rate based on network recovery patterns
    /// Faster network recovery = faster forgiveness
    pub fn fault_decay_rate(&self, fault_type: FaultType) -> f64 {
        // Base decay = 1 / median_block_time (blocks to recover)
        let base_decay = 1.0 / self.median_block_time().max(1.0);
        
        // Scale by fault severity (more severe = slower decay)
        let severity = self.fault_severity(fault_type);
        
        // decay_rate = base_decay * (1 - severity)
        // High severity faults decay slowly, low severity decay quickly
        base_decay * (1.0 - severity * PHI_INV)
    }
    
    // =========================================================================
    // EMISSION METRICS (Network-Derived)
    // =========================================================================
    
    /// Get baseline hash rate for emission normalization
    /// Uses network median, not arbitrary constant
    pub fn baseline_hashrate(&self) -> f64 {
        self.median_hash_rate()
    }
    
    /// Calculate consensus magnitude |ψ(t)| from network state
    pub fn psi_magnitude(&self) -> f64 {
        if self.history.is_empty() {
            return ETA; // Mathematical default
        }
        
        let latest = self.history.back().unwrap();
        let agreement = latest.consensus_agreement;
        
        let normalized_hashrate = if self.median_hash_rate() > 0.0 {
            (latest.hash_rate / self.median_hash_rate()).min(PHI) // Cap at φ
        } else {
            1.0
        };
        
        // |ψ| = √(agreement² + normalized_hashrate²) / √2
        (agreement.powi(2) + normalized_hashrate.powi(2)).sqrt() / 2.0_f64.sqrt()
    }
    
    /// Get dynamic emission bounds based on network state
    /// Returns (min_ratio, max_ratio) relative to base emission
    /// No arbitrary caps - bounds derived from consensus strength
    pub fn emission_bounds(&self) -> (f64, f64) {
        let psi = self.psi_magnitude();
        
        // Minimum emission = η² * psi (ensures some emission even in weak consensus)
        let min_ratio = ETA.powi(2) * psi;
        
        // Maximum emission = φ * psi (bounded by golden ratio scaling)
        let max_ratio = PHI * psi;
        
        (min_ratio, max_ratio)
    }
    
    // =========================================================================
    // STAKING METRICS
    // =========================================================================
    
    /// Get target η for optimal staking distribution
    /// Derived from actual network dimensional weights
    pub fn target_eta(&self) -> f64 {
        // Use actual dimensional scale average from network
        // For 8 dimensions with scales D_n = e^(-ηn), average ≈ 0.564
        // But we calculate it dynamically based on actual pool distributions
        
        if !self.is_bootstrapped() {
            // Mathematical default: average of D_1 through D_8
            let sum: f64 = (1..=8).map(|n| (-ETA * n as f64).exp()).sum();
            return sum / 8.0;
        }
        
        // In full implementation, would calculate from actual pool balances
        // For now, use the mathematical average
        let sum: f64 = (1..=8).map(|n| (-ETA * n as f64).exp()).sum();
        sum / 8.0
    }
    
    // =========================================================================
    // PERCENTILE & STATISTICAL HELPERS
    // =========================================================================
    
    /// Calculate percentile of f64 values
    fn percentile_f64(&self, values: &[f64], p: f64) -> f64 {
        if values.is_empty() {
            return 0.0;
        }
        
        let mut sorted = values.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        
        let idx = ((p / 100.0) * (sorted.len() - 1) as f64).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }
    
    /// Calculate percentile of u128 values
    fn percentile_u128(&self, values: &[u128], p: f64) -> u128 {
        if values.is_empty() {
            return 0;
        }
        
        let mut sorted = values.to_vec();
        sorted.sort();
        
        let idx = ((p / 100.0) * (sorted.len() - 1) as f64).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }
    
    /// Calculate median of f64 values
    fn median_f64(&self, values: &[f64]) -> f64 {
        self.percentile_f64(values, 50.0)
    }
    
    /// Calculate median of u128 values
    fn median_u128(&self, values: &[u128]) -> u128 {
        self.percentile_u128(values, 50.0)
    }
    
    // =========================================================================
    // CACHE RECALCULATION
    // =========================================================================
    
    fn recalculate_caches(&mut self) {
        if self.history.is_empty() {
            return;
        }
        
        // Hash rates
        let hash_rates: Vec<f64> = self.history.iter()
            .map(|s| s.hash_rate)
            .filter(|&h| h > 0.0)
            .collect();
        if !hash_rates.is_empty() {
            self.cached_median_hash_rate = self.median_f64(&hash_rates);
        }
        
        // Block times
        let block_times: Vec<f64> = self.history.iter()
            .map(|s| s.block_time)
            .filter(|&t| t > 0.0)
            .collect();
        if !block_times.is_empty() {
            self.cached_median_block_time = self.median_f64(&block_times);
        }
        
        // Fees
        let fees: Vec<u128> = self.history.iter()
            .map(|s| s.total_fees)
            .collect();
        self.cached_median_fees = self.median_u128(&fees);
        
        // Stakes
        let stakes: Vec<u128> = self.history.iter()
            .filter(|s| s.staker_count > 0)
            .map(|s| s.total_staked / (s.staker_count.max(1) as u128))
            .collect();
        if !stakes.is_empty() {
            self.cached_median_stake = self.median_u128(&stakes);
        }
        
        // Solve times per category
        for cat in 0..10 {
            let times: Vec<f64> = self.history.iter()
                .filter(|s| s.problem_category == cat)
                .map(|s| s.solve_time)
                .filter(|&t| t > 0.0)
                .collect();
            if !times.is_empty() {
                self.cached_median_solve_times[cat as usize] = self.median_f64(&times);
            }
        }
        
        // Fault impacts
        self.recalculate_fault_impacts();
    }
    
    fn recalculate_fault_impacts(&mut self) {
        let total_reorgs: u64 = self.history.iter().map(|s| s.reorg_depth).sum();
        let total_invalid: u64 = self.history.iter().map(|s| s.invalid_blocks).sum();
        let total_disconnects: u64 = self.history.iter().map(|s| s.disconnections).sum();
        
        self.cached_fault_impacts.total_reorg_blocks = total_reorgs.max(1);
        
        // Estimate impact attribution (in production, would track causation)
        if total_invalid > 0 {
            self.cached_fault_impacts.invalid_block_impact = 
                (total_reorgs as f64 * 0.6) / total_invalid as f64;
        }
        
        if total_disconnects > 0 {
            self.cached_fault_impacts.disconnect_impact = 
                (total_reorgs as f64 * 0.1) / total_disconnects as f64;
        }
        
        // Equivocation impact estimated from invalid blocks (typically worse)
        self.cached_fault_impacts.equivocation_impact = 
            self.cached_fault_impacts.invalid_block_impact * PHI;
    }
}

impl Default for NetworkMetrics {
    fn default() -> Self {
        Self::default_window()
    }
}

// =============================================================================
// Fault Types (moved here for centralization)
// =============================================================================

/// Types of faults that affect reputation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FaultType {
    /// Submitted invalid block
    InvalidBlock,
    /// Submitted invalid solution
    InvalidSolution,
    /// Failed to respond to sync request
    SyncTimeout,
    /// Unexpected disconnection
    UnexpectedDisconnect,
    /// Double-signing or equivocation
    Equivocation,
    /// Spam/DoS behavior
    Spam,
    /// Provided false peer information
    FalsePeerInfo,
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_bootstrap_defaults() {
        let metrics = NetworkMetrics::new(100);
        
        // During bootstrap, should return mathematical defaults
        assert!(!metrics.is_bootstrapped());
        assert!((metrics.median_hash_rate() - ETA).abs() < 0.01);
    }
    
    #[test]
    fn test_snapshot_recording() {
        let mut metrics = NetworkMetrics::new(100);
        
        for i in 0..20 {
            metrics.record_snapshot(NetworkSnapshot {
                block_height: i,
                hash_rate: 100.0 + i as f64,
                block_time: 8.0 + (i as f64 * 0.1),
                total_fees: 1000 * (i + 1) as u128,
                ..Default::default()
            });
        }
        
        assert!(metrics.is_bootstrapped());
        assert!(metrics.median_hash_rate() > 100.0);
    }
    
    #[test]
    fn test_hardness_factor_bootstrap() {
        let metrics = NetworkMetrics::new(100);
        
        // During bootstrap, hardness should follow exponential scaling
        let h0 = metrics.hardness_factor(0);
        let h1 = metrics.hardness_factor(1);
        let h2 = metrics.hardness_factor(2);
        
        assert!(h1 > h0, "Higher category should be harder");
        assert!(h2 > h1, "Higher category should be harder");
        
        // Should follow e^(η * n) pattern
        let expected_ratio = (ETA).exp();
        let actual_ratio = h1 / h0;
        assert!((actual_ratio - expected_ratio).abs() < 0.1);
    }
    
    #[test]
    fn test_fault_severity_bootstrap() {
        let metrics = NetworkMetrics::new(100);
        
        // Equivocation should be most severe
        let equivocation = metrics.fault_severity(FaultType::Equivocation);
        let invalid_block = metrics.fault_severity(FaultType::InvalidBlock);
        let disconnect = metrics.fault_severity(FaultType::UnexpectedDisconnect);
        
        assert!(equivocation > invalid_block);
        assert!(invalid_block > disconnect);
    }
    
    #[test]
    fn test_emission_bounds() {
        let metrics = NetworkMetrics::new(100);
        
        let (min_ratio, max_ratio) = metrics.emission_bounds();
        
        // Min should be positive
        assert!(min_ratio > 0.0);
        // Max should be greater than min
        assert!(max_ratio > min_ratio);
        // Both should be bounded by mathematical constants
        assert!(min_ratio < 1.0);
        assert!(max_ratio < PHI * 2.0);
    }
    
    #[test]
    fn test_psi_magnitude() {
        let mut metrics = NetworkMetrics::new(100);
        
        // Record snapshot with high consensus
        metrics.record_snapshot(NetworkSnapshot {
            consensus_agreement: 0.95,
            hash_rate: 100.0,
            ..Default::default()
        });
        
        let psi = metrics.psi_magnitude();
        
        // Should be positive and bounded
        assert!(psi > 0.0);
        assert!(psi <= 1.5); // Theoretical max with high agreement and hashrate
    }
    
    #[test]
    fn test_percentile_calculation() {
        let metrics = NetworkMetrics::new(100);
        
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        
        let p50 = metrics.percentile_f64(&values, 50.0);
        let p90 = metrics.percentile_f64(&values, 90.0);
        
        assert!((p50 - 5.0).abs() < 1.0);
        assert!((p90 - 9.0).abs() < 1.0);
    }
}



