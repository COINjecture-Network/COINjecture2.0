// =============================================================================
// Epoch Phase State Machine
// =============================================================================
//
// Manages the lifecycle of a single epoch through four phases:
//   Salt → Mine → Commit → Seal
//
// Each phase has a deadline. The coordinator drives transitions by calling
// `try_advance()` with the current time. Invalid transitions are rejected.

use std::time::{Duration, Instant};

use super::config::CoordinatorConfig;

/// The four phases of an epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EpochPhase {
    /// Leader broadcasts the epoch salt derived from prev_hash.
    Salt,
    /// All nodes solve the NP-hard problem seeded by the salt.
    Mine,
    /// Nodes broadcast solution commitments (hash of solution).
    Commit,
    /// Leader aggregates commits, selects winner, produces block.
    Seal,
}

impl EpochPhase {
    /// Get the duration for this phase from config.
    pub fn duration(&self, config: &CoordinatorConfig) -> Duration {
        match self {
            EpochPhase::Salt => config.salt_duration,
            EpochPhase::Mine => config.mine_duration,
            EpochPhase::Commit => config.commit_duration,
            EpochPhase::Seal => config.seal_duration,
        }
    }

    /// Get the next phase, or None if this is the final phase.
    pub fn next(&self) -> Option<EpochPhase> {
        match self {
            EpochPhase::Salt => Some(EpochPhase::Mine),
            EpochPhase::Mine => Some(EpochPhase::Commit),
            EpochPhase::Commit => Some(EpochPhase::Seal),
            EpochPhase::Seal => None,
        }
    }
}

impl std::fmt::Display for EpochPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EpochPhase::Salt => write!(f, "Salt"),
            EpochPhase::Mine => write!(f, "Mine"),
            EpochPhase::Commit => write!(f, "Commit"),
            EpochPhase::Seal => write!(f, "Seal"),
        }
    }
}

/// State of the current epoch.
#[derive(Debug, Clone)]
pub struct EpochState {
    /// Current epoch number.
    pub epoch: u64,
    /// Current phase within the epoch.
    pub phase: EpochPhase,
    /// When the current phase started.
    pub phase_start: Instant,
    /// The epoch salt (set during Salt phase, used throughout).
    pub salt: Option<[u8; 32]>,
}

impl EpochState {
    /// Create a new epoch starting in the Salt phase.
    pub fn new(epoch: u64) -> Self {
        Self {
            epoch,
            phase: EpochPhase::Salt,
            phase_start: Instant::now(),
            salt: None,
        }
    }

    /// Create with an explicit start time (for testing).
    pub fn new_at(epoch: u64, start: Instant) -> Self {
        Self {
            epoch,
            phase: EpochPhase::Salt,
            phase_start: start,
            salt: None,
        }
    }

    /// How long the current phase has been running.
    pub fn phase_elapsed(&self) -> Duration {
        self.phase_start.elapsed()
    }

    /// Whether the current phase's duration has expired.
    pub fn phase_expired(&self, config: &CoordinatorConfig) -> bool {
        self.phase_elapsed() >= self.phase.duration(config)
    }

    /// Whether the epoch has stalled (phase expired + stall_timeout exceeded).
    pub fn is_stalled(&self, config: &CoordinatorConfig) -> bool {
        self.phase_elapsed() >= self.phase.duration(config) + config.stall_timeout
    }

    /// Try to advance to the next phase. Returns the new phase if successful,
    /// or None if we're at the end (Seal completed → new epoch needed).
    pub fn try_advance(&mut self, config: &CoordinatorConfig) -> Option<EpochPhase> {
        if !self.phase_expired(config) {
            return Some(self.phase); // Not ready yet, stay in current phase
        }

        match self.phase.next() {
            Some(next_phase) => {
                self.phase = next_phase;
                self.phase_start = Instant::now();
                Some(next_phase)
            }
            None => None, // Seal completed, epoch is done
        }
    }

    /// Force transition to a specific phase (used in stall recovery).
    pub fn force_phase(&mut self, phase: EpochPhase) {
        self.phase = phase;
        self.phase_start = Instant::now();
    }

    /// Set the epoch salt (called when Salt phase message is received/generated).
    pub fn set_salt(&mut self, salt: [u8; 32]) {
        self.salt = Some(salt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_phase_ordering() {
        assert_eq!(EpochPhase::Salt.next(), Some(EpochPhase::Mine));
        assert_eq!(EpochPhase::Mine.next(), Some(EpochPhase::Commit));
        assert_eq!(EpochPhase::Commit.next(), Some(EpochPhase::Seal));
        assert_eq!(EpochPhase::Seal.next(), None);
    }

    #[test]
    fn test_phase_durations() {
        let config = CoordinatorConfig::default();
        assert_eq!(EpochPhase::Salt.duration(&config), Duration::from_secs(2));
        assert_eq!(EpochPhase::Mine.duration(&config), Duration::from_secs(20));
        assert_eq!(EpochPhase::Commit.duration(&config), Duration::from_secs(5));
        assert_eq!(EpochPhase::Seal.duration(&config), Duration::from_secs(3));
    }

    #[test]
    fn test_epoch_state_new() {
        let state = EpochState::new(42);
        assert_eq!(state.epoch, 42);
        assert_eq!(state.phase, EpochPhase::Salt);
        assert!(state.salt.is_none());
    }

    #[test]
    fn test_phase_not_expired_immediately() {
        let config = CoordinatorConfig::default();
        let state = EpochState::new(1);
        assert!(!state.phase_expired(&config));
        assert!(!state.is_stalled(&config));
    }

    #[test]
    fn test_try_advance_not_ready() {
        let config = CoordinatorConfig::default();
        let mut state = EpochState::new(1);
        // Phase just started, should stay in Salt
        let result = state.try_advance(&config);
        assert_eq!(result, Some(EpochPhase::Salt));
    }

    #[test]
    fn test_try_advance_expired() {
        let config = CoordinatorConfig {
            salt_duration: Duration::from_millis(1),
            ..CoordinatorConfig::default()
        };
        let mut state = EpochState::new_at(1, Instant::now() - Duration::from_millis(10));
        let result = state.try_advance(&config);
        assert_eq!(result, Some(EpochPhase::Mine));
        assert_eq!(state.phase, EpochPhase::Mine);
    }

    #[test]
    fn test_full_epoch_lifecycle() {
        let config = CoordinatorConfig {
            salt_duration: Duration::from_millis(1),
            mine_duration: Duration::from_millis(1),
            commit_duration: Duration::from_millis(1),
            seal_duration: Duration::from_millis(1),
            ..CoordinatorConfig::default()
        };

        let mut state = EpochState::new_at(1, Instant::now() - Duration::from_millis(10));

        // Salt → Mine
        assert_eq!(state.try_advance(&config), Some(EpochPhase::Mine));
        state.phase_start = Instant::now() - Duration::from_millis(10);

        // Mine → Commit
        assert_eq!(state.try_advance(&config), Some(EpochPhase::Commit));
        state.phase_start = Instant::now() - Duration::from_millis(10);

        // Commit → Seal
        assert_eq!(state.try_advance(&config), Some(EpochPhase::Seal));
        state.phase_start = Instant::now() - Duration::from_millis(10);

        // Seal → None (epoch complete)
        assert_eq!(state.try_advance(&config), None);
    }

    #[test]
    fn test_force_phase() {
        let mut state = EpochState::new(1);
        state.force_phase(EpochPhase::Commit);
        assert_eq!(state.phase, EpochPhase::Commit);
    }

    #[test]
    fn test_set_salt() {
        let mut state = EpochState::new(1);
        assert!(state.salt.is_none());
        state.set_salt([0xAB; 32]);
        assert_eq!(state.salt, Some([0xAB; 32]));
    }

    #[test]
    fn test_stall_detection() {
        let config = CoordinatorConfig {
            salt_duration: Duration::from_millis(1),
            stall_timeout: Duration::from_millis(1),
            ..CoordinatorConfig::default()
        };
        let state = EpochState::new_at(1, Instant::now() - Duration::from_millis(10));
        assert!(state.phase_expired(&config));
        assert!(state.is_stalled(&config));
    }

    #[test]
    fn test_phase_display() {
        assert_eq!(format!("{}", EpochPhase::Salt), "Salt");
        assert_eq!(format!("{}", EpochPhase::Mine), "Mine");
        assert_eq!(format!("{}", EpochPhase::Commit), "Commit");
        assert_eq!(format!("{}", EpochPhase::Seal), "Seal");
    }
}
