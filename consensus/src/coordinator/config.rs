// =============================================================================
// Epoch Coordinator Configuration
// =============================================================================
//
// Phase durations, quorum thresholds, and stall recovery settings for the
// multi-node epoch coordination protocol.

use std::time::Duration;

/// Configuration for the EpochCoordinator.
///
/// Default phase durations are tuned for a 30-second target block time:
///   Salt(2s) + Mine(20s) + Commit(5s) + Seal(3s) = 30s
#[derive(Debug, Clone)]
pub struct CoordinatorConfig {
    // ── Phase durations ──────────────────────────────────────────────────
    /// Duration of the Salt phase (leader broadcasts epoch salt).
    pub salt_duration: Duration,

    /// Duration of the Mine phase (all nodes solve NP-hard problems).
    pub mine_duration: Duration,

    /// Duration of the Commit phase (nodes broadcast solution commitments).
    pub commit_duration: Duration,

    /// Duration of the Seal phase (leader aggregates and produces block).
    pub seal_duration: Duration,

    // ── Quorum ───────────────────────────────────────────────────────────
    /// Minimum fraction of known peers that must commit for a valid seal.
    /// Range: 0.0..=1.0. Default: 0.67 (2/3 supermajority).
    pub quorum_threshold: f64,

    // ── Stall recovery ───────────────────────────────────────────────────
    /// How long to wait for a phase to complete before declaring a stall.
    /// If a phase exceeds its duration + stall_timeout, the coordinator
    /// force-transitions to the next epoch with the current leader rotated out.
    pub stall_timeout: Duration,

    /// Maximum consecutive stalls before hard-resetting to epoch 0.
    pub max_consecutive_stalls: u32,

    // ── Leader election ──────────────────────────────────────────────────
    /// Number of failover candidates considered when the primary leader stalls.
    /// The top N candidates from the sorted peer set are eligible as fallback leaders.
    pub failover_depth: usize,
}

impl Default for CoordinatorConfig {
    fn default() -> Self {
        Self {
            salt_duration: Duration::from_secs(2),
            mine_duration: Duration::from_secs(20),
            commit_duration: Duration::from_secs(5),
            seal_duration: Duration::from_secs(3),
            quorum_threshold: 0.67,
            stall_timeout: Duration::from_secs(10),
            max_consecutive_stalls: 3,
            failover_depth: 3,
        }
    }
}

impl CoordinatorConfig {
    /// Total expected epoch duration (sum of all phases).
    pub fn epoch_duration(&self) -> Duration {
        self.salt_duration + self.mine_duration + self.commit_duration + self.seal_duration
    }

    /// Maximum tolerable epoch duration before stall recovery kicks in.
    pub fn max_epoch_duration(&self) -> Duration {
        self.epoch_duration() + self.stall_timeout
    }

    /// Validate configuration values. Returns an error message if invalid.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.quorum_threshold <= 0.0 || self.quorum_threshold > 1.0 {
            return Err("quorum_threshold must be in (0.0, 1.0]");
        }
        if self.mine_duration.is_zero() {
            return Err("mine_duration must be > 0");
        }
        if self.failover_depth == 0 {
            return Err("failover_depth must be >= 1");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = CoordinatorConfig::default();
        assert_eq!(cfg.epoch_duration(), Duration::from_secs(30));
        assert_eq!(cfg.salt_duration, Duration::from_secs(2));
        assert_eq!(cfg.mine_duration, Duration::from_secs(20));
        assert_eq!(cfg.commit_duration, Duration::from_secs(5));
        assert_eq!(cfg.seal_duration, Duration::from_secs(3));
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_max_epoch_duration() {
        let cfg = CoordinatorConfig::default();
        assert_eq!(cfg.max_epoch_duration(), Duration::from_secs(40));
    }

    #[test]
    #[allow(clippy::field_reassign_with_default)]
    fn test_invalid_quorum() {
        let mut cfg = CoordinatorConfig::default();
        cfg.quorum_threshold = 0.0;
        assert!(cfg.validate().is_err());
        cfg.quorum_threshold = 1.5;
        assert!(cfg.validate().is_err());
    }

    #[test]
    #[allow(clippy::field_reassign_with_default)]
    fn test_invalid_mine_duration() {
        let mut cfg = CoordinatorConfig::default();
        cfg.mine_duration = Duration::ZERO;
        assert!(cfg.validate().is_err());
    }

    #[test]
    #[allow(clippy::field_reassign_with_default)]
    fn test_invalid_failover_depth() {
        let mut cfg = CoordinatorConfig::default();
        cfg.failover_depth = 0;
        assert!(cfg.validate().is_err());
    }
}
