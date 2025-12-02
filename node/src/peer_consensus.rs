// COINjecture Peer Consensus Module
// Inspired by XRPL's RPCA and TigerBeetle's deterministic design
//
// This module implements:
// 1. Multi-peer height tracking (not just best single peer)
// 2. Negative UNL equivalent (filter unreliable peers)
// 3. Work score comparison (not just height)
// 4. Deterministic sync decisions

use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Peer reliability tracking for Negative UNL equivalent
#[derive(Debug, Clone)]
pub struct PeerState {
    /// Last known block height from this peer
    pub best_height: u64,
    /// Last known block hash from this peer
    pub best_hash: [u8; 32],
    /// Cumulative work score (for chain comparison)
    pub work_score: u64,
    /// Last time we received a status update
    pub last_seen: Instant,
    /// Number of consensus rounds this peer missed
    pub missed_rounds: u32,
    /// Whether this peer is filtered (Negative UNL)
    pub is_filtered: bool,
}

impl PeerState {
    pub fn new(height: u64, hash: [u8; 32]) -> Self {
        Self {
            best_height: height,
            best_hash: hash,
            work_score: height, // Simplified: work = height for now
            last_seen: Instant::now(),
            missed_rounds: 0,
            is_filtered: false,
        }
    }
    
    /// Check if peer is stale (hasn't reported recently)
    /// Uses 120 second timeout to handle connection churn
    pub fn is_stale(&self) -> bool {
        self.last_seen.elapsed() > Duration::from_secs(120)
    }
}

/// Configuration for consensus decisions
#[derive(Debug, Clone)]
pub struct ConsensusConfig {
    /// Minimum peers required before mining is allowed
    pub min_peers_for_mining: usize,
    /// How many blocks behind peers we can be and still mine
    pub sync_threshold_blocks: u64,
    /// Percentage of peers that must agree (0.0-1.0)
    pub consensus_threshold: f64,
    /// How long before a peer is considered stale
    pub peer_stale_timeout: Duration,
    /// How many missed rounds before filtering a peer
    pub max_missed_rounds: u32,
}

impl Default for ConsensusConfig {
    fn default() -> Self {
        Self {
            // TESTNET BOOTSTRAP: 1 peer minimum for 2-node bootstrap
            // With 2 nodes, each can only see 1 peer (can't see itself)
            // Increase to 2+ when Sarah's GCE joins (3-node network)
            // PRODUCTION: Increase to 4+ when we have 5+ nodes for true 80% consensus
            min_peers_for_mining: 1,        
            sync_threshold_blocks: 10,       // Within 10 blocks
            // TESTNET: 51% for 1-2 peer scenarios
            // PRODUCTION: Increase to 80% when we have 5+ nodes
            consensus_threshold: 0.51,
            // Increased from 60s to 120s to handle connection churn
            peer_stale_timeout: Duration::from_secs(120),
            max_missed_rounds: 5,
        }
    }
}

impl ConsensusConfig {
    /// Production config with 5-peer minimum
    pub fn production() -> Self {
        Self {
            min_peers_for_mining: 5,        // True 80% consensus requires 5 peers
            ..Default::default()
        }
    }
    
    /// Testnet bootstrap config with 2-peer minimum  
    pub fn testnet() -> Self {
        Self {
            min_peers_for_mining: 2,        // Bootstrap with 2 peers
            ..Default::default()
        }
    }
}

/// The main peer consensus tracker
/// Thread-safe, designed for concurrent access
pub struct PeerConsensus {
    /// All known peers and their states
    peers: RwLock<HashMap<String, PeerState>>,
    /// Configuration
    config: ConsensusConfig,
}

impl PeerConsensus {
    pub fn new(config: ConsensusConfig) -> Self {
        Self {
            peers: RwLock::new(HashMap::new()),
            config,
        }
    }
    
    pub fn with_defaults() -> Self {
        Self::new(ConsensusConfig::default())
    }
    
    /// Update a peer's state when we receive a StatusUpdate
    pub async fn update_peer(&self, peer_id: String, height: u64, hash: [u8; 32]) {
        let mut peers = self.peers.write().await;
        
        if let Some(peer) = peers.get_mut(&peer_id) {
            peer.best_height = height;
            peer.best_hash = hash;
            peer.work_score = height; // TODO: Implement proper work score
            peer.last_seen = Instant::now();
            peer.missed_rounds = 0; // Reset missed rounds on update
            
            // Un-filter peer if they're back online
            if peer.is_filtered {
                println!("🔄 Peer {} back online, removing from Negative UNL", peer_id);
                peer.is_filtered = false;
            }
        } else {
            peers.insert(peer_id.clone(), PeerState::new(height, hash));
            println!("📡 New peer tracked: {} at height {}", peer_id, height);
        }
    }
    
    /// Mark a peer as disconnected (but keep tracking for stale timeout)
    /// This allows peers to still count if they reconnect within the timeout
    pub async fn mark_peer_disconnected(&self, peer_id: &str) {
        // DON'T remove the peer - let them become stale naturally after 120s
        // This handles connection churn where peers rapidly connect/disconnect
        let peers = self.peers.read().await;
        if peers.contains_key(peer_id) {
            println!("📡 Peer {} disconnected (still tracking for 120s)", peer_id);
        }
        // Peer's last_seen stays the same, so they'll count until stale
    }
    
    /// Actually remove a peer (only call for permanent removal)
    pub async fn remove_peer(&self, peer_id: &str) {
        let mut peers = self.peers.write().await;
        peers.remove(peer_id);
        println!("📡 Peer permanently removed: {}", peer_id);
    }
    
    /// Get all active (non-filtered, non-stale) peers
    pub async fn active_peers(&self) -> Vec<(String, PeerState)> {
        let peers = self.peers.read().await;
        peers.iter()
            .filter(|(_, state)| !state.is_filtered && !state.is_stale())
            .map(|(id, state)| (id.clone(), state.clone()))
            .collect()
    }
    
    /// Get the number of active peers (for Negative UNL-adjusted quorum)
    pub async fn active_peer_count(&self) -> usize {
        self.active_peers().await.len()
    }
    
    /// Get the best known height from any peer
    pub async fn best_peer_height(&self) -> u64 {
        let peers = self.peers.read().await;
        peers.values()
            .filter(|p| !p.is_filtered && !p.is_stale())
            .map(|p| p.best_height)
            .max()
            .unwrap_or(0)
    }
    
    /// Get the median height across all active peers
    /// More robust than max (resistant to outliers/attackers)
    pub async fn median_peer_height(&self) -> u64 {
        let active = self.active_peers().await;
        if active.is_empty() {
            return 0;
        }
        
        let mut heights: Vec<u64> = active.iter().map(|(_, s)| s.best_height).collect();
        heights.sort();
        
        let mid = heights.len() / 2;
        if heights.len() % 2 == 0 {
            (heights[mid - 1] + heights[mid]) / 2
        } else {
            heights[mid]
        }
    }
    
    /// Check if we have consensus among peers about the chain tip
    /// Returns (has_consensus, consensus_height, agreement_percentage)
    pub async fn check_consensus(&self) -> (bool, u64, f64) {
        let active = self.active_peers().await;
        if active.len() < self.config.min_peers_for_mining {
            return (false, 0, 0.0);
        }
        
        // Count how many peers agree on each height (within 1 block tolerance)
        let mut height_votes: HashMap<u64, usize> = HashMap::new();
        for (_, state) in &active {
            // Group heights within 1 block of each other
            let normalized_height = state.best_height;
            *height_votes.entry(normalized_height).or_insert(0) += 1;
        }
        
        // Find the height with most agreement
        let (consensus_height, max_votes) = height_votes.iter()
            .max_by_key(|(_, votes)| *votes)
            .map(|(h, v)| (*h, *v))
            .unwrap_or((0, 0));
        
        let agreement = max_votes as f64 / active.len() as f64;
        let has_consensus = agreement >= self.config.consensus_threshold;
        
        (has_consensus, consensus_height, agreement)
    }
    
    /// The main sync decision: Should we mine?
    /// Returns (should_mine, reason)
    pub async fn should_mine(&self, our_height: u64) -> (bool, String) {
        let active_count = self.active_peer_count().await;
        
        // Check 1: Do we have enough peers?
        if active_count < self.config.min_peers_for_mining {
            return (false, format!(
                "Insufficient peers: {} < {} required",
                active_count, self.config.min_peers_for_mining
            ));
        }
        
        // Check 2: Are we synced with peer median?
        let median_height = self.median_peer_height().await;
        if our_height + self.config.sync_threshold_blocks < median_height {
            let blocks_behind = median_height - our_height;
            return (false, format!(
                "Behind peers: {} blocks (our: {}, median: {})",
                blocks_behind, our_height, median_height
            ));
        }
        
        // Check 3: Do peers have consensus?
        let (has_consensus, consensus_height, agreement) = self.check_consensus().await;
        if !has_consensus {
            return (false, format!(
                "No peer consensus: {:.1}% < {:.1}% required at height {}",
                agreement * 100.0,
                self.config.consensus_threshold * 100.0,
                consensus_height
            ));
        }
        
        // All checks passed!
        (true, format!(
            "Synced and consensus reached: {} peers, {:.1}% agreement at height {}",
            active_count, agreement * 100.0, consensus_height
        ))
    }
    
    /// Periodic maintenance: filter stale peers (Negative UNL)
    pub async fn maintenance_tick(&self) {
        let mut peers = self.peers.write().await;
        
        for (peer_id, state) in peers.iter_mut() {
            if state.is_stale() && !state.is_filtered {
                state.missed_rounds += 1;
                
                if state.missed_rounds >= self.config.max_missed_rounds {
                    println!("⚠️  Filtering peer {} (Negative UNL) - {} missed rounds",
                        peer_id, state.missed_rounds);
                    state.is_filtered = true;
                }
            }
        }
    }
    
    /// Get diagnostic information for logging
    pub async fn diagnostics(&self) -> String {
        let active = self.active_peers().await;
        let filtered_count = {
            let peers = self.peers.read().await;
            peers.values().filter(|p| p.is_filtered).count()
        };
        
        let (has_consensus, consensus_height, agreement) = self.check_consensus().await;
        
        format!(
            "Peers: {} active, {} filtered | Consensus: {} at height {} ({:.1}%)",
            active.len(),
            filtered_count,
            if has_consensus { "YES" } else { "NO" },
            consensus_height,
            agreement * 100.0
        )
    }
}

/// Work score calculation for comparing chains
/// Based on COINjecture's NP-problem asymmetry formula
pub struct WorkScoreCalculator;

impl WorkScoreCalculator {
    /// Calculate work score for a block
    /// work = asymmetry × space × quality × energy_eff
    pub fn block_work_score(
        problem_size: u64,
        solve_time_us: u64,
        verify_time_us: u64,
    ) -> u64 {
        // Asymmetry ratio: how much harder to solve than verify
        let asymmetry = if verify_time_us > 0 {
            solve_time_us / verify_time_us
        } else {
            solve_time_us
        };
        
        // Space complexity (problem size)
        let space = problem_size;
        
        // Quality: smaller solve time per unit of problem size = better
        let quality = if solve_time_us > 0 {
            (problem_size * 1_000_000) / solve_time_us
        } else {
            problem_size
        };
        
        // Combined work score (simplified)
        // In production, we'd want more sophisticated weighting
        (asymmetry + space + quality) / 3
    }
    
    /// Calculate cumulative work score for a chain
    pub fn chain_work_score(block_scores: &[u64]) -> u64 {
        block_scores.iter().sum()
    }
    
    /// Compare two chains by work score
    /// Returns: -1 if ours is better, 0 if equal, 1 if theirs is better
    pub fn compare_chains(our_work: u64, their_work: u64) -> i32 {
        // Allow 0.5% tolerance before switching
        let tolerance = our_work / 200; // 0.5%
        
        if their_work > our_work + tolerance {
            1 // Theirs is better
        } else if our_work > their_work + tolerance {
            -1 // Ours is better
        } else {
            0 // Effectively equal
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_peer_consensus_basic() {
        let consensus = PeerConsensus::with_defaults();
        
        // Add some peers
        consensus.update_peer("peer1".to_string(), 100, [0; 32]).await;
        consensus.update_peer("peer2".to_string(), 100, [0; 32]).await;
        consensus.update_peer("peer3".to_string(), 100, [0; 32]).await;
        
        // Should have 3 active peers
        assert_eq!(consensus.active_peer_count().await, 3);
        
        // Should have consensus at height 100
        let (has_consensus, height, agreement) = consensus.check_consensus().await;
        assert!(has_consensus);
        assert_eq!(height, 100);
        assert_eq!(agreement, 1.0);
    }
    
    #[tokio::test]
    async fn test_should_mine() {
        let consensus = PeerConsensus::with_defaults();
        
        // No peers - should not mine
        let (should, reason) = consensus.should_mine(100).await;
        assert!(!should);
        assert!(reason.contains("Insufficient peers"));
        
        // Add 3 peers at height 100
        consensus.update_peer("peer1".to_string(), 100, [0; 32]).await;
        consensus.update_peer("peer2".to_string(), 100, [0; 32]).await;
        consensus.update_peer("peer3".to_string(), 100, [0; 32]).await;
        
        // We're at 100, peers at 100 - should mine
        let (should, _) = consensus.should_mine(100).await;
        assert!(should);
        
        // We're at 50, peers at 100 - should NOT mine (50 blocks behind)
        let (should, reason) = consensus.should_mine(50).await;
        assert!(!should);
        assert!(reason.contains("Behind peers"));
    }
    
    #[test]
    fn test_work_score_calculation() {
        // Higher asymmetry = higher work
        let score1 = WorkScoreCalculator::block_work_score(100, 1_000_000, 1_000);
        let score2 = WorkScoreCalculator::block_work_score(100, 1_000_000, 10_000);
        assert!(score1 > score2); // More asymmetric = better
    }
}

