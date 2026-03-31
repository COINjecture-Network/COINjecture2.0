// =============================================================================
// COINjecture P2P Protocol (CPP) - Node Classification Integration
// =============================================================================
// Integrates CPP protocol with node classification system using dimensionless metrics

use crate::cpp::{
    config::NodeType as CppNodeType,
    message::MessagePriority,
    peer::Peer,
    router::{PeerId, PeerInfo},
};

/// Node classification thresholds (dimensionless ratios)
///
/// These match the thresholds in node/src/node_types.rs but are
/// expressed as pure dimensionless ratios for CPP protocol use.
pub mod thresholds {
    /// Archive node: stores >= 95% of chain
    pub const ARCHIVE_STORAGE_RATIO: f64 = 0.95;

    /// Full node: stores >= 50% of chain
    pub const FULL_STORAGE_RATIO: f64 = 0.50;

    /// Light node: stores < 1% (headers only)
    pub const LIGHT_STORAGE_RATIO: f64 = 0.01;

    /// Validator: validates >= 10 blocks/second
    pub const VALIDATOR_SPEED_RATIO: f64 = 10.0; // blocks/sec

    /// Bounty: solves >= 5 problems/hour
    pub const BOUNTY_SOLVE_RATE: f64 = 5.0; // solutions/hour

    /// Oracle: uptime >= 99%
    pub const ORACLE_UPTIME_RATIO: f64 = 0.99;

    /// Minimum observation period for classification
    pub const MIN_OBSERVATION_BLOCKS: u64 = 1000;
}

/// Node metrics for classification (all dimensionless or rate-based)
#[derive(Debug, Clone)]
pub struct NodeMetrics {
    /// Storage ratio: blocks_stored / total_chain_length
    pub storage_ratio: f64,

    /// Validation speed: blocks_validated / time_elapsed (blocks/sec)
    pub validation_speed: f64,

    /// Solve rate: problems_solved / time_elapsed (solutions/hour)
    pub solve_rate: f64,

    /// Uptime ratio: active_time / total_time
    pub uptime_ratio: f64,

    /// Response time: average time to respond to requests (seconds)
    pub avg_response_time: f64,

    /// Bandwidth ratio: bytes_transferred / reference_bandwidth
    pub bandwidth_ratio: f64,

    /// Number of blocks observed (for classification confidence)
    pub blocks_observed: u64,
}

impl NodeMetrics {
    /// Create default metrics
    pub fn new() -> Self {
        NodeMetrics {
            storage_ratio: 0.0,
            validation_speed: 0.0,
            solve_rate: 0.0,
            uptime_ratio: 1.0,
            avg_response_time: 0.1, // 100ms default
            bandwidth_ratio: 0.0,
            blocks_observed: 0,
        }
    }

    /// Check if metrics are sufficient for classification
    pub fn sufficient_for_classification(&self) -> bool {
        self.blocks_observed >= thresholds::MIN_OBSERVATION_BLOCKS
    }

    /// Classify node type based on metrics
    pub fn classify(&self) -> CppNodeType {
        // Validator: high validation speed
        if self.validation_speed >= thresholds::VALIDATOR_SPEED_RATIO {
            return CppNodeType::Validator;
        }

        // Oracle: very high uptime
        if self.uptime_ratio >= thresholds::ORACLE_UPTIME_RATIO {
            return CppNodeType::Oracle;
        }

        // Bounty: high solve rate
        if self.solve_rate >= thresholds::BOUNTY_SOLVE_RATE {
            return CppNodeType::Bounty;
        }

        // Archive: stores most of chain
        if self.storage_ratio >= thresholds::ARCHIVE_STORAGE_RATIO {
            return CppNodeType::Archive;
        }

        // Full: stores significant portion of chain
        if self.storage_ratio >= thresholds::FULL_STORAGE_RATIO {
            return CppNodeType::Full;
        }

        // Light: minimal storage
        CppNodeType::Light
    }
}

impl Default for NodeMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Node type extensions for CPP protocol
impl CppNodeType {
    /// Get default message priority for this node type
    pub fn default_priority(&self) -> MessagePriority {
        match self {
            CppNodeType::Validator => MessagePriority::D1_Critical, // Consensus-critical
            CppNodeType::Oracle => MessagePriority::D2_High,        // External data important
            CppNodeType::Full => MessagePriority::D3_Normal,        // Standard operation
            CppNodeType::Bounty => MessagePriority::D4_Low,         // Computation-focused
            CppNodeType::Light => MessagePriority::D5_Background,   // Limited bandwidth
            CppNodeType::Archive => MessagePriority::D7_Archive,    // Historical data
        }
    }

    /// Get reward multiplier based on dimensional scale
    ///
    /// Matches the golden ratio cascade from node/src/node_types.rs
    pub fn reward_multiplier(&self) -> f64 {
        match self {
            CppNodeType::Validator => 1.000, // D1 - highest contribution
            CppNodeType::Oracle => 0.750,    // D3 - data premium
            CppNodeType::Bounty => 0.618,    // D4 - golden ratio (φ⁻¹)
            CppNodeType::Full => 0.500,      // D5 - standard (2⁻¹)
            CppNodeType::Archive => 0.382,   // D6 - storage premium (φ⁻²)
            CppNodeType::Light => 0.146,     // D8 - minimal (e⁻¹)
        }
    }

    /// Get expected bandwidth ratio for this node type
    pub fn expected_bandwidth_ratio(&self) -> f64 {
        match self {
            CppNodeType::Validator => 0.8, // High bandwidth
            CppNodeType::Oracle => 0.6,    // Medium-high
            CppNodeType::Full => 0.5,      // Medium
            CppNodeType::Archive => 0.7,   // High (serving historical data)
            CppNodeType::Bounty => 0.3,    // Low (computation-focused)
            CppNodeType::Light => 0.1,     // Very low
        }
    }

    /// Get expected storage ratio for this node type
    pub fn expected_storage_ratio(&self) -> f64 {
        match self {
            CppNodeType::Archive => 0.95,   // Stores almost everything
            CppNodeType::Full => 0.50,      // Stores half the chain
            CppNodeType::Validator => 0.50, // Stores enough to validate
            CppNodeType::Oracle => 0.30,    // Stores recent data
            CppNodeType::Bounty => 0.20,    // Stores minimal chain data
            CppNodeType::Light => 0.01,     // Headers only
        }
    }

    /// Check if this node type should serve sync requests
    pub fn can_serve_sync(&self) -> bool {
        matches!(
            self,
            CppNodeType::Full | CppNodeType::Archive | CppNodeType::Validator
        )
    }

    /// Check if this node type should participate in consensus
    pub fn can_validate(&self) -> bool {
        matches!(self, CppNodeType::Validator)
    }

    /// Check if this node type should solve bounty problems
    pub fn can_solve_bounties(&self) -> bool {
        matches!(
            self,
            CppNodeType::Bounty | CppNodeType::Full | CppNodeType::Validator
        )
    }
}

/// Convert peer to PeerInfo for routing
impl From<&Peer> for PeerInfo {
    fn from(peer: &Peer) -> Self {
        PeerInfo {
            id: peer.id,
            best_height: peer.best_height,
            node_type: peer.node_type as u8,
            quality: peer.quality,
            last_seen: peer.last_seen.elapsed().as_secs(),
            // Murmuration fields - default until updated from StatusMessage
            flock_phase: 0,
            flock_epoch: 0,
            velocity: 0.0,
        }
    }
}

/// Peer selection strategies based on node type
pub struct PeerSelector;

impl PeerSelector {
    /// Select best peers for block propagation
    ///
    /// Prioritizes: Validators > Full > Archive
    pub fn select_for_propagation(peers: &[&Peer], count: usize) -> Vec<PeerId> {
        let mut scored: Vec<_> = peers
            .iter()
            .map(|p| {
                let type_score = match p.node_type {
                    CppNodeType::Validator => 1.0,
                    CppNodeType::Full => 0.8,
                    CppNodeType::Archive => 0.6,
                    CppNodeType::Oracle => 0.4,
                    CppNodeType::Bounty => 0.3,
                    CppNodeType::Light => 0.1,
                };

                let score = type_score * 0.5 + p.quality * 0.5;
                (p, score)
            })
            .collect();

        scored.sort_by(|a, b| b.1.total_cmp(&a.1));

        scored.into_iter().take(count).map(|(p, _)| p.id).collect()
    }

    /// Select best peers for sync
    ///
    /// Prioritizes: Archive > Full > Validator
    pub fn select_for_sync(peers: &[&Peer], required_height: u64, count: usize) -> Vec<PeerId> {
        let mut candidates: Vec<_> = peers
            .iter()
            .filter(|p| p.best_height >= required_height && p.node_type.can_serve_sync())
            .collect();

        candidates.sort_by(|a, b| {
            let a_score = match a.node_type {
                CppNodeType::Archive => 1.0,
                CppNodeType::Full => 0.8,
                CppNodeType::Validator => 0.6,
                _ => 0.0,
            } * a.quality;

            let b_score = match b.node_type {
                CppNodeType::Archive => 1.0,
                CppNodeType::Full => 0.8,
                CppNodeType::Validator => 0.6,
                _ => 0.0,
            } * b.quality;

            b_score.total_cmp(&a_score)
        });

        candidates.into_iter().take(count).map(|p| p.id).collect()
    }

    /// Select best peers for bounty problem distribution
    ///
    /// Prioritizes: Bounty > Validator > Full
    pub fn select_for_bounties(peers: &[&Peer], count: usize) -> Vec<PeerId> {
        let mut scored: Vec<_> = peers
            .iter()
            .filter(|p| p.node_type.can_solve_bounties())
            .map(|p| {
                let type_score = match p.node_type {
                    CppNodeType::Bounty => 1.0,
                    CppNodeType::Validator => 0.7,
                    CppNodeType::Full => 0.5,
                    _ => 0.0,
                };

                let score = type_score * 0.6 + p.quality * 0.4;
                (p, score)
            })
            .collect();

        scored.sort_by(|a, b| b.1.total_cmp(&a.1));

        scored.into_iter().take(count).map(|(p, _)| p.id).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_classification() {
        // Validator: high validation speed (>= 10.0 blocks/sec) overrides storage
        let mut metrics = NodeMetrics::new();
        metrics.validation_speed = 15.0;
        metrics.blocks_observed = 1000;
        metrics.uptime_ratio = 0.5; // Set low to avoid Oracle classification
        assert_eq!(
            metrics.classify(),
            CppNodeType::Validator,
            "High validation speed should classify as Validator"
        );

        // Archive node: storage_ratio >= 0.95
        metrics.validation_speed = 0.0; // Reset
        metrics.storage_ratio = 0.96;
        metrics.uptime_ratio = 0.5; // Set low to avoid Oracle classification (default is 1.0 which triggers Oracle)
        assert_eq!(
            metrics.classify(),
            CppNodeType::Archive,
            "High storage ratio should classify as Archive"
        );

        // Full node: storage_ratio >= 0.50 but < 0.95
        metrics.storage_ratio = 0.60;
        metrics.uptime_ratio = 0.5; // Set low to avoid Oracle classification
        assert_eq!(
            metrics.classify(),
            CppNodeType::Full,
            "Medium storage ratio should classify as Full"
        );

        // Light node: storage_ratio < 0.50
        metrics.storage_ratio = 0.005;
        metrics.uptime_ratio = 0.5; // Set low to avoid Oracle classification
        assert_eq!(
            metrics.classify(),
            CppNodeType::Light,
            "Low storage ratio should classify as Light"
        );
    }

    #[test]
    fn test_reward_multipliers() {
        // Verify golden ratio cascade
        assert_eq!(CppNodeType::Validator.reward_multiplier(), 1.000);
        assert_eq!(CppNodeType::Bounty.reward_multiplier(), 0.618); // φ⁻¹
        assert_eq!(CppNodeType::Full.reward_multiplier(), 0.500); // 2⁻¹
        assert_eq!(CppNodeType::Archive.reward_multiplier(), 0.382); // φ⁻²

        // Verify ordering: Validator > Oracle > Bounty > Full > Archive > Light
        assert!(
            CppNodeType::Validator.reward_multiplier() > CppNodeType::Oracle.reward_multiplier()
        );
        assert!(CppNodeType::Oracle.reward_multiplier() > CppNodeType::Bounty.reward_multiplier());
        assert!(CppNodeType::Bounty.reward_multiplier() > CppNodeType::Full.reward_multiplier());
        assert!(CppNodeType::Full.reward_multiplier() > CppNodeType::Archive.reward_multiplier());
        assert!(CppNodeType::Archive.reward_multiplier() > CppNodeType::Light.reward_multiplier());
    }

    #[test]
    fn test_message_priorities() {
        // Validators get highest priority
        assert_eq!(
            CppNodeType::Validator.default_priority(),
            MessagePriority::D1_Critical
        );

        // Light nodes get lower priority
        assert_eq!(
            CppNodeType::Light.default_priority(),
            MessagePriority::D5_Background
        );

        // Archive nodes get archive priority
        assert_eq!(
            CppNodeType::Archive.default_priority(),
            MessagePriority::D7_Archive
        );
    }

    #[test]
    fn test_node_capabilities() {
        // Validators can validate
        assert!(CppNodeType::Validator.can_validate());
        assert!(!CppNodeType::Light.can_validate());

        // Full, Archive, Validator can serve sync
        assert!(CppNodeType::Full.can_serve_sync());
        assert!(CppNodeType::Archive.can_serve_sync());
        assert!(CppNodeType::Validator.can_serve_sync());
        assert!(!CppNodeType::Light.can_serve_sync());

        // Bounty, Full, Validator can solve bounties
        assert!(CppNodeType::Bounty.can_solve_bounties());
        assert!(CppNodeType::Full.can_solve_bounties());
        assert!(CppNodeType::Validator.can_solve_bounties());
        assert!(!CppNodeType::Light.can_solve_bounties());
    }
}
