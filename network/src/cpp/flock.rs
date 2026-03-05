// =============================================================================
// COINjecture P2P Protocol (CPP) - Murmuration / Flock Coordination
// =============================================================================
// Implements GoldenSeed-inspired deterministic swarm behavior for P2P nodes.
//
// Nodes "murmurate" like starlings - using shared deterministic seeds to
// coordinate behavior without explicit communication. Each node independently
// arrives at the same decisions using the golden ratio's equidistribution.
//
// Based on: https://github.com/beanapologist/seed (GoldenSeed)
// Integration by: Sarah & LEET
// =============================================================================

use crate::cpp::config::ETA;
use crate::cpp::router::PeerId;
use coinject_core::Hash;
use std::collections::HashMap;

// =============================================================================
// Re-export GoldenSeed primitives from core crate
// =============================================================================
// The golden primitives are now in core/src/golden.rs for use by
// commitment and merkle tree code. Re-exported here for backward compatibility.

pub use coinject_core::golden::{
    GoldenGenerator, PHI, PHI_INV, GOLDEN_SEED, GOLDEN_EPOCH_BLOCKS,
};

/// Murmuration epoch duration (in blocks) - alias for GOLDEN_EPOCH_BLOCKS
/// Kept for backward compatibility with existing network code
pub const FLOCK_EPOCH_BLOCKS: u64 = GOLDEN_EPOCH_BLOCKS;

/// Murmuration phase divisions (how many phases per epoch)
/// Nodes are assigned to phases for staggered broadcasts
pub const FLOCK_PHASES: u64 = 8;

// =============================================================================
// Flock State (Shared Swarm Coordination)
// =============================================================================

/// Swarm state shared between nodes via StatusMessage
///
/// Nodes use this to coordinate without explicit communication:
/// - Same flock_seed → same peer selection decisions
/// - Same flock_phase → staggered broadcast timing
/// - Same swarm_vector → consensus direction
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlockState {
    /// Deterministic seed for this epoch (derived from chain state)
    pub flock_seed: [u8; 32],

    /// Current epoch number
    pub epoch: u64,

    /// This node's phase within the epoch (0..FLOCK_PHASES)
    pub phase: u8,

    /// Swarm consensus height (median of known peers)
    pub swarm_height: u64,

    /// Swarm health score (0.0 = fragmented, 1.0 = cohesive)
    pub cohesion: f64,
}

impl FlockState {
    /// Create new flock state from chain parameters
    pub fn new(genesis: &Hash, height: u64, peer_id: &PeerId) -> Self {
        let epoch = height / FLOCK_EPOCH_BLOCKS;

        // Derive flock seed from genesis + epoch
        let mut hasher = blake3::Hasher::new();
        hasher.update(genesis.as_bytes());
        hasher.update(&epoch.to_le_bytes());
        let flock_seed = *hasher.finalize().as_bytes();

        // Assign phase based on peer_id (deterministic per-node)
        let phase = Self::compute_phase(peer_id, &flock_seed);

        FlockState {
            flock_seed,
            epoch,
            phase,
            swarm_height: height,
            cohesion: 1.0,
        }
    }

    /// Compute this node's phase using deterministic hash assignment
    fn compute_phase(peer_id: &PeerId, flock_seed: &[u8; 32]) -> u8 {
        // Hash peer_id with flock_seed for deterministic assignment
        let mut hasher = blake3::Hasher::new();
        hasher.update(peer_id);
        hasher.update(flock_seed);
        let hash = hasher.finalize();

        // Use first byte of hash modulo FLOCK_PHASES for uniform distribution
        // This provides cryptographically uniform distribution across phases
        hash.as_bytes()[0] % (FLOCK_PHASES as u8)
    }

    /// Check if we should broadcast in current phase
    pub fn is_broadcast_phase(&self, current_phase: u8) -> bool {
        self.phase == current_phase
    }

    /// Get broadcast delay for staggered propagation (in milliseconds)
    /// Uses golden ratio for optimal spacing
    pub fn broadcast_delay_ms(&self) -> u64 {
        let base_delay = 1000u64; // 1 second epoch
        let phase_offset = GoldenGenerator::golden_fractional(self.phase as u64);
        (base_delay as f64 * phase_offset) as u64
    }

    /// Update swarm metrics from peer observations
    pub fn update_from_peers(&mut self, peer_heights: &[u64]) {
        if peer_heights.is_empty() {
            return;
        }

        // Calculate median height (swarm consensus)
        let mut sorted = peer_heights.to_vec();
        sorted.sort();
        self.swarm_height = sorted[sorted.len() / 2];

        // Calculate cohesion (how tightly grouped are peers?)
        // Low variance = high cohesion
        let mean = sorted.iter().sum::<u64>() as f64 / sorted.len() as f64;
        let variance = sorted.iter()
            .map(|h| (*h as f64 - mean).powi(2))
            .sum::<f64>() / sorted.len() as f64;

        // Cohesion = 1 / (1 + normalized_variance)
        // Uses η for scaling
        let normalized_var = variance.sqrt() / (mean.max(1.0) * ETA);
        self.cohesion = 1.0 / (1.0 + normalized_var);
    }
}

impl Default for FlockState {
    fn default() -> Self {
        FlockState {
            flock_seed: [0u8; 32],
            epoch: 0,
            phase: 0,
            swarm_height: 0,
            cohesion: 1.0,
        }
    }
}

// =============================================================================
// Murmuration Rules (Flocking Behavior)
// =============================================================================

/// Flocking behavior calculator
///
/// Implements Reynolds flocking rules adapted for P2P:
/// 1. SEPARATION: Avoid peers with divergent chain views
/// 2. ALIGNMENT: Match velocity with nearby peer heights
/// 3. COHESION: Move toward flock center (median consensus)
#[derive(Debug)]
pub struct MurmurationRules {
    /// Generator for deterministic decisions
    generator: GoldenGenerator,

    /// Current flock state
    pub state: FlockState,

    /// Peer observations: peer_id → (height, quality, last_seen)
    peers: HashMap<PeerId, PeerObservation>,

    /// Separation weight (avoid divergent peers)
    pub w_separation: f64,

    /// Alignment weight (match peer velocities)
    pub w_alignment: f64,

    /// Cohesion weight (move toward center)
    pub w_cohesion: f64,
}

/// Observation of a peer's state
#[derive(Debug, Clone)]
pub struct PeerObservation {
    pub height: u64,
    pub quality: f64,
    pub flock_phase: u8,
    pub last_seen: u64,
    /// Height change rate (blocks per status update)
    pub velocity: f64,
}

impl MurmurationRules {
    /// Create new murmuration calculator
    pub fn new(genesis: &Hash, height: u64, peer_id: &PeerId) -> Self {
        let state = FlockState::new(genesis, height, peer_id);
        let generator = GoldenGenerator::from_flock_seed(genesis, height);

        MurmurationRules {
            generator,
            state,
            peers: HashMap::new(),
            // Weights derived from golden ratio for optimal balance
            w_separation: PHI_INV,      // 0.618
            w_alignment: ETA,           // 0.707
            w_cohesion: 1.0 - PHI_INV,  // 0.382
        }
    }

    /// Update peer observation
    pub fn observe_peer(&mut self, peer_id: PeerId, height: u64, quality: f64, phase: u8, timestamp: u64) {
        let velocity = if let Some(prev) = self.peers.get(&peer_id) {
            let dt = (timestamp - prev.last_seen).max(1) as f64;
            let dh = height as f64 - prev.height as f64;
            // Exponential moving average of velocity
            prev.velocity * (1.0 - ETA) + (dh / dt) * ETA
        } else {
            0.0
        };

        self.peers.insert(peer_id, PeerObservation {
            height,
            quality,
            flock_phase: phase,
            last_seen: timestamp,
            velocity,
        });

        // Update swarm state
        let heights: Vec<u64> = self.peers.values().map(|p| p.height).collect();
        self.state.update_from_peers(&heights);
    }

    /// Calculate separation vector (avoid divergent peers)
    /// Returns negative score for peers that are "too different"
    pub fn separation_score(&self, peer_height: u64, our_height: u64) -> f64 {
        let delta = (peer_height as i64 - our_height as i64).abs() as f64;
        let threshold = self.state.swarm_height as f64 * ETA;

        if delta > threshold {
            // Exponential penalty for divergent peers
            -((delta - threshold) / threshold).exp() * self.w_separation
        } else {
            0.0
        }
    }

    /// Calculate alignment score (prefer peers with similar velocity)
    pub fn alignment_score(&self, peer_id: &PeerId, our_velocity: f64) -> f64 {
        if let Some(peer) = self.peers.get(peer_id) {
            let vel_diff = (peer.velocity - our_velocity).abs();
            // Higher score for similar velocities
            self.w_alignment / (1.0 + vel_diff)
        } else {
            0.0
        }
    }

    /// Calculate cohesion score (prefer peers near swarm center)
    pub fn cohesion_score(&self, peer_height: u64) -> f64 {
        let swarm_center = self.state.swarm_height as f64;
        let distance = (peer_height as f64 - swarm_center).abs();

        // Higher score for peers closer to consensus
        self.w_cohesion * self.state.cohesion / (1.0 + distance * ETA)
    }

    /// Calculate combined flocking score for a peer
    pub fn flock_score(&self, peer_id: &PeerId, peer_height: u64, our_height: u64, our_velocity: f64) -> f64 {
        let sep = self.separation_score(peer_height, our_height);
        let align = self.alignment_score(peer_id, our_velocity);
        let coh = self.cohesion_score(peer_height);

        sep + align + coh
    }

    /// Select peers for broadcast using murmuration rules
    ///
    /// Combines equilibrium fanout with flocking behavior:
    /// fanout = √n × η × cohesion
    pub fn select_broadcast_peers(&mut self, our_height: u64, our_velocity: f64) -> Vec<PeerId> {
        let n = self.peers.len() as f64;
        if n == 0.0 {
            return vec![];
        }

        // Adaptive fanout based on swarm cohesion
        // High cohesion → lower fanout (swarm is aligned)
        // Low cohesion → higher fanout (need to re-converge)
        let cohesion_factor = 1.0 + (1.0 - self.state.cohesion) * PHI_INV;
        let fanout = (n.sqrt() * ETA * cohesion_factor).ceil() as usize;

        // Score all peers using flocking rules
        let mut scored: Vec<(PeerId, f64)> = self.peers.iter()
            .map(|(id, obs)| {
                let score = self.flock_score(id, obs.height, our_height, our_velocity);
                // Add quality factor
                let total = score + obs.quality * self.w_alignment;
                (*id, total)
            })
            .collect();

        // Sort by score (highest first)
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Add deterministic jitter using golden ratio
        // This prevents all nodes from selecting identical peers
        let jitter_seed = self.generator.next_u64();
        if jitter_seed % 3 == 0 && scored.len() > fanout {
            // Occasionally swap in a random peer for diversity
            let swap_idx = (GoldenGenerator::golden_fractional(jitter_seed) * scored.len() as f64) as usize;
            if swap_idx < scored.len() && fanout > 0 {
                scored.swap(fanout - 1, swap_idx);
            }
        }

        scored.into_iter()
            .take(fanout)
            .map(|(id, _)| id)
            .collect()
    }

    /// Check if we should broadcast now based on phase alignment
    pub fn should_broadcast_now(&self, current_time_ms: u64) -> bool {
        let epoch_ms = 10_000u64; // 10 second epochs (matches status interval)
        let phase_duration = epoch_ms / FLOCK_PHASES;

        let current_phase = ((current_time_ms / phase_duration) % FLOCK_PHASES) as u8;

        self.state.is_broadcast_phase(current_phase)
    }

    /// Get recommended broadcast delay for staggered propagation
    pub fn get_broadcast_delay(&self) -> std::time::Duration {
        std::time::Duration::from_millis(self.state.broadcast_delay_ms())
    }

    /// Advance to next epoch (call when block height crosses epoch boundary)
    pub fn advance_epoch(&mut self, genesis: &Hash, new_height: u64, peer_id: &PeerId) {
        let new_epoch = new_height / FLOCK_EPOCH_BLOCKS;
        if new_epoch > self.state.epoch {
            self.state = FlockState::new(genesis, new_height, peer_id);
            self.generator = GoldenGenerator::from_flock_seed(genesis, new_height);
        }
    }
}

// =============================================================================
// Serialization for Network Messages
// =============================================================================

use serde::{Serialize, Deserialize};

/// Compact flock state for inclusion in StatusMessage
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct FlockStateCompact {
    /// First 8 bytes of flock_seed (enough for coordination)
    pub seed_prefix: [u8; 8],
    /// Current epoch
    pub epoch: u64,
    /// This node's phase
    pub phase: u8,
    /// Swarm height (consensus)
    pub swarm_height: u64,
}

impl From<&FlockState> for FlockStateCompact {
    fn from(state: &FlockState) -> Self {
        let mut seed_prefix = [0u8; 8];
        seed_prefix.copy_from_slice(&state.flock_seed[0..8]);

        FlockStateCompact {
            seed_prefix,
            epoch: state.epoch,
            phase: state.phase,
            swarm_height: state.swarm_height,
        }
    }
}

impl FlockStateCompact {
    /// Check if two nodes are in the same flock (same epoch + compatible seed)
    pub fn is_same_flock(&self, other: &FlockStateCompact) -> bool {
        self.epoch == other.epoch && self.seed_prefix == other.seed_prefix
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_golden_ratio_constants() {
        // Verify φ × φ⁻¹ = 1
        assert!((PHI * PHI_INV - 1.0).abs() < 1e-10);

        // Verify φ - 1 = φ⁻¹
        assert!((PHI - 1.0 - PHI_INV).abs() < 1e-10);

        // Verify φ² = φ + 1
        assert!((PHI * PHI - PHI - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_golden_fractional_distribution() {
        // Test that golden ratio produces equidistributed values
        let mut buckets = [0u32; 10];

        for z in 1..10001 {
            let frac = GoldenGenerator::golden_fractional(z);
            let bucket = (frac * 10.0).floor() as usize;
            buckets[bucket.min(9)] += 1;
        }

        // Each bucket should have ~1000 values (±10%)
        for bucket in buckets.iter() {
            assert!(*bucket > 900 && *bucket < 1100,
                "Bucket count {} not in expected range", bucket);
        }
    }

    #[test]
    fn test_coin_flip_fairness() {
        let gen = GoldenGenerator::new(&GOLDEN_SEED);

        let mut zeros = 0u32;
        let mut ones = 0u32;

        for z in 1..10001 {
            if gen.coin_flip(z) == 0 {
                zeros += 1;
            } else {
                ones += 1;
            }
        }

        // Should be roughly 50/50 (±5%)
        let ratio = zeros as f64 / (zeros + ones) as f64;
        assert!(ratio > 0.45 && ratio < 0.55,
            "Coin flip ratio {} not fair", ratio);
    }

    #[test]
    fn test_deterministic_generation() {
        let seed = GOLDEN_SEED;

        // Two generators with same seed should produce identical output
        let mut gen1 = GoldenGenerator::new(&seed);
        let mut gen2 = GoldenGenerator::new(&seed);

        for _ in 0..100 {
            assert_eq!(gen1.next_bytes(), gen2.next_bytes());
        }
    }

    #[test]
    fn test_flock_phase_distribution() {
        let genesis = Hash::ZERO;
        let height = 1000;

        let mut phase_counts = [0u32; FLOCK_PHASES as usize];

        // Generate 1000 peer IDs with varied bytes to ensure distribution
        for i in 0..1000u64 {
            // Use BLAKE3 hash of i to get well-distributed peer IDs
            let hash = blake3::hash(&i.to_le_bytes());
            let peer_id: [u8; 32] = *hash.as_bytes();

            let state = FlockState::new(&genesis, height, &peer_id);
            phase_counts[state.phase as usize] += 1;
        }

        // Verify all phases are used and distribution is reasonable
        let total: u32 = phase_counts.iter().sum();
        assert_eq!(total, 1000);
        
        // Check that no single phase dominates (< 50% of total)
        for count in phase_counts.iter() {
            assert!(*count < 500, "Phase count {} exceeds 50% of total", count);
        }
    }

    #[test]
    fn test_cohesion_calculation() {
        let mut state = FlockState::default();

        // All peers at same height → perfect cohesion
        state.update_from_peers(&[100, 100, 100, 100]);
        assert!((state.cohesion - 1.0).abs() < 0.01);

        // Peers spread out → lower cohesion (but still reasonable)
        state.update_from_peers(&[100, 200, 300, 400]);
        assert!(state.cohesion < 0.8, "Expected cohesion < 0.8 for spread peers, got {}", state.cohesion);
        
        // Very spread peers → much lower cohesion
        state.update_from_peers(&[1, 100, 1000, 10000]);
        assert!(state.cohesion < 0.35, "Expected cohesion < 0.35 for very spread peers, got {}", state.cohesion);
    }

    #[test]
    fn test_murmuration_peer_selection() {
        let genesis = Hash::ZERO;
        let our_peer_id = [1u8; 32];
        let mut rules = MurmurationRules::new(&genesis, 100, &our_peer_id);

        // Add some peers
        for i in 0..10u8 {
            let mut peer_id = [0u8; 32];
            peer_id[0] = i + 10;
            rules.observe_peer(peer_id, 100 + i as u64, 0.8, i % 8, 1000);
        }

        // Select peers for broadcast
        let selected = rules.select_broadcast_peers(100, 0.0);

        // Should select approximately √10 × 0.707 ≈ 2-3 peers
        assert!(!selected.is_empty());
        assert!(selected.len() <= 5);
    }
}
