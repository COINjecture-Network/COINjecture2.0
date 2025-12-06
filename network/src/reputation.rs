// =============================================================================
// Peer Reputation System
// R_n = (S_token × T_age) / (1 + E_faults)
// =============================================================================
//
// Institutional-grade reputation scoring for peer selection:
// - S_token: Staked COIN amount (economic commitment)
// - T_age: Duration of stake (time commitment)
// - E_faults: Invalid submissions/disconnects (reliability metric)
//
// Higher reputation = higher probability of:
// - Winning bounty assignments
// - Being selected for block propagation
// - Priority in peer connection slots

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

// =============================================================================
// Constants
// =============================================================================

/// Minimum stake to participate in reputation system (1000 tokens)
pub const MIN_STAKE_THRESHOLD: u128 = 1_000_000_000; // 1000 * 10^6 (6 decimals)

/// Maximum reputation score (prevents unbounded growth)
pub const MAX_REPUTATION_SCORE: f64 = 1_000_000.0;

/// Decay rate for reputation (η = 1/√2 from whitepaper)
pub const REPUTATION_DECAY_RATE: f64 = 0.7071067811865476;

/// Fault penalty multiplier
pub const FAULT_PENALTY_MULTIPLIER: f64 = 1.0;

/// Blocks considered "aged" for full T_age bonus
pub const FULL_AGE_BLOCKS: u64 = 100_000; // ~10 days at 8.64s/block

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

impl FaultType {
    /// Get severity weight for this fault type
    pub fn severity(&self) -> f64 {
        match self {
            FaultType::InvalidBlock => 5.0,
            FaultType::InvalidSolution => 3.0,
            FaultType::SyncTimeout => 0.5,
            FaultType::UnexpectedDisconnect => 0.2,
            FaultType::Equivocation => 10.0,
            FaultType::Spam => 2.0,
            FaultType::FalsePeerInfo => 3.0,
        }
    }

    /// Get decay rate for this fault (how fast it's forgiven)
    /// Higher = faster forgiveness
    pub fn decay_rate(&self) -> f64 {
        match self {
            FaultType::InvalidBlock => 0.001,         // Very slow forgiveness
            FaultType::InvalidSolution => 0.002,
            FaultType::SyncTimeout => 0.05,           // Fast forgiveness
            FaultType::UnexpectedDisconnect => 0.1,   // Fastest forgiveness
            FaultType::Equivocation => 0.0001,        // Almost permanent
            FaultType::Spam => 0.01,
            FaultType::FalsePeerInfo => 0.005,
        }
    }
}

/// Recorded fault event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaultEvent {
    pub fault_type: FaultType,
    pub block_height: u64,
    pub timestamp: i64,
    pub severity: f64,
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
    
    // === Stake Component (S_token) ===
    /// Current staked amount
    pub staked_amount: u128,
    /// Block height when stake was made
    pub stake_start_block: u64,
    
    // === Age Component (T_age) ===
    /// First seen block height
    pub first_seen_block: u64,
    /// Total blocks as active peer
    pub active_blocks: u64,
    /// Last activity block
    pub last_active_block: u64,
    
    // === Fault Component (E_faults) ===
    /// Fault history
    pub faults: Vec<FaultEvent>,
    /// Cumulative weighted faults
    pub cumulative_fault_score: f64,
    
    // === Positive Actions ===
    /// Successful block propagations
    pub blocks_propagated: u64,
    /// Valid solutions submitted
    pub valid_solutions: u64,
    /// Successful sync responses
    pub sync_responses: u64,
    
    // === Computed Scores ===
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
            cumulative_fault_score: 0.0,
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

    /// Record a fault
    pub fn record_fault(&mut self, fault_type: FaultType, block_height: u64, details: Option<String>) {
        let severity = fault_type.severity();
        
        self.faults.push(FaultEvent {
            fault_type,
            block_height,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0),
            severity,
            details,
        });
        
        // Add to cumulative score
        self.cumulative_fault_score += severity;
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

    /// Calculate current E_faults with decay
    fn calculate_effective_faults(&self, current_block: u64) -> f64 {
        let mut effective_faults = 0.0;
        
        for fault in &self.faults {
            let blocks_since = current_block.saturating_sub(fault.block_height);
            let decay = (-fault.fault_type.decay_rate() * blocks_since as f64).exp();
            effective_faults += fault.severity * decay;
        }
        
        effective_faults
    }

    /// Calculate S_token component (normalized stake)
    fn calculate_stake_score(&self) -> f64 {
        if self.staked_amount < MIN_STAKE_THRESHOLD {
            return 0.0;
        }
        
        // Log scale for stake (prevents whale domination)
        // S = log2(1 + stake/MIN_STAKE)
        let stake_ratio = self.staked_amount as f64 / MIN_STAKE_THRESHOLD as f64;
        (1.0 + stake_ratio).log2()
    }

    /// Calculate T_age component (time commitment)
    fn calculate_age_score(&self, current_block: u64) -> f64 {
        // Use stake age if staked, otherwise use first-seen age
        let age = if self.staked_amount > 0 {
            current_block.saturating_sub(self.stake_start_block)
        } else {
            current_block.saturating_sub(self.first_seen_block)
        };
        
        // Asymptotic growth: T = 1 - e^(-η × age/FULL_AGE)
        let normalized_age = age as f64 / FULL_AGE_BLOCKS as f64;
        1.0 - (-REPUTATION_DECAY_RATE * normalized_age).exp()
    }

    /// Calculate full reputation score: R_n = (S_token × T_age) / (1 + E_faults)
    pub fn calculate_reputation(&mut self, current_block: u64) -> f64 {
        let s_token = self.calculate_stake_score();
        let t_age = self.calculate_age_score(current_block);
        let e_faults = self.calculate_effective_faults(current_block);
        
        // Core formula: R_n = (S_token × T_age) / (1 + E_faults)
        let base_reputation = (s_token * t_age) / (1.0 + e_faults * FAULT_PENALTY_MULTIPLIER);
        
        // Add positive action bonus (up to 20% boost)
        let positive_bonus = self.calculate_positive_bonus();
        let with_bonus = base_reputation * (1.0 + positive_bonus);
        
        // Clamp to max
        self.reputation_score = with_bonus.min(MAX_REPUTATION_SCORE);
        self.last_update_block = current_block;
        
        self.reputation_score
    }

    /// Calculate bonus from positive actions
    fn calculate_positive_bonus(&self) -> f64 {
        // Each positive action type contributes up to ~6.67% bonus (20% total max)
        let propagation_bonus = (self.blocks_propagated as f64 / 1000.0).min(0.0667);
        let solution_bonus = (self.valid_solutions as f64 / 100.0).min(0.0667);
        let sync_bonus = (self.sync_responses as f64 / 500.0).min(0.0667);
        
        propagation_bonus + solution_bonus + sync_bonus
    }

    /// Get detailed reputation breakdown
    pub fn breakdown(&self, current_block: u64) -> ReputationBreakdown {
        ReputationBreakdown {
            peer_id: self.peer_id.clone(),
            stake_score: self.calculate_stake_score(),
            age_score: self.calculate_age_score(current_block),
            effective_faults: self.calculate_effective_faults(current_block),
            positive_bonus: self.calculate_positive_bonus(),
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
    pub stake_score: f64,
    pub age_score: f64,
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
    /// Current block height
    current_block: u64,
    /// Configuration
    config: ReputationConfig,
}

/// Configuration for reputation system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReputationConfig {
    /// Minimum reputation to be considered for bounties
    pub min_bounty_reputation: f64,
    /// Minimum reputation for priority peer slots
    pub min_priority_reputation: f64,
    /// Ban threshold (below this = temporary ban)
    pub ban_threshold: f64,
    /// How often to recalculate percentiles (blocks)
    pub percentile_update_interval: u64,
}

impl Default for ReputationConfig {
    fn default() -> Self {
        ReputationConfig {
            min_bounty_reputation: 1.0,
            min_priority_reputation: 5.0,
            ban_threshold: -10.0,
            percentile_update_interval: 100,
        }
    }
}

impl ReputationManager {
    pub fn new() -> Self {
        Self::with_config(ReputationConfig::default())
    }

    pub fn with_config(config: ReputationConfig) -> Self {
        ReputationManager {
            peers: HashMap::new(),
            current_block: 0,
            config,
        }
    }

    /// Set current block height
    pub fn set_block(&mut self, block: u64) {
        self.current_block = block;
    }

    /// Get or create peer reputation
    pub fn get_or_create(&mut self, peer_id: &str) -> &mut PeerReputation {
        if !self.peers.contains_key(peer_id) {
            self.peers.insert(
                peer_id.to_string(),
                PeerReputation::new(peer_id.to_string(), self.current_block),
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
        let block = self.current_block;
        let peer = self.get_or_create(peer_id);
        peer.record_fault(fault_type, block, details);
        peer.calculate_reputation(block);
        
        println!("⚠️  Fault recorded for {}: {:?} (score: {:.2})", 
            peer_id, fault_type, peer.reputation_score);
    }

    /// Record positive action for a peer
    pub fn record_positive(&mut self, peer_id: &str, action: PositiveAction) {
        let block = self.current_block;
        let peer = self.get_or_create(peer_id);
        peer.record_positive(action);
        peer.calculate_reputation(block);
    }

    /// Update stake for a peer
    pub fn update_stake(&mut self, peer_id: &str, amount: u128) {
        let block = self.current_block;
        let peer = self.get_or_create(peer_id);
        peer.update_stake(amount, block);
        peer.calculate_reputation(block);
    }

    /// Update activity for a peer
    pub fn update_activity(&mut self, peer_id: &str) {
        let block = self.current_block;
        if let Some(peer) = self.peers.get_mut(peer_id) {
            peer.update_activity(block);
        }
    }

    /// Recalculate all reputations and percentiles
    pub fn recalculate_all(&mut self) {
        let block = self.current_block;
        
        // Recalculate all scores
        for peer in self.peers.values_mut() {
            peer.calculate_reputation(block);
        }
        
        // Calculate percentiles
        let mut scores: Vec<f64> = self.peers.values()
            .map(|p| p.reputation_score)
            .collect();
        scores.sort_by(|a, b| a.partial_cmp(b).unwrap());
        
        let n = scores.len();
        if n > 0 {
            for peer in self.peers.values_mut() {
                let rank = scores.iter()
                    .position(|&s| s >= peer.reputation_score)
                    .unwrap_or(n);
                peer.percentile = (rank as f64 / n as f64) * 100.0;
            }
        }
    }

    /// Get peers eligible for bounties (sorted by reputation)
    pub fn bounty_eligible_peers(&self) -> Vec<(&String, &PeerReputation)> {
        let mut eligible: Vec<_> = self.peers.iter()
            .filter(|(_, p)| p.reputation_score >= self.config.min_bounty_reputation)
            .collect();
        eligible.sort_by(|a, b| b.1.reputation_score.partial_cmp(&a.1.reputation_score).unwrap());
        eligible
    }

    /// Get priority peers for connection slots
    pub fn priority_peers(&self) -> Vec<(&String, &PeerReputation)> {
        let mut priority: Vec<_> = self.peers.iter()
            .filter(|(_, p)| p.reputation_score >= self.config.min_priority_reputation)
            .collect();
        priority.sort_by(|a, b| b.1.reputation_score.partial_cmp(&a.1.reputation_score).unwrap());
        priority
    }

    /// Get banned peers (below threshold)
    pub fn banned_peers(&self) -> Vec<&String> {
        self.peers.iter()
            .filter(|(_, p)| p.reputation_score < self.config.ban_threshold)
            .map(|(id, _)| id)
            .collect()
    }

    /// Select peer for bounty assignment using weighted random
    /// Higher reputation = higher probability
    pub fn select_for_bounty(&self, exclude: &[&str]) -> Option<String> {
        use rand::Rng;
        
        let eligible: Vec<_> = self.bounty_eligible_peers()
            .into_iter()
            .filter(|(id, _)| !exclude.contains(&id.as_str()))
            .collect();
        
        if eligible.is_empty() {
            return None;
        }
        
        // Weight by reputation score
        let total_weight: f64 = eligible.iter().map(|(_, p)| p.reputation_score).sum();
        if total_weight <= 0.0 {
            return None;
        }
        
        let mut rng = rand::thread_rng();
        let target = rng.gen_range(0.0..total_weight);
        
        let mut cumulative = 0.0;
        for (id, peer) in eligible {
            cumulative += peer.reputation_score;
            if cumulative >= target {
                return Some(id.clone());
            }
        }
        
        None
    }

    /// Get reputation statistics
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
            bounty_eligible: self.bounty_eligible_peers().len(),
            priority_peers: self.priority_peers().len(),
            banned_peers: self.banned_peers().len(),
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
    pub bounty_eligible: usize,
    pub priority_peers: usize,
    pub banned_peers: usize,
    pub total_faults: usize,
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reputation_formula() {
        let mut peer = PeerReputation::new("test-peer".to_string(), 0);
        
        // Set stake (10x minimum)
        peer.update_stake(MIN_STAKE_THRESHOLD * 10, 0);
        
        // Calculate at block 50000 (halfway to full age)
        let score = peer.calculate_reputation(50000);
        
        assert!(score > 0.0, "Score should be positive: {}", score);
        println!("Reputation with 10x stake at 50% age: {:.4}", score);
    }

    #[test]
    fn test_fault_impact() {
        let mut peer = PeerReputation::new("test-peer".to_string(), 0);
        peer.update_stake(MIN_STAKE_THRESHOLD * 10, 0);
        
        let score_before = peer.calculate_reputation(50000);
        
        // Record a fault
        peer.record_fault(FaultType::InvalidBlock, 50000, None);
        
        let score_after = peer.calculate_reputation(50000);
        
        assert!(score_after < score_before, 
            "Score should decrease after fault: {} -> {}", score_before, score_after);
        println!("Score before fault: {:.4}, after: {:.4}", score_before, score_after);
    }

    #[test]
    fn test_fault_decay() {
        let mut peer = PeerReputation::new("test-peer".to_string(), 0);
        peer.update_stake(MIN_STAKE_THRESHOLD * 10, 0);
        peer.record_fault(FaultType::UnexpectedDisconnect, 0, None);
        
        let score_at_fault = peer.calculate_reputation(0);
        let score_later = peer.calculate_reputation(10000);
        
        assert!(score_later > score_at_fault,
            "Score should improve as fault decays: {} -> {}", score_at_fault, score_later);
    }

    #[test]
    fn test_positive_actions() {
        let mut peer = PeerReputation::new("test-peer".to_string(), 0);
        peer.update_stake(MIN_STAKE_THRESHOLD * 10, 0);
        
        let score_before = peer.calculate_reputation(50000);
        
        // Record positive actions
        for _ in 0..100 {
            peer.record_positive(PositiveAction::BlockPropagated);
        }
        
        let score_after = peer.calculate_reputation(50000);
        
        assert!(score_after > score_before,
            "Score should increase with positive actions: {} -> {}", score_before, score_after);
    }

    #[test]
    fn test_manager_selection() {
        let mut manager = ReputationManager::new();
        
        // Stake at block 0
        manager.set_block(0);
        manager.update_stake("peer-a", MIN_STAKE_THRESHOLD * 100);
        manager.update_stake("peer-b", MIN_STAKE_THRESHOLD * 10);
        manager.update_stake("peer-c", MIN_STAKE_THRESHOLD * 1);
        
        // Check at block 50000 (peers have aged)
        manager.set_block(50000);
        manager.recalculate_all();
        
        // Higher stake should have higher score
        let a = manager.get("peer-a").unwrap();
        let b = manager.get("peer-b").unwrap();
        let c = manager.get("peer-c").unwrap();
        
        println!("Scores: a={}, b={}, c={}", a.reputation_score, b.reputation_score, c.reputation_score);
        
        assert!(a.reputation_score > b.reputation_score,
            "a ({}) should be > b ({})", a.reputation_score, b.reputation_score);
        assert!(b.reputation_score > c.reputation_score,
            "b ({}) should be > c ({})", b.reputation_score, c.reputation_score);
    }

    #[test]
    fn test_bounty_selection() {
        let mut manager = ReputationManager::new();
        
        // Stake at block 0
        manager.set_block(0);
        manager.update_stake("peer-a", MIN_STAKE_THRESHOLD * 100);
        manager.update_stake("peer-b", MIN_STAKE_THRESHOLD * 10);
        
        // Check at block 50000 (peers have aged)
        manager.set_block(50000);
        manager.recalculate_all();
        
        // Should be able to select for bounty
        let selected = manager.select_for_bounty(&[]);
        assert!(selected.is_some(), "Should have bounty-eligible peers");
        
        // Should respect exclusions
        let selected2 = manager.select_for_bounty(&["peer-a", "peer-b"]);
        assert!(selected2.is_none());
    }

    #[test]
    fn test_fault_severity() {
        assert!(FaultType::Equivocation.severity() > FaultType::InvalidBlock.severity());
        assert!(FaultType::InvalidBlock.severity() > FaultType::UnexpectedDisconnect.severity());
    }
}

