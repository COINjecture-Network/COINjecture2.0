// =============================================================================
// COINjecture P2P Protocol (CPP) - Equilibrium-Based Routing with Murmuration
// =============================================================================
// Message routing using the equilibrium constant η = λ = 1/√2 ≈ 0.7071
// Enhanced with GoldenSeed-inspired murmuration for swarm coordination

use crate::cpp::config::ETA;
use crate::cpp::flock::{FlockStateCompact, PHI_INV};
use std::collections::HashMap;

/// Peer ID (32-byte hash)
pub type PeerId = [u8; 32];

/// Peer information for routing decisions with murmuration support
#[derive(Debug, Clone)]
pub struct PeerInfo {
    /// Peer ID
    pub id: PeerId,

    /// Best block height
    pub best_height: u64,

    /// Node type
    pub node_type: u8,

    /// Connection quality (0.0 = poor, 1.0 = excellent)
    pub quality: f64,

    /// Last seen timestamp
    pub last_seen: u64,

    /// Flock phase for murmuration coordination (0..7)
    pub flock_phase: u8,

    /// Flock epoch this peer is in
    pub flock_epoch: u64,

    /// Height velocity (blocks per update period)
    pub velocity: f64,
}

/// Equilibrium-based message router
/// 
/// Uses the equilibrium constant to determine:
/// - Broadcast fanout (√n × η peers)
/// - Sync peer selection (dimensional distance)
/// - Load balancing (dimensional priority)
pub struct EquilibriumRouter {
    /// All connected peers
    peers: HashMap<PeerId, PeerInfo>,
    
    /// Equilibrium constant
    eta: f64,
}

impl EquilibriumRouter {
    /// Create new router
    pub fn new() -> Self {
        EquilibriumRouter {
            peers: HashMap::new(),
            eta: ETA,
        }
    }
    
    /// Add or update peer
    pub fn add_peer(&mut self, peer: PeerInfo) {
        self.peers.insert(peer.id, peer);
    }
    
    /// Remove peer
    pub fn remove_peer(&mut self, peer_id: &PeerId) {
        self.peers.remove(peer_id);
    }
    
    /// Get peer by ID
    pub fn get_peer(&self, peer_id: &PeerId) -> Option<&PeerInfo> {
        self.peers.get(peer_id)
    }
    
    /// Get all peers
    pub fn all_peers(&self) -> Vec<&PeerInfo> {
        self.peers.values().collect()
    }
    
    /// Get number of peers
    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }
    
    /// Select peers for broadcast using equilibrium fanout
    /// 
    /// Fanout = √n × η
    /// 
    /// Where:
    /// - n = total number of peers
    /// - η = equilibrium constant (1/√2 ≈ 0.7071)
    /// 
    /// This achieves optimal propagation:
    /// - Too few peers: slow propagation
    /// - Too many peers: network congestion
    /// - √n × η: critical damping (fastest without congestion)
    pub fn select_broadcast_peers(&self) -> Vec<PeerId> {
        let n = self.peers.len() as f64;
        
        if n == 0.0 {
            return vec![];
        }
        
        // Calculate equilibrium fanout
        let fanout = (n.sqrt() * self.eta).ceil() as usize;
        
        // Select peers with highest quality
        let mut peers: Vec<_> = self.peers.values().collect();
        peers.sort_by(|a, b| b.quality.total_cmp(&a.quality));
        
        peers.into_iter()
            .take(fanout)
            .map(|p| p.id)
            .collect()
    }
    
    /// Select peer for sync based on dimensional distance
    /// 
    /// Dimensional distance τ = |peer_height - required_height| × η
    /// 
    /// Selects the peer with smallest τ (closest in dimensional space)
    pub fn select_sync_peer(&self, required_height: u64) -> Option<PeerId> {
        // Find peers with height >= required_height
        let mut candidates: Vec<_> = self.peers.values()
            .filter(|p| p.best_height >= required_height)
            .collect();
        
        if candidates.is_empty() {
            return None;
        }
        
        // Sort by dimensional distance (smallest τ first)
        candidates.sort_by_key(|p| {
            let delta = (p.best_height - required_height) as f64;
            let tau = delta * self.eta;
            (tau * 1000.0) as u64  // Scale for integer sorting
        });
        
        // Return closest peer
        Some(candidates[0].id)
    }
    
    /// Select multiple peers for parallel sync
    /// 
    /// Selects √n × η peers for parallel block requests
    pub fn select_sync_peers(&self, required_height: u64, max_peers: usize) -> Vec<PeerId> {
        // Find peers with height >= required_height
        let mut candidates: Vec<_> = self.peers.values()
            .filter(|p| p.best_height >= required_height)
            .collect();
        
        if candidates.is_empty() {
            return vec![];
        }
        
        // Calculate equilibrium fanout
        let n = candidates.len() as f64;
        let fanout = ((n.sqrt() * self.eta).ceil() as usize).min(max_peers);
        
        // Sort by quality and dimensional distance
        candidates.sort_by(|a, b| {
            let a_tau = ((a.best_height - required_height) as f64) * self.eta;
            let b_tau = ((b.best_height - required_height) as f64) * self.eta;
            
            // Combine quality and distance (higher quality, lower distance = better)
            let a_score = a.quality - a_tau;
            let b_score = b.quality - b_tau;
            
            b_score.total_cmp(&a_score)
        });
        
        candidates.into_iter()
            .take(fanout)
            .map(|p| p.id)
            .collect()
    }
    
    /// Select peers by node type
    pub fn select_peers_by_type(&self, node_type: u8) -> Vec<PeerId> {
        self.peers.values()
            .filter(|p| p.node_type == node_type)
            .map(|p| p.id)
            .collect()
    }
    
    /// Calculate optimal chunk size for sync
    /// 
    /// Uses equilibrium-based adaptive chunking:
    /// chunk_size = base × (1 + Δh × η / scale)
    /// 
    /// Where:
    /// - Δh = height difference
    /// - η = equilibrium constant
    /// - scale = scaling factor (default 10)
    pub fn calculate_chunk_size(&self, height_delta: u64, base_chunk: u64, max_chunk: u64) -> u64 {
        let delta = height_delta as f64;
        let adaptive = (base_chunk as f64) * (1.0 + (delta * self.eta / 10.0));
        adaptive.min(max_chunk as f64).ceil() as u64
    }
    
    /// Update peer quality based on performance
    /// 
    /// Quality decays exponentially on failure, increases linearly on success
    pub fn update_peer_quality(&mut self, peer_id: &PeerId, success: bool) {
        if let Some(peer) = self.peers.get_mut(peer_id) {
            if success {
                // Linear increase on success
                peer.quality = (peer.quality + 0.1).min(1.0);
            } else {
                // Exponential decay on failure (multiply by (1 - η))
                peer.quality *= 1.0 - self.eta;
                peer.quality = peer.quality.max(0.1);  // Minimum quality
            }
        }
    }

    // =========================================================================
    // Murmuration-Aware Peer Selection
    // =========================================================================

    /// Select peers for broadcast using murmuration flocking rules
    ///
    /// Combines equilibrium fanout with Reynolds flocking:
    /// - SEPARATION: Avoid peers with divergent chain views
    /// - ALIGNMENT: Prefer peers with similar height velocity
    /// - COHESION: Prefer peers near swarm center (median height)
    ///
    /// Fanout = √n × η × cohesion_factor
    pub fn select_broadcast_peers_flock(&self, our_height: u64, our_phase: u8) -> Vec<PeerId> {
        let n = self.peers.len() as f64;

        if n == 0.0 {
            return vec![];
        }

        // Calculate swarm metrics
        let heights: Vec<u64> = self.peers.values().map(|p| p.best_height).collect();
        let swarm_center = self.calculate_median(&heights);
        let cohesion = self.calculate_cohesion(&heights, swarm_center);

        // Adaptive fanout based on swarm cohesion
        // High cohesion -> lower fanout (swarm is aligned)
        // Low cohesion -> higher fanout (need to re-converge)
        let cohesion_factor = 1.0 + (1.0 - cohesion) * PHI_INV;
        let fanout = (n.sqrt() * self.eta * cohesion_factor).ceil() as usize;

        // Score all peers using flocking rules
        let mut scored: Vec<(&PeerInfo, f64)> = self.peers.values()
            .map(|p| {
                let sep = self.separation_score(p.best_height, our_height, swarm_center);
                let align = self.alignment_score(p, our_phase);
                let coh = self.cohesion_score(p.best_height, swarm_center, cohesion);
                let total = sep + align + coh + p.quality * 0.5;
                (p, total)
            })
            .collect();

        // Sort by score (highest first)
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        scored.into_iter()
            .take(fanout)
            .map(|(p, _)| p.id)
            .collect()
    }

    /// Calculate median height of peer set
    fn calculate_median(&self, heights: &[u64]) -> u64 {
        if heights.is_empty() {
            return 0;
        }
        let mut sorted = heights.to_vec();
        sorted.sort();
        sorted[sorted.len() / 2]
    }

    /// Calculate swarm cohesion (0.0 = fragmented, 1.0 = tight)
    fn calculate_cohesion(&self, heights: &[u64], _center: u64) -> f64 {
        if heights.is_empty() {
            return 1.0;
        }
        let mean = heights.iter().sum::<u64>() as f64 / heights.len() as f64;
        let variance = heights.iter()
            .map(|h| (*h as f64 - mean).powi(2))
            .sum::<f64>() / heights.len() as f64;

        // Cohesion = 1 / (1 + normalized_variance)
        let normalized_var = variance.sqrt() / (mean.max(1.0) * self.eta);
        1.0 / (1.0 + normalized_var)
    }

    /// Separation score: penalize peers far from our height
    fn separation_score(&self, peer_height: u64, our_height: u64, swarm_center: u64) -> f64 {
        let delta = (peer_height as i64 - our_height as i64).abs() as f64;
        let threshold = (swarm_center as f64) * self.eta;

        if delta > threshold {
            // Exponential penalty for divergent peers
            -((delta - threshold) / threshold.max(1.0)).exp() * PHI_INV
        } else {
            0.0
        }
    }

    /// Alignment score: prefer peers in same flock phase
    fn alignment_score(&self, peer: &PeerInfo, our_phase: u8) -> f64 {
        // Peers in same phase get bonus (they broadcast together)
        let phase_match = if peer.flock_phase == our_phase { 0.3 } else { 0.0 };

        // Also consider velocity alignment
        let vel_factor = 1.0 / (1.0 + peer.velocity.abs());

        self.eta * (phase_match + vel_factor * 0.2)
    }

    /// Cohesion score: prefer peers near swarm center
    fn cohesion_score(&self, peer_height: u64, swarm_center: u64, cohesion: f64) -> f64 {
        let distance = (peer_height as f64 - swarm_center as f64).abs();
        // Higher score for peers closer to consensus
        (1.0 - PHI_INV) * cohesion / (1.0 + distance * self.eta)
    }

    /// Update peer with flock state from StatusMessage
    pub fn update_peer_flock(&mut self, peer_id: &PeerId, flock: &FlockStateCompact, new_height: u64) {
        if let Some(peer) = self.peers.get_mut(peer_id) {
            // Calculate velocity (height change rate)
            let height_delta = new_height as f64 - peer.best_height as f64;
            peer.velocity = peer.velocity * (1.0 - self.eta) + height_delta * self.eta;

            // Update flock state
            peer.flock_phase = flock.phase;
            peer.flock_epoch = flock.epoch;
            peer.best_height = new_height;
        }
    }
}

impl Default for EquilibriumRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    fn create_test_peer(id: u8, height: u64, quality: f64) -> PeerInfo {
        PeerInfo {
            id: [id; 32],
            best_height: height,
            node_type: 1,
            quality,
            last_seen: 0,
            flock_phase: id % 8,
            flock_epoch: 0,
            velocity: 0.0,
        }
    }
    
    #[test]
    fn test_broadcast_fanout() {
        let mut router = EquilibriumRouter::new();
        
        // Add 16 peers
        for i in 0..16 {
            router.add_peer(create_test_peer(i, 100, 1.0));
        }
        
        // Fanout should be √16 × η = 4 × 0.707 ≈ 3
        let selected = router.select_broadcast_peers();
        assert!(selected.len() >= 2 && selected.len() <= 4);
    }
    
    #[test]
    fn test_sync_peer_selection() {
        let mut router = EquilibriumRouter::new();
        
        // Add peers with different heights
        router.add_peer(create_test_peer(1, 100, 1.0));
        router.add_peer(create_test_peer(2, 150, 1.0));
        router.add_peer(create_test_peer(3, 200, 1.0));
        router.add_peer(create_test_peer(4, 50, 1.0));  // Too low
        
        // Request height 120 - should select peer 2 (closest above)
        let selected = router.select_sync_peer(120).unwrap();
        assert_eq!(selected, [2; 32]);
    }
    
    #[test]
    fn test_chunk_size_calculation() {
        let router = EquilibriumRouter::new();
        
        // Small delta (10): chunk = 20 * (1 + (10 * η / 10)) = 20 * (1 + 0.707) ≈ 34
        let chunk = router.calculate_chunk_size(10, 20, 100);
        assert!((30..=40).contains(&chunk), "Expected chunk ~34 for delta=10, got {}", chunk);
        
        // Medium delta (100): chunk = 20 * (1 + (100 * η / 10)) = 20 * (1 + 7.07) ≈ 161, capped at 100
        let chunk = router.calculate_chunk_size(100, 20, 100);
        assert_eq!(chunk, 100, "Expected chunk=100 (capped) for delta=100, got {}", chunk);
        
        // Large delta (1000): chunk capped at max
        let chunk = router.calculate_chunk_size(1000, 20, 100);
        assert_eq!(chunk, 100, "Expected chunk=100 (capped) for delta=1000, got {}", chunk);
    }
    
    #[test]
    fn test_peer_quality_update() {
        let mut router = EquilibriumRouter::new();
        let peer_id = [1; 32];
        
        router.add_peer(create_test_peer(1, 100, 0.5));
        
        // Success increases quality
        router.update_peer_quality(&peer_id, true);
        assert!(router.get_peer(&peer_id).unwrap().quality > 0.5);
        
        // Failure decreases quality exponentially
        let before = router.get_peer(&peer_id).unwrap().quality;
        router.update_peer_quality(&peer_id, false);
        let after = router.get_peer(&peer_id).unwrap().quality;
        assert!(after < before);
        assert!((after - before * (1.0 - ETA)).abs() < 0.01);
    }
    
    #[test]
    fn test_equilibrium_scaling() {
        let router = EquilibriumRouter::new();
        
        // Verify η = 1/√2
        assert!((router.eta - std::f64::consts::FRAC_1_SQRT_2).abs() < 0.0001);
        
        // Verify √n × η scaling
        for n in [4, 9, 16, 25, 36] {
            let expected_fanout = ((n as f64).sqrt() * router.eta).ceil() as usize;
            
            let mut test_router = EquilibriumRouter::new();
            for i in 0..n {
                test_router.add_peer(create_test_peer(i as u8, 100, 1.0));
            }
            
            let selected = test_router.select_broadcast_peers();
            assert_eq!(selected.len(), expected_fanout);
        }
    }
}
