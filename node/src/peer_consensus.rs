// COINjecture Peer Consensus Module
// Inspired by XRPL's RPCA and TigerBeetle's deterministic design
//
// This module implements:
// 1. Multi-peer height tracking (not just best single peer)
// 2. Negative UNL equivalent (filter unreliable peers)
// 3. Work score comparison (not just height)
// 4. Deterministic sync decisions
//
// NOTE: Full peer consensus integration is prepared for future use
#![allow(dead_code)]

use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

// Import unified timeout constants from network layer
// These are ETA-derived to maintain dimensional consistency
use coinject_network::cpp::timeouts::{
    consensus_peer_timeout, consensus_stale_threshold, MAX_MISSED_ROUNDS_CONSENSUS,
};

const INCOMPATIBLE_TIP_QUARANTINE_AFTER: u32 = 3;
const INCOMPATIBLE_TIP_QUARANTINE_SECS: u64 = 120;

/// Peer reliability tracking for Negative UNL equivalent
#[derive(Debug, Clone)]
pub struct PeerState {
    /// Last known block height from this peer
    pub best_height: u64,
    /// Last known block hash from this peer
    pub best_hash: [u8; 32],
    /// Cumulative work on peer's advertised best chain (`0` = unknown / legacy status).
    pub cumulative_work: u128,
    /// Last time we received a status update
    pub last_seen: Instant,
    /// Number of consensus rounds this peer missed
    pub missed_rounds: u32,
    /// Whether this peer is filtered (Negative UNL)
    pub is_filtered: bool,
    /// Consecutive incompatible-tip reports for this peer.
    pub incompatible_tip_reports: u32,
    /// Temporary quarantine window for likely fork sources.
    pub quarantine_until: Option<Instant>,
}

impl PeerState {
    pub fn new(height: u64, hash: [u8; 32]) -> Self {
        Self {
            best_height: height,
            best_hash: hash,
            cumulative_work: 0,
            last_seen: Instant::now(),
            missed_rounds: 0,
            is_filtered: false,
            incompatible_tip_reports: 0,
            quarantine_until: None,
        }
    }

    /// Check if peer is stale (hasn't reported recently)
    /// Uses unified timeout from network layer: NETWORK × (1 + η) ≈ 153s
    /// This ETA-derived threshold provides a grace period beyond the network timeout
    /// while maintaining dimensional consistency across the system
    pub fn is_stale(&self) -> bool {
        self.last_seen.elapsed() > consensus_stale_threshold()
    }

    pub fn is_quarantined(&self) -> bool {
        self.quarantine_until
            .map(|until| Instant::now() < until)
            .unwrap_or(false)
    }
}

/// Largest subset of sorted heights where `max - min ≤ 1` (single-block propagation skew).
/// Returns `(cluster_size, representative_height)` using the median index of the best window.
fn largest_height_cluster_within_one_block(heights: &[u64]) -> (usize, u64) {
    if heights.is_empty() {
        return (0, 0);
    }
    let mut best_len = 0usize;
    let mut best_height = 0u64;
    let mut i = 0usize;
    for j in 0..heights.len() {
        while heights[j].saturating_sub(heights[i]) > 1 {
            i += 1;
        }
        let len = j - i + 1;
        if len > best_len {
            best_len = len;
            let mid = (i + j) / 2;
            best_height = heights[mid];
        }
    }
    (best_len, best_height)
}

/// Configuration for consensus decisions
#[derive(Debug, Clone)]
pub struct ConsensusConfig {
    /// Minimum peers required before mining is allowed
    pub min_peers_for_mining: usize,
    /// How many blocks behind peers we can be and still mine
    pub sync_threshold_blocks: u64,
    /// If our tip is more than this many blocks **above** the peer median, refuse to mine.
    /// Prevents extending a local fork when the mesh agrees on a lower canonical tip.
    pub max_ahead_of_peers_blocks: u64,
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
            // Minimum 1 peer to mine (allows 2-node bootstrap)
            min_peers_for_mining: 1,
            sync_threshold_blocks: 10, // Within 10 blocks
            // Allow normal propagation skew (we may be one or a few blocks ahead briefly).
            // Anything larger vs peer median is almost certainly a fork / partition.
            max_ahead_of_peers_blocks: 96,
            // ADAPTIVE CONSENSUS: Threshold adjusts based on peer count
            // BOOTSTRAP MODE (peers < 4): 50% - prioritizes liveness
            // SECURE MODE (peers >= 4): 80% - prioritizes safety (BFT)
            consensus_threshold: 0.80, // Target for production
            // peer_stale_timeout: Unified ETA-derived timeout from network layer
            // NETWORK / η ≈ 127s - consensus has more patience than network layer (90s)
            peer_stale_timeout: consensus_peer_timeout(),
            // max_missed_rounds: Unified from network layer (scaled by √2)
            max_missed_rounds: MAX_MISSED_ROUNDS_CONSENSUS,
        }
    }
}

impl ConsensusConfig {
    /// Get adaptive consensus threshold based on peer count
    /// Small networks need lower thresholds to maintain liveness
    /// Large networks can enforce strict BFT thresholds for security
    pub fn get_adaptive_threshold(&self, peer_count: usize) -> (f64, &'static str) {
        match peer_count {
            0 => (1.0, "SOLO"),         // No peers, impossible to reach consensus
            1 => (0.50, "BOOTSTRAP-1"), // 1 peer: need 50% (1/1 = 100% works)
            2 => (0.50, "BOOTSTRAP-2"), // 2 peers: 50% allows 1/2 to pass
            3 => (0.60, "BOOTSTRAP-3"), // 3 peers: 60% allows 2/3 to pass
            _ => (self.consensus_threshold, "SECURE"), // 4+ peers: full BFT threshold
        }
    }
}

impl ConsensusConfig {
    /// Production config with 5-peer minimum
    pub fn production() -> Self {
        Self {
            min_peers_for_mining: 5, // True 80% consensus requires 5 peers
            ..Default::default()
        }
    }

    /// Testnet bootstrap config with 2-peer minimum  
    pub fn testnet() -> Self {
        Self {
            min_peers_for_mining: 2, // Bootstrap with 2 peers
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

    /// Get sync threshold from config
    pub fn sync_threshold_blocks(&self) -> u64 {
        self.config.sync_threshold_blocks
    }

    pub fn max_ahead_of_peers_blocks(&self) -> u64 {
        self.config.max_ahead_of_peers_blocks
    }

    /// Update a peer's state when we receive a StatusUpdate (or handshake with `cumulative_work = 0`).
    pub async fn update_peer(
        &self,
        peer_id: String,
        height: u64,
        hash: [u8; 32],
        cumulative_work: u128,
    ) {
        let mut peers = self.peers.write().await;

        if let Some(peer) = peers.get_mut(&peer_id) {
            let old_height = peer.best_height;
            peer.best_height = height;
            peer.best_hash = hash;
            peer.cumulative_work = cumulative_work;
            peer.last_seen = Instant::now();
            peer.missed_rounds = 0; // Reset missed rounds on update

            // Un-filter peer if they're back online
            if peer.is_filtered {
                println!(
                    "🔄 Peer {} back online, removing from Negative UNL",
                    peer_id
                );
                peer.is_filtered = false;
            }

            // Log height changes
            if height != old_height {
                println!(
                    "📡 Peer {} height updated: {} -> {}",
                    &peer_id[..8.min(peer_id.len())],
                    old_height,
                    height
                );
            }
        } else {
            let mut state = PeerState::new(height, hash);
            state.cumulative_work = cumulative_work;
            peers.insert(peer_id.clone(), state);
            println!(
                "📡 New peer tracked: {} at height {}",
                &peer_id[..8.min(peer_id.len())],
                height
            );
        }
    }

    /// Mark a peer as disconnected.
    ///
    /// For fork recovery and block sync we must only target live CPP sessions.
    /// Keeping disconnected peers in the active set lets reorg logic pick a
    /// peer that is no longer present in the network's `peers` map, which then
    /// fails later with `Peer not found`.
    pub async fn mark_peer_disconnected(&self, peer_id: &str) {
        let mut peers = self.peers.write().await;
        if peers.remove(peer_id).is_some() {
            println!(
                "📡 Peer {} disconnected and removed from active consensus tracking",
                peer_id
            );
        }
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
        peers
            .iter()
            .filter(|(_, state)| !state.is_filtered && !state.is_stale() && !state.is_quarantined())
            .map(|(id, state)| (id.clone(), state.clone()))
            .collect()
    }

    /// Record an incompatible-tip event and quarantine repeat offenders temporarily.
    pub async fn note_incompatible_tip(&self, peer_id: &str) {
        let mut peers = self.peers.write().await;
        let Some(state) = peers.get_mut(peer_id) else {
            return;
        };
        state.incompatible_tip_reports = state.incompatible_tip_reports.saturating_add(1);
        if state.incompatible_tip_reports >= INCOMPATIBLE_TIP_QUARANTINE_AFTER {
            state.quarantine_until =
                Some(Instant::now() + Duration::from_secs(INCOMPATIBLE_TIP_QUARANTINE_SECS));
            state.incompatible_tip_reports = 0;
            println!(
                "⚠️  Quarantining peer {} after incompatible-tip reports",
                peer_id
            );
        }
    }

    /// Clear incompatibility counters after successful canonical sync from this peer.
    pub async fn clear_incompatible_tip_penalty(&self, peer_id: &str) {
        let mut peers = self.peers.write().await;
        if let Some(state) = peers.get_mut(peer_id) {
            state.incompatible_tip_reports = 0;
        }
    }

    /// Get the number of active peers (for Negative UNL-adjusted quorum)
    pub async fn active_peer_count(&self) -> usize {
        self.active_peers().await.len()
    }

    /// Get the best known height from any peer
    pub async fn best_peer_height(&self) -> u64 {
        let peers = self.peers.read().await;
        peers
            .values()
            .filter(|p| !p.is_filtered && !p.is_stale())
            .map(|p| p.best_height)
            .max()
            .unwrap_or(0)
    }

    /// Get the height of a specific peer (by peer_id string)
    pub async fn get_peer_height(&self, peer_id: &str) -> Option<u64> {
        let peers = self.peers.read().await;
        peers.get(peer_id).map(|p| p.best_height)
    }

    /// Advertised tip for a peer (height, hash, cumulative work).
    pub async fn get_peer_tip(&self, peer_id: &str) -> Option<(u64, [u8; 32], u128)> {
        let peers = self.peers.read().await;
        peers
            .get(peer_id)
            .map(|p| (p.best_height, p.best_hash, p.cumulative_work))
    }

    /// Active peer with the greatest advertised cumulative work (hash-anchored sync source).
    pub async fn best_active_peer_by_cumulative_work(&self) -> Option<(String, PeerState)> {
        self.active_peers()
            .await
            .into_iter()
            .max_by(|a, b| a.1.cumulative_work.cmp(&b.1.cumulative_work))
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
        if heights.len().is_multiple_of(2) {
            (heights[mid - 1] + heights[mid]) / 2
        } else {
            heights[mid]
        }
    }

    /// Check if we have consensus among peers about the chain tip
    /// Returns (has_consensus, consensus_height, agreement_percentage)
    /// Uses ADAPTIVE threshold based on peer count
    pub async fn check_consensus(&self) -> (bool, u64, f64) {
        let active = self.active_peers().await;
        let peer_count = active.len();

        if peer_count < self.config.min_peers_for_mining {
            return (false, 0, 0.0);
        }

        // Get adaptive threshold based on network size
        let (threshold, _mode) = self.config.get_adaptive_threshold(peer_count);

        // Count agreement using a **≤1 block** spread (propagation skew: 100 vs 101 is normal).
        // Previously we bucketed exact heights only, which split votes and made `should_mine`
        // fail on most nodes while one peer cluster still passed — only that node appeared to mine.
        let mut heights: Vec<u64> = active.iter().map(|(_, s)| s.best_height).collect();
        heights.sort_unstable();
        let (max_votes, consensus_height) = largest_height_cluster_within_one_block(&heights);

        let agreement = max_votes as f64 / peer_count as f64;
        let has_consensus = agreement >= threshold; // Use adaptive threshold!

        (has_consensus, consensus_height, agreement)
    }

    /// The main sync decision: Should we mine?
    /// Returns (should_mine, reason)
    pub async fn should_mine(&self, our_height: u64) -> (bool, String) {
        let active_count = self.active_peer_count().await;

        // DIAGNOSTIC: Log all peers and their states
        {
            let peers = self.peers.read().await;
            let total = peers.len();
            let stale_count = peers.values().filter(|p| p.is_stale()).count();
            let filtered_count = peers.values().filter(|p| p.is_filtered).count();
            println!(
                "📊 [PeerConsensus] Total peers: {}, Active: {}, Stale: {}, Filtered: {}",
                total, active_count, stale_count, filtered_count
            );
            for (id, state) in peers.iter() {
                let status = if state.is_stale() {
                    "STALE"
                } else if state.is_filtered {
                    "FILTERED"
                } else {
                    "ACTIVE"
                };
                println!(
                    "   - {} height={} last_seen={}s ago [{}]",
                    &id[..8.min(id.len())],
                    state.best_height,
                    state.last_seen.elapsed().as_secs(),
                    status
                );
            }
        }

        // Check 1: Do we have enough peers?
        if active_count < self.config.min_peers_for_mining {
            return (
                false,
                format!(
                    "Insufficient peers: {} < {} required",
                    active_count, self.config.min_peers_for_mining
                ),
            );
        }

        // Check 2: Are we synced with peer median?
        let median_height = self.median_peer_height().await;
        if our_height + self.config.sync_threshold_blocks < median_height {
            let blocks_behind = median_height - our_height;
            return (
                false,
                format!(
                    "Behind peers: {} blocks (our: {}, median: {})",
                    blocks_behind, our_height, median_height
                ),
            );
        }

        // Check 2b: Not far *ahead* of peer-reported tips (local fork / eclipse)
        if median_height > 0
            && our_height > median_height.saturating_add(self.config.max_ahead_of_peers_blocks)
        {
            let ahead = our_height - median_height;
            return (
                false,
                format!(
                    "Ahead of peers: {} blocks above median (our: {}, median: {}, max allowed: {}) — pause mining until reorg/sync",
                    ahead,
                    our_height,
                    median_height,
                    self.config.max_ahead_of_peers_blocks
                ),
            );
        }

        // Check 3: Do peers have consensus? (ADAPTIVE THRESHOLD)
        let (threshold, mode) = self.config.get_adaptive_threshold(active_count);
        let (has_consensus, consensus_height, agreement) = self.check_consensus().await;

        if !has_consensus {
            return (
                false,
                format!(
                    "[{}] No peer consensus: {:.1}% < {:.1}% required at height {}",
                    mode,
                    agreement * 100.0,
                    threshold * 100.0,
                    consensus_height
                ),
            );
        }

        // All checks passed!
        (
            true,
            format!(
                "[{}] Consensus OK: {} peers, {:.1}% agreement (threshold: {:.0}%)",
                mode,
                active_count,
                agreement * 100.0,
                threshold * 100.0
            ),
        )
    }

    /// Periodic maintenance: filter stale peers (Negative UNL)
    pub async fn maintenance_tick(&self) {
        let mut peers = self.peers.write().await;

        for (peer_id, state) in peers.iter_mut() {
            if state.is_stale() && !state.is_filtered {
                state.missed_rounds += 1;

                if state.missed_rounds >= self.config.max_missed_rounds {
                    println!(
                        "⚠️  Filtering peer {} (Negative UNL) - {} missed rounds",
                        peer_id, state.missed_rounds
                    );
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
    pub fn block_work_score(problem_size: u64, solve_time_us: u64, verify_time_us: u64) -> u64 {
        // Asymmetry ratio: how much harder to solve than verify
        let asymmetry = solve_time_us
            .checked_div(verify_time_us)
            .unwrap_or(solve_time_us);

        // Space complexity (problem size)
        let space = problem_size;

        // Quality: smaller solve time per unit of problem size = better
        let quality = if solve_time_us == 0 {
            problem_size
        } else {
            (problem_size * 1_000_000)
                .checked_div(solve_time_us)
                .unwrap_or(problem_size)
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

    #[test]
    fn largest_cluster_within_one_block() {
        assert_eq!(largest_height_cluster_within_one_block(&[]), (0, 0));
        assert_eq!(largest_height_cluster_within_one_block(&[100]), (1, 100));
        assert_eq!(
            largest_height_cluster_within_one_block(&[100, 100, 101]),
            (3, 100)
        );
        assert_eq!(
            largest_height_cluster_within_one_block(&[100, 102, 104]),
            (1, 100)
        );
    }

    #[tokio::test]
    async fn test_check_consensus_one_block_skew() {
        let consensus = PeerConsensus::with_defaults();
        consensus
            .update_peer("a".to_string(), 100, [0; 32], 0)
            .await;
        consensus
            .update_peer("b".to_string(), 101, [0; 32], 0)
            .await;
        consensus
            .update_peer("c".to_string(), 100, [0; 32], 0)
            .await;

        let (has, height, agreement) = consensus.check_consensus().await;
        assert!(has, "100/101 skew should not split the quorum");
        assert_eq!(height, 100);
        assert!((agreement - 1.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_peer_consensus_basic() {
        let consensus = PeerConsensus::with_defaults();

        // Add some peers
        consensus
            .update_peer("peer1".to_string(), 100, [0; 32], 0)
            .await;
        consensus
            .update_peer("peer2".to_string(), 100, [0; 32], 0)
            .await;
        consensus
            .update_peer("peer3".to_string(), 100, [0; 32], 0)
            .await;

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
        consensus
            .update_peer("peer1".to_string(), 100, [0; 32], 0)
            .await;
        consensus
            .update_peer("peer2".to_string(), 100, [0; 32], 0)
            .await;
        consensus
            .update_peer("peer3".to_string(), 100, [0; 32], 0)
            .await;

        // We're at 100, peers at 100 - should mine
        let (should, _) = consensus.should_mine(100).await;
        assert!(should);

        // We're at 50, peers at 100 - should NOT mine (50 blocks behind)
        let (should, reason) = consensus.should_mine(50).await;
        assert!(!should);
        assert!(reason.contains("Behind peers"));

        // Peers at 100, we're far ahead on a local fork — should NOT mine
        let (should, reason) = consensus.should_mine(250).await;
        assert!(!should);
        assert!(reason.contains("Ahead of peers"));
    }

    #[tokio::test]
    async fn test_quarantine_after_repeated_incompatible_tips() {
        let consensus = PeerConsensus::with_defaults();
        consensus
            .update_peer("peer1".to_string(), 100, [0; 32], 1_000)
            .await;
        assert_eq!(consensus.active_peer_count().await, 1);

        consensus.note_incompatible_tip("peer1").await;
        consensus.note_incompatible_tip("peer1").await;
        assert_eq!(consensus.active_peer_count().await, 1);
        consensus.note_incompatible_tip("peer1").await;
        assert_eq!(consensus.active_peer_count().await, 0);
    }

    #[test]
    fn test_work_score_calculation() {
        // Higher asymmetry = higher work
        let score1 = WorkScoreCalculator::block_work_score(100, 1_000_000, 1_000);
        let score2 = WorkScoreCalculator::block_work_score(100, 1_000_000, 10_000);
        assert!(score1 > score2); // More asymmetric = better
    }
}
