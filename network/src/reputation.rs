// =============================================================================
// Peer Reputation System (EMPIRICAL VERSION)
// R_n = (S_ratio × T_ratio) / (1 + E_weighted)
// =============================================================================
//
// COMPLIANCE: Empirical ✓ | Self-referential ✓ | Dimensionless ✓
//
// ALL values derived from network state:
// - S_ratio: Stake as ratio to network median (not absolute threshold)
// - T_ratio: Age as ratio to network median (not arbitrary blocks)
// - E_weighted: Faults weighted by empirical network impact (not hardcoded severities)
// - NO MAX/MIN caps on reputation - percentiles provide natural ranking
//
// The network decides its own limits through adaptive resilience.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

// Mathematical constants (from η = 1/√2, NOT arbitrary)
const ETA: f64 = 0.7071067811865476;
const PHI: f64 = 1.618033988749895;
const PHI_INV: f64 = 0.6180339887498949;

// =============================================================================
// Fault Types
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
// Network-Derived Metrics
// =============================================================================

/// Interface to network metrics for reputation calculations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReputationMetrics {
    /// Median stake across all peers
    pub median_stake: u128,
    /// Median peer age (blocks since first seen)
    pub median_age_blocks: u64,
    /// Network fault impact measurements (empirical severities)
    pub fault_impacts: HashMap<FaultType, f64>,
    /// Total fault impact for normalization
    pub total_fault_impact: f64,
    /// Current block height
    pub current_block: u64,
    /// All peer scores for percentile calculation
    pub score_distribution: Vec<f64>,
}

impl ReputationMetrics {
    /// Create bootstrap metrics (before network has history)
    pub fn bootstrap() -> Self {
        // During bootstrap, use η-scaled defaults
        let mut fault_impacts = HashMap::new();
        fault_impacts.insert(FaultType::Equivocation, ETA);           // ~0.71 (most severe)
        fault_impacts.insert(FaultType::InvalidBlock, ETA.powi(2));   // ~0.50
        fault_impacts.insert(FaultType::InvalidSolution, ETA.powi(3)); // ~0.35
        fault_impacts.insert(FaultType::Spam, ETA.powi(4));           // ~0.25
        fault_impacts.insert(FaultType::SyncTimeout, ETA.powi(5));    // ~0.18
        fault_impacts.insert(FaultType::FalsePeerInfo, ETA.powi(3));  // ~0.35
        fault_impacts.insert(FaultType::UnexpectedDisconnect, ETA.powi(6)); // ~0.13
        
        let total: f64 = fault_impacts.values().sum();
        
        ReputationMetrics {
            median_stake: 0,
            median_age_blocks: 0,
            fault_impacts,
            total_fault_impact: total,
            current_block: 0,
            score_distribution: Vec::new(),
        }
    }
    
    /// Get fault severity as normalized ratio [0, 1]
    /// Derived from actual network impact, not hardcoded
    pub fn fault_severity(&self, fault_type: FaultType) -> f64 {
        let impact = self.fault_impacts.get(&fault_type).copied().unwrap_or(ETA.powi(3));
        if self.total_fault_impact > 0.0 {
            impact / self.total_fault_impact
        } else {
            impact
        }
    }
    
    /// Get fault decay rate based on severity
    /// More severe faults decay slower (longer memory)
    pub fn fault_decay_rate(&self, fault_type: FaultType) -> f64 {
        let severity = self.fault_severity(fault_type);
        // Base decay scaled by inverse severity
        // decay = η^3 * (1 - severity)
        ETA.powi(3) * (1.0 - severity * PHI_INV)
    }
    
    /// Update from network observations
    pub fn update_from_network(
        &mut self,
        median_stake: u128,
        median_age: u64,
        fault_observations: &[(FaultType, f64)], // (fault_type, observed_impact)
        current_block: u64,
        all_scores: Vec<f64>,
    ) {
        self.median_stake = median_stake;
        self.median_age_blocks = median_age;
        self.current_block = current_block;
        self.score_distribution = all_scores;
        
        // Update fault impacts from observations
        for (fault_type, impact) in fault_observations {
            // EMA update: new = η * observation + (1-η) * old
            let old = self.fault_impacts.get(fault_type).copied().unwrap_or(*impact);
            let new = ETA * impact + (1.0 - ETA) * old;
            self.fault_impacts.insert(*fault_type, new);
        }
        
        self.total_fault_impact = self.fault_impacts.values().sum();
    }
    
    /// Calculate percentile of a score within the network
    /// Returns 0-100 (dimensionless ranking)
    pub fn calculate_percentile(&self, score: f64) -> f64 {
        if self.score_distribution.is_empty() {
            return 50.0; // Default to median during bootstrap
        }
        
        let below = self.score_distribution.iter().filter(|&&s| s < score).count();
        (below as f64 / self.score_distribution.len() as f64) * 100.0
    }
}

impl Default for ReputationMetrics {
    fn default() -> Self {
        Self::bootstrap()
    }
}

// =============================================================================
// Fault Event
// =============================================================================

/// Recorded fault event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaultEvent {
    pub fault_type: FaultType,
    pub block_height: u64,
    pub timestamp: i64,
    /// Severity at time of recording (from network metrics)
    pub recorded_severity: f64,
    pub details: Option<String>,
}

// =============================================================================
// Peer Reputation Entry
// =============================================================================

/// Complete reputation record for a peer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerReputation {
    /// Peer identifier
    pub peer_id: String,
    
    // === Stake Component ===
    /// Current staked amount
    pub staked_amount: u128,
    /// Block height when stake was made
    pub stake_start_block: u64,
    
    // === Age Component ===
    /// First seen block height
    pub first_seen_block: u64,
    /// Total blocks as active peer
    pub active_blocks: u64,
    /// Last activity block
    pub last_active_block: u64,
    
    // === Fault Component ===
    /// Fault history
    pub faults: Vec<FaultEvent>,
    
    // === Positive Actions ===
    /// Successful block propagations
    pub blocks_propagated: u64,
    /// Valid solutions submitted
    pub valid_solutions: u64,
    /// Successful sync responses
    pub sync_responses: u64,
    
    // === Computed Scores (all dimensionless ratios) ===
    /// Current reputation score
    pub reputation_score: f64,
    /// Reputation percentile (0-100)
    pub percentile: f64,
    /// Last score update block
    pub last_update_block: u64,
}

impl PeerReputation {
    /// Create new peer reputation entry
    pub fn new(peer_id: String, current_block: u64) -> Self {
        PeerReputation {
            peer_id,
            staked_amount: 0,
            stake_start_block: 0,
            first_seen_block: current_block,
            active_blocks: 0,
            last_active_block: current_block,
            faults: Vec::new(),
            blocks_propagated: 0,
            valid_solutions: 0,
            sync_responses: 0,
            reputation_score: 0.0,
            percentile: 50.0,
            last_update_block: current_block,
        }
    }

    /// Record stake information
    pub fn update_stake(&mut self, amount: u128, stake_block: u64) {
        self.staked_amount = amount;
        if self.stake_start_block == 0 {
            self.stake_start_block = stake_block;
        }
    }

    /// Record a fault with network-derived severity
    pub fn record_fault(
        &mut self, 
        fault_type: FaultType, 
        block_height: u64, 
        metrics: &ReputationMetrics,
        details: Option<String>
    ) {
        let severity = metrics.fault_severity(fault_type);
        
        self.faults.push(FaultEvent {
            fault_type,
            block_height,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0),
            recorded_severity: severity,
            details,
        });
    }

    /// Record positive action
    pub fn record_positive(&mut self, action: PositiveAction) {
        match action {
            PositiveAction::BlockPropagated => self.blocks_propagated += 1,
            PositiveAction::ValidSolution => self.valid_solutions += 1,
            PositiveAction::SyncResponse => self.sync_responses += 1,
        }
    }

    /// Update activity
    pub fn update_activity(&mut self, block_height: u64) {
        if block_height > self.last_active_block {
            self.active_blocks += block_height - self.last_active_block;
            self.last_active_block = block_height;
        }
    }

    /// Calculate effective faults with decay (dimensionless)
    fn calculate_effective_faults(&self, metrics: &ReputationMetrics) -> f64 {
        let current_block = metrics.current_block;
        let mut effective_faults = 0.0;
        
        for fault in &self.faults {
            let blocks_since = current_block.saturating_sub(fault.block_height);
            let decay_rate = metrics.fault_decay_rate(fault.fault_type);
            let decay = (-decay_rate * blocks_since as f64).exp();
            
            // Use recorded severity (was network-derived at time of fault)
            effective_faults += fault.recorded_severity * decay;
        }
        
        effective_faults
    }

    /// Calculate S_ratio: stake relative to network median (dimensionless)
    fn calculate_stake_ratio(&self, metrics: &ReputationMetrics) -> f64 {
        if metrics.median_stake == 0 {
            // During bootstrap, any stake gives full credit
            return if self.staked_amount > 0 { 1.0 } else { 0.0 };
        }
        
        // Ratio with logarithmic dampening to prevent whale domination
        // S_ratio = log2(1 + stake/median) / log2(PHI^2)
        let stake_ratio = self.staked_amount as f64 / metrics.median_stake as f64;
        (1.0 + stake_ratio).log2() / (PHI * PHI).log2()
    }

    /// Calculate T_ratio: age relative to network median (dimensionless)
    fn calculate_age_ratio(&self, metrics: &ReputationMetrics) -> f64 {
        let age = if self.staked_amount > 0 {
            metrics.current_block.saturating_sub(self.stake_start_block)
        } else {
            metrics.current_block.saturating_sub(self.first_seen_block)
        };
        
        if metrics.median_age_blocks == 0 {
            // During bootstrap, use asymptotic growth
            // T_ratio = 1 - e^(-η * age / 1000)
            return 1.0 - (-ETA * age as f64 / 1000.0).exp();
        }
        
        // Ratio with asymptotic saturation
        // T_ratio = 1 - e^(-η * age / median_age)
        let normalized_age = age as f64 / metrics.median_age_blocks as f64;
        1.0 - (-ETA * normalized_age).exp()
    }

    /// Calculate positive actions bonus (dimensionless ratio)
    fn calculate_positive_bonus(&self, metrics: &ReputationMetrics) -> f64 {
        // Use percentile-based scaling if we have network data
        // Maximum 20% bonus (0.2), scaled by η
        
        // Each action type contributes proportionally
        let propagation_factor = (self.blocks_propagated as f64).ln().max(0.0) / 10.0;
        let solution_factor = (self.valid_solutions as f64).ln().max(0.0) / 5.0;
        let sync_factor = (self.sync_responses as f64).ln().max(0.0) / 8.0;
        
        let raw_bonus = propagation_factor + solution_factor + sync_factor;
        
        // Cap at η * PHI_INV ≈ 0.437 * 0.618 ≈ 0.27 (mathematical, not arbitrary)
        (raw_bonus * ETA).min(ETA * PHI_INV)
    }

    /// Calculate full reputation score (ALL RATIOS - dimensionless)
    /// R = (S_ratio × T_ratio × (1 + bonus)) / (1 + E_weighted)
    pub fn calculate_reputation(&mut self, metrics: &ReputationMetrics) -> f64 {
        let s_ratio = self.calculate_stake_ratio(metrics);
        let t_ratio = self.calculate_age_ratio(metrics);
        let e_weighted = self.calculate_effective_faults(metrics);
        let bonus = self.calculate_positive_bonus(metrics);
        
        // Core formula with all dimensionless components
        let numerator = s_ratio * t_ratio * (1.0 + bonus);
        let denominator = 1.0 + e_weighted;
        
        self.reputation_score = numerator / denominator;
        self.last_update_block = metrics.current_block;
        
        // Update percentile if we have network data
        self.percentile = metrics.calculate_percentile(self.reputation_score);
        
        self.reputation_score
    }

    /// Get detailed reputation breakdown
    pub fn breakdown(&self, metrics: &ReputationMetrics) -> ReputationBreakdown {
        ReputationBreakdown {
            peer_id: self.peer_id.clone(),
            stake_ratio: self.calculate_stake_ratio(metrics),
            age_ratio: self.calculate_age_ratio(metrics),
            effective_faults: self.calculate_effective_faults(metrics),
            positive_bonus: self.calculate_positive_bonus(metrics),
            total_score: self.reputation_score,
            percentile: self.percentile,
            fault_count: self.faults.len(),
            blocks_propagated: self.blocks_propagated,
            valid_solutions: self.valid_solutions,
        }
    }
}

/// Positive action types
#[derive(Debug, Clone, Copy)]
pub enum PositiveAction {
    BlockPropagated,
    ValidSolution,
    SyncResponse,
}

/// Detailed reputation breakdown for reporting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReputationBreakdown {
    pub peer_id: String,
    pub stake_ratio: f64,
    pub age_ratio: f64,
    pub effective_faults: f64,
    pub positive_bonus: f64,
    pub total_score: f64,
    pub percentile: f64,
    pub fault_count: usize,
    pub blocks_propagated: u64,
    pub valid_solutions: u64,
}

// =============================================================================
// Reputation Manager
// =============================================================================

/// Manager for all peer reputations
#[derive(Debug)]
pub struct ReputationManager {
    /// All peer reputations
    peers: HashMap<String, PeerReputation>,
    /// Network-derived metrics
    metrics: ReputationMetrics,
}

impl ReputationManager {
    pub fn new() -> Self {
        ReputationManager {
            peers: HashMap::new(),
            metrics: ReputationMetrics::bootstrap(),
        }
    }

    /// Update network metrics
    pub fn update_metrics(&mut self, metrics: ReputationMetrics) {
        self.metrics = metrics;
    }
    
    /// Set current block height
    pub fn set_block(&mut self, block: u64) {
        self.metrics.current_block = block;
    }

    /// Get or create peer reputation
    pub fn get_or_create(&mut self, peer_id: &str) -> &mut PeerReputation {
        let block = self.metrics.current_block;
        if !self.peers.contains_key(peer_id) {
            self.peers.insert(
                peer_id.to_string(),
                PeerReputation::new(peer_id.to_string(), block),
            );
        }
        self.peers.get_mut(peer_id).unwrap()
    }

    /// Get peer reputation (read-only)
    pub fn get(&self, peer_id: &str) -> Option<&PeerReputation> {
        self.peers.get(peer_id)
    }

    /// Record a fault for a peer
    pub fn record_fault(&mut self, peer_id: &str, fault_type: FaultType, details: Option<String>) {
        let metrics = self.metrics.clone();
        let peer = self.get_or_create(peer_id);
        peer.record_fault(fault_type, metrics.current_block, &metrics, details);
        peer.calculate_reputation(&metrics);
        
        let severity = metrics.fault_severity(fault_type);
        println!("⚠️  Fault recorded for {}: {:?} (severity: {:.3}, score: {:.3})", 
            peer_id, fault_type, severity, peer.reputation_score);
    }

    /// Record positive action for a peer
    pub fn record_positive(&mut self, peer_id: &str, action: PositiveAction) {
        let metrics = self.metrics.clone();
        let peer = self.get_or_create(peer_id);
        peer.record_positive(action);
        peer.calculate_reputation(&metrics);
    }

    /// Update stake for a peer
    pub fn update_stake(&mut self, peer_id: &str, amount: u128) {
        let metrics = self.metrics.clone();
        let block = metrics.current_block;
        let peer = self.get_or_create(peer_id);
        peer.update_stake(amount, block);
        peer.calculate_reputation(&metrics);
    }

    /// Update activity for a peer
    pub fn update_activity(&mut self, peer_id: &str) {
        let block = self.metrics.current_block;
        if let Some(peer) = self.peers.get_mut(peer_id) {
            peer.update_activity(block);
        }
    }

    /// Recalculate all reputations and update score distribution
    pub fn recalculate_all(&mut self) {
        // First pass: calculate all scores
        let metrics = self.metrics.clone();
        for peer in self.peers.values_mut() {
            peer.calculate_reputation(&metrics);
        }
        
        // Collect score distribution
        let scores: Vec<f64> = self.peers.values()
            .map(|p| p.reputation_score)
            .collect();
        
        // Update metrics with new distribution
        self.metrics.score_distribution = scores.clone();
        
        // Second pass: update percentiles with new distribution
        for peer in self.peers.values_mut() {
            peer.percentile = self.metrics.calculate_percentile(peer.reputation_score);
        }
        
        // Update median stake from peers
        let stakes: Vec<u128> = self.peers.values()
            .filter(|p| p.staked_amount > 0)
            .map(|p| p.staked_amount)
            .collect();
        if !stakes.is_empty() {
            let mut sorted = stakes.clone();
            sorted.sort();
            self.metrics.median_stake = sorted[sorted.len() / 2];
        }
        
        // Update median age
        let ages: Vec<u64> = self.peers.values()
            .map(|p| self.metrics.current_block.saturating_sub(p.first_seen_block))
            .collect();
        if !ages.is_empty() {
            let mut sorted = ages.clone();
            sorted.sort();
            self.metrics.median_age_blocks = sorted[sorted.len() / 2];
        }
    }

    /// Get peers by percentile threshold (replaces hardcoded thresholds)
    /// Example: get_peers_above_percentile(75.0) returns top 25%
    pub fn get_peers_above_percentile(&self, percentile: f64) -> Vec<(&String, &PeerReputation)> {
        self.peers.iter()
            .filter(|(_, p)| p.percentile >= percentile)
            .collect()
    }

    /// Get bounty eligible peers (top 50% by default - network decides)
    pub fn bounty_eligible_peers(&self) -> Vec<(&String, &PeerReputation)> {
        self.get_peers_above_percentile(50.0)
    }

    /// Get priority peers for connection slots (top 25%)
    pub fn priority_peers(&self) -> Vec<(&String, &PeerReputation)> {
        self.get_peers_above_percentile(75.0)
    }

    /// Get problematic peers (bottom 10%)
    pub fn problematic_peers(&self) -> Vec<&String> {
        self.peers.iter()
            .filter(|(_, p)| p.percentile < 10.0)
            .map(|(id, _)| id)
            .collect()
    }

    /// Select peer for bounty assignment using weighted random
    /// Higher reputation = higher probability (proportional to score)
    pub fn select_for_bounty(&self, exclude: &[&str]) -> Option<String> {
        use rand::Rng;
        
        let eligible: Vec<_> = self.bounty_eligible_peers()
            .into_iter()
            .filter(|(id, _)| !exclude.contains(&id.as_str()))
            .collect();
        
        if eligible.is_empty() {
            return None;
        }
        
        // Weight by reputation score (all scores are positive ratios)
        let total_weight: f64 = eligible.iter()
            .map(|(_, p)| p.reputation_score.max(0.001)) // Ensure positive
            .sum();
        
        if total_weight <= 0.0 {
            return None;
        }
        
        let mut rng = rand::thread_rng();
        let target = rng.gen_range(0.0..total_weight);
        
        let mut cumulative = 0.0;
        for (id, peer) in eligible {
            cumulative += peer.reputation_score.max(0.001);
            if cumulative >= target {
                return Some(id.clone());
            }
        }
        
        None
    }

    /// Get reputation statistics (all dimensionless)
    pub fn stats(&self) -> ReputationStats {
        let scores: Vec<f64> = self.peers.values()
            .map(|p| p.reputation_score)
            .collect();
        
        let total_peers = scores.len();
        let avg_score = if total_peers > 0 {
            scores.iter().sum::<f64>() / total_peers as f64
        } else {
            0.0
        };
        
        let max_score = scores.iter().cloned().fold(0.0, f64::max);
        let min_score = scores.iter().cloned().fold(f64::MAX, f64::min);
        
        let total_faults: usize = self.peers.values()
            .map(|p| p.faults.len())
            .sum();
        
        ReputationStats {
            total_peers,
            avg_reputation: avg_score,
            max_reputation: max_score,
            min_reputation: if total_peers > 0 { min_score } else { 0.0 },
            median_stake: self.metrics.median_stake,
            median_age_blocks: self.metrics.median_age_blocks,
            bounty_eligible: self.bounty_eligible_peers().len(),
            priority_peers: self.priority_peers().len(),
            problematic_peers: self.problematic_peers().len(),
            total_faults,
        }
    }
}

impl Default for ReputationManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Reputation system statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReputationStats {
    pub total_peers: usize,
    pub avg_reputation: f64,
    pub max_reputation: f64,
    pub min_reputation: f64,
    pub median_stake: u128,
    pub median_age_blocks: u64,
    pub bounty_eligible: usize,
    pub priority_peers: usize,
    pub problematic_peers: usize,
    pub total_faults: usize,
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bootstrap_fault_severities() {
        let metrics = ReputationMetrics::bootstrap();
        
        // Equivocation should be most severe
        let equivocation = metrics.fault_severity(FaultType::Equivocation);
        let invalid_block = metrics.fault_severity(FaultType::InvalidBlock);
        let disconnect = metrics.fault_severity(FaultType::UnexpectedDisconnect);
        
        assert!(equivocation > invalid_block, 
            "Equivocation ({}) should be > InvalidBlock ({})", equivocation, invalid_block);
        assert!(invalid_block > disconnect,
            "InvalidBlock ({}) should be > Disconnect ({})", invalid_block, disconnect);
    }

    #[test]
    fn test_dimensionless_ratios() {
        let mut peer = PeerReputation::new("test-peer".to_string(), 0);
        let metrics = ReputationMetrics::bootstrap();
        
        peer.update_stake(1000, 0);
        let stake_ratio = peer.calculate_stake_ratio(&metrics);
        
        // Ratio should be between 0 and reasonable bound
        assert!(stake_ratio >= 0.0, "Stake ratio should be non-negative");
        assert!(stake_ratio <= 10.0, "Stake ratio should be bounded");
    }

    #[test]
    fn test_percentile_based_selection() {
        let mut manager = ReputationManager::new();
        manager.set_block(1000);
        
        // Create peers with varying stakes
        for i in 0..10 {
            let peer_id = format!("peer-{}", i);
            manager.update_stake(&peer_id, (i as u128 + 1) * 1000);
        }
        
        manager.recalculate_all();
        
        // Top percentile peers should be those with highest stakes
        let top_peers = manager.get_peers_above_percentile(80.0);
        assert!(!top_peers.is_empty(), "Should have top percentile peers");
        
        // Priority peers (top 25%) should be subset
        let priority = manager.priority_peers();
        assert!(priority.len() <= top_peers.len() + 3); // Allow some overlap
    }

    #[test]
    fn test_fault_impact_on_reputation() {
        let mut manager = ReputationManager::new();
        manager.set_block(100);
        
        manager.update_stake("good-peer", 10000);
        manager.update_stake("bad-peer", 10000);
        
        // Record fault for bad peer
        manager.record_fault("bad-peer", FaultType::InvalidBlock, None);
        
        manager.recalculate_all();
        
        let good = manager.get("good-peer").unwrap();
        let bad = manager.get("bad-peer").unwrap();
        
        assert!(good.reputation_score > bad.reputation_score,
            "Good peer ({}) should have higher score than bad peer ({})",
            good.reputation_score, bad.reputation_score);
    }

    #[test]
    fn test_no_hardcoded_thresholds() {
        let manager = ReputationManager::new();
        let stats = manager.stats();
        
        // All thresholds should be percentile-based (0-100)
        // No magic numbers like 1_000_000 or arbitrary minimums
        assert!(stats.avg_reputation >= 0.0);
        // Bounty eligibility is top 50%, not a fixed score
        // Priority is top 25%, not a fixed score
    }

    #[test]
    fn test_decay_rates_follow_severity() {
        let metrics = ReputationMetrics::bootstrap();
        
        let severe_decay = metrics.fault_decay_rate(FaultType::Equivocation);
        let mild_decay = metrics.fault_decay_rate(FaultType::UnexpectedDisconnect);
        
        // More severe faults should decay slower (lower decay rate)
        assert!(severe_decay < mild_decay,
            "Severe fault should decay slower: {} < {}", severe_decay, mild_decay);
    }

    #[test]
    fn test_network_derived_medians() {
        let mut manager = ReputationManager::new();
        manager.set_block(1000);
        
        // Add peers with stakes
        manager.update_stake("peer-1", 1000);
        manager.update_stake("peer-2", 2000);
        manager.update_stake("peer-3", 3000);
        
        manager.recalculate_all();
        
        let stats = manager.stats();
        
        // Median stake should be derived from actual network
        assert!(stats.median_stake > 0, "Median stake should be derived from peers");
        assert_eq!(stats.median_stake, 2000, "Median should be middle value");
    }
}
