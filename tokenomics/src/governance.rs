// =============================================================================
// Multi-Dimensional Governance
// voting_power = Σ(balance_n × D_n × unlock_n(τ))
// =============================================================================
// Rewards long-term holders across multiple dimensions

use coinject_core::ETA; // Import from core (single source of truth)
use crate::pools::PoolType;
use crate::staking::delta_critical;
use coinject_core::Address;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─── Proposal Actions ─────────────────────────────────────────────────────────

/// Typed payload that describes WHAT a proposal does when executed.
///
/// Each `ProposalType` maps to specific `ProposalAction` variants:
/// - `Parameter` proposals use `ChangeParameter`.
/// - `Upgrade` proposals use `ProtocolUpgrade`.
/// - `Treasury` proposals use `TreasuryTransfer`.
/// - `Emergency` proposals may use any variant.
/// - `Constitutional` proposals use `ConstitutionalAmendment`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProposalAction {
    /// Change a named network parameter.
    ///
    /// The `key` is a dot-path string (e.g. `"consensus.block_time_target_ms"`,
    /// `"mempool.max_tx_per_block"`, `"tokenomics.emission_rate_bps"`).
    /// The `value` is a JSON-encoded new value so arbitrary types can be carried.
    ChangeParameter {
        key: String,
        old_value: String,
        new_value: String,
    },

    /// Activate a new protocol version via feature flag.
    ///
    /// `target_version` is the CPP protocol version byte to activate.
    /// `activation_height` is the block height at which the upgrade takes effect.
    /// `feature_flags` is a bitmask of new features to enable (for soft-forks).
    ProtocolUpgrade {
        target_version: u8,
        activation_height: u64,
        description: String,
    },

    /// Transfer funds from the treasury pool to a recipient.
    TreasuryTransfer {
        recipient: Address,
        amount: u128,
        purpose: String,
    },

    /// Amend governance rules (quorum thresholds, voting period, etc.).
    ConstitutionalAmendment {
        amendment_text: String,
        new_quorum_threshold: Option<f64>,
        new_supermajority_threshold: Option<f64>,
        new_voting_period_blocks: Option<u64>,
    },

    /// Emergency: halt/resume the network or specific subsystem.
    EmergencyAction {
        action_type: EmergencyActionType,
        reason: String,
    },
}

/// Type of emergency action that can be triggered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EmergencyActionType {
    /// Pause all transaction processing (maintenance mode).
    PauseNetwork,
    /// Resume normal operation after a pause.
    ResumeNetwork,
    /// Freeze a specific account (exploit response).
    FreezeAccount,
    /// Slash a validator (misbehavior response).
    SlashValidator,
}

/// Execution result after a proposal action is applied.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionReceipt {
    pub proposal_id: u64,
    pub action: ProposalAction,
    pub executed_at: u64,
    pub success: bool,
    pub message: String,
}

/// Errors that can occur during proposal execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionError {
    /// Proposal is not in the Passed state.
    NotPassed,
    /// Timelock period has not elapsed.
    TimelockNotExpired,
    /// Proposal action payload is missing.
    NoAction,
    /// The action itself failed (with reason string).
    ActionFailed(String),
    /// Proposal was already executed.
    AlreadyExecuted,
}

/// Proposal threshold: total_supply × Δ_critical = 23.1%
pub fn proposal_threshold_pct() -> f64 { delta_critical() }

/// Quorum required for proposal to pass: 50%
pub const QUORUM_THRESHOLD: f64 = 0.50;

/// Supermajority for constitutional changes: 80%
pub const SUPERMAJORITY_THRESHOLD: f64 = 0.80;

/// Voting period in blocks
pub const VOTING_PERIOD_BLOCKS: u64 = 100_000;

/// Proposal types with different requirements
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalType {
    /// Simple parameter change (50% quorum)
    Parameter,
    /// Treasury allocation (50% quorum)
    Treasury,
    /// Protocol upgrade (80% supermajority)
    Upgrade,
    /// Constitutional change (80% supermajority + 30% participation)
    Constitutional,
    /// Emergency action (immediate execution if 90% agree)
    Emergency,
}

impl ProposalType {
    /// Get required approval threshold
    pub fn approval_threshold(&self) -> f64 {
        match self {
            ProposalType::Parameter => QUORUM_THRESHOLD,
            ProposalType::Treasury => QUORUM_THRESHOLD,
            ProposalType::Upgrade => SUPERMAJORITY_THRESHOLD,
            ProposalType::Constitutional => SUPERMAJORITY_THRESHOLD,
            ProposalType::Emergency => 0.90,
        }
    }

    /// Get required participation threshold
    pub fn participation_threshold(&self) -> f64 {
        match self {
            ProposalType::Parameter => 0.10,      // 10% participation
            ProposalType::Treasury => 0.15,       // 15% participation
            ProposalType::Upgrade => 0.20,        // 20% participation
            ProposalType::Constitutional => 0.30, // 30% participation
            ProposalType::Emergency => 0.05,      // 5% for emergencies
        }
    }

    /// Get timelock period in blocks
    pub fn timelock_blocks(&self) -> u64 {
        match self {
            ProposalType::Parameter => 10_000,     // ~1 day
            ProposalType::Treasury => 20_000,      // ~2 days
            ProposalType::Upgrade => 50_000,       // ~5 days
            ProposalType::Constitutional => 100_000, // ~10 days
            ProposalType::Emergency => 0,          // Immediate
        }
    }
}

/// Vote option
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VoteOption {
    For,
    Against,
    Abstain,
}

/// Voter's dimensional holdings for voting power calculation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoterPosition {
    /// Balances per dimensional pool
    pub balances: HashMap<PoolType, u128>,
    /// Stake start blocks per pool
    pub stake_starts: HashMap<PoolType, u64>,
}

impl VoterPosition {
    /// Calculate total voting power
    /// voting_power = Σ(balance_n × D_n × unlock_n(τ))
    pub fn calculate_voting_power(&self, current_block: u64) -> u128 {
        let power: f64 = self.balances.iter()
            .map(|(pool_type, balance)| {
                let d_n = pool_type.scale();
                let unlock = self.unlock_factor(*pool_type, current_block);
                (*balance as f64) * d_n * unlock
            })
            .sum();
        
        power as u128
    }

    /// Calculate unlock factor based on time staked
    fn unlock_factor(&self, pool_type: PoolType, current_block: u64) -> f64 {
        let start = self.stake_starts.get(&pool_type).copied().unwrap_or(current_block);
        let blocks_staked = current_block.saturating_sub(start);
        
        // Unlock follows same exponential curve
        // unlock = 1 - e^(-η × τ) where τ is time in years (scaled to blocks)
        let tau = (blocks_staked as f64) / 100_000.0; // ~1 year = 100k blocks
        1.0 - (-ETA * tau).exp()
    }
}

/// Governance proposal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proposal {
    /// Unique proposal ID
    pub id: u64,
    /// Proposal type
    pub proposal_type: ProposalType,
    /// Title
    pub title: String,
    /// Description
    pub description: String,
    /// Proposer address
    pub proposer: Address,
    /// Creation block
    pub created_at: u64,
    /// Voting starts at
    pub voting_starts: u64,
    /// Voting ends at
    pub voting_ends: u64,
    /// Execution block (after timelock)
    pub execution_at: u64,
    /// Current status
    pub status: ProposalStatus,
    /// Votes for
    pub votes_for: u128,
    /// Votes against
    pub votes_against: u128,
    /// Abstentions
    pub votes_abstain: u128,
    /// Individual votes by address
    pub votes: HashMap<Address, Vote>,
    /// Typed action to execute when this proposal passes.
    /// `None` = informational proposal with no on-chain effect.
    #[serde(default)]
    pub action: Option<ProposalAction>,
    /// Receipt populated after execution.
    #[serde(default)]
    pub execution_receipt: Option<ExecutionReceipt>,
}

/// Proposal status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalStatus {
    /// Pending voting period start
    Pending,
    /// Active voting
    Active,
    /// Passed, awaiting execution
    Passed,
    /// Failed to reach quorum or approval
    Failed,
    /// Executed
    Executed,
    /// Cancelled by proposer
    Cancelled,
}

/// Individual vote
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vote {
    pub voter: Address,
    pub option: VoteOption,
    pub power: u128,
    pub cast_at: u64,
}

impl Proposal {
    /// Create new proposal (informational — no on-chain action).
    pub fn new(
        id: u64,
        proposal_type: ProposalType,
        title: String,
        description: String,
        proposer: Address,
        current_block: u64,
    ) -> Self {
        Self::new_with_action(id, proposal_type, title, description, proposer, current_block, None)
    }

    /// Create new proposal with a typed action to execute on passage.
    pub fn new_with_action(
        id: u64,
        proposal_type: ProposalType,
        title: String,
        description: String,
        proposer: Address,
        current_block: u64,
        action: Option<ProposalAction>,
    ) -> Self {
        let voting_starts = current_block + 1000; // Start after 1000 blocks
        let voting_ends = voting_starts + VOTING_PERIOD_BLOCKS;
        let execution_at = voting_ends + proposal_type.timelock_blocks();

        Proposal {
            id,
            proposal_type,
            title,
            description,
            proposer,
            created_at: current_block,
            voting_starts,
            voting_ends,
            execution_at,
            status: ProposalStatus::Pending,
            votes_for: 0,
            votes_against: 0,
            votes_abstain: 0,
            votes: HashMap::new(),
            action,
            execution_receipt: None,
        }
    }

    /// Execute this proposal's action.
    ///
    /// # Lifecycle
    /// Proposal must be in `Passed` status and the timelock must have elapsed
    /// (`current_block >= execution_at`).  On success the status transitions to
    /// `Executed` and an `ExecutionReceipt` is stored.
    ///
    /// Callers are responsible for actually applying the action; this method
    /// only validates the preconditions and records the outcome.
    pub fn execute(
        &mut self,
        current_block: u64,
        success: bool,
        message: String,
    ) -> Result<ExecutionReceipt, ExecutionError> {
        if self.status == ProposalStatus::Executed {
            return Err(ExecutionError::AlreadyExecuted);
        }
        if self.status != ProposalStatus::Passed {
            return Err(ExecutionError::NotPassed);
        }
        if current_block < self.execution_at {
            return Err(ExecutionError::TimelockNotExpired);
        }

        let action = self.action.clone().ok_or(ExecutionError::NoAction)?;

        let receipt = ExecutionReceipt {
            proposal_id: self.id,
            action,
            executed_at: current_block,
            success,
            message,
        };

        self.execution_receipt = Some(receipt.clone());
        if success {
            self.status = ProposalStatus::Executed;
        }

        Ok(receipt)
    }

    /// Cast vote
    pub fn cast_vote(
        &mut self,
        voter: Address,
        option: VoteOption,
        power: u128,
        current_block: u64,
    ) -> Result<(), GovernanceError> {
        if current_block < self.voting_starts {
            return Err(GovernanceError::VotingNotStarted);
        }
        if current_block > self.voting_ends {
            return Err(GovernanceError::VotingEnded);
        }
        if self.status != ProposalStatus::Active {
            return Err(GovernanceError::NotActive);
        }
        if self.votes.contains_key(&voter) {
            return Err(GovernanceError::AlreadyVoted);
        }
        
        match option {
            VoteOption::For => self.votes_for += power,
            VoteOption::Against => self.votes_against += power,
            VoteOption::Abstain => self.votes_abstain += power,
        }
        
        self.votes.insert(voter, Vote {
            voter,
            option,
            power,
            cast_at: current_block,
        });
        
        Ok(())
    }

    /// Check and update proposal status
    pub fn update_status(&mut self, current_block: u64, total_voting_power: u128) {
        // Transition to active
        if self.status == ProposalStatus::Pending && current_block >= self.voting_starts {
            self.status = ProposalStatus::Active;
        }
        
        // Check voting ended
        if self.status == ProposalStatus::Active && current_block > self.voting_ends {
            let total_votes = self.votes_for + self.votes_against + self.votes_abstain;
            let participation = (total_votes as f64) / (total_voting_power as f64);
            let approval = if self.votes_for + self.votes_against > 0 {
                (self.votes_for as f64) / ((self.votes_for + self.votes_against) as f64)
            } else {
                0.0
            };
            
            let passed = participation >= self.proposal_type.participation_threshold()
                && approval >= self.proposal_type.approval_threshold();
            
            self.status = if passed {
                ProposalStatus::Passed
            } else {
                ProposalStatus::Failed
            };
        }
    }

    /// Get vote summary
    pub fn summary(&self, total_voting_power: u128) -> ProposalSummary {
        let total_votes = self.votes_for + self.votes_against + self.votes_abstain;
        let participation = (total_votes as f64) / (total_voting_power as f64);
        let approval = if self.votes_for + self.votes_against > 0 {
            (self.votes_for as f64) / ((self.votes_for + self.votes_against) as f64)
        } else {
            0.0
        };
        
        ProposalSummary {
            id: self.id,
            title: self.title.clone(),
            status: self.status,
            votes_for: self.votes_for,
            votes_against: self.votes_against,
            votes_abstain: self.votes_abstain,
            participation_pct: participation * 100.0,
            approval_pct: approval * 100.0,
            required_participation: self.proposal_type.participation_threshold() * 100.0,
            required_approval: self.proposal_type.approval_threshold() * 100.0,
            num_voters: self.votes.len(),
        }
    }
}

/// Proposal summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalSummary {
    pub id: u64,
    pub title: String,
    pub status: ProposalStatus,
    pub votes_for: u128,
    pub votes_against: u128,
    pub votes_abstain: u128,
    pub participation_pct: f64,
    pub approval_pct: f64,
    pub required_participation: f64,
    pub required_approval: f64,
    pub num_voters: usize,
}

/// Governance errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GovernanceError {
    VotingNotStarted,
    VotingEnded,
    NotActive,
    AlreadyVoted,
    InsufficientPower,
    ProposalNotFound,
}

/// Governance manager
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceManager {
    /// All proposals
    pub proposals: HashMap<u64, Proposal>,
    /// Next proposal ID
    pub next_id: u64,
    /// Total voting power in network
    pub total_voting_power: u128,
    /// Voter positions
    pub voter_positions: HashMap<Address, VoterPosition>,
}

impl GovernanceManager {
    pub fn new() -> Self {
        GovernanceManager {
            proposals: HashMap::new(),
            next_id: 1,
            total_voting_power: 0,
            voter_positions: HashMap::new(),
        }
    }

    /// Calculate required voting power to submit proposal
    pub fn proposal_threshold(&self) -> u128 {
        ((self.total_voting_power as f64) * proposal_threshold_pct()) as u128
    }

    /// Create new informational proposal (no on-chain action).
    pub fn create_proposal(
        &mut self,
        proposal_type: ProposalType,
        title: String,
        description: String,
        proposer: Address,
        current_block: u64,
    ) -> Result<u64, GovernanceError> {
        self.create_proposal_with_action(proposal_type, title, description, proposer, current_block, None)
    }

    /// Create new proposal with a typed on-chain action.
    pub fn create_proposal_with_action(
        &mut self,
        proposal_type: ProposalType,
        title: String,
        description: String,
        proposer: Address,
        current_block: u64,
        action: Option<ProposalAction>,
    ) -> Result<u64, GovernanceError> {
        let proposer_power = self.get_voting_power(&proposer, current_block);
        if proposer_power < self.proposal_threshold() {
            return Err(GovernanceError::InsufficientPower);
        }

        let id = self.next_id;
        self.next_id += 1;

        let proposal = Proposal::new_with_action(
            id,
            proposal_type,
            title,
            description,
            proposer,
            current_block,
            action,
        );

        self.proposals.insert(id, proposal);
        Ok(id)
    }

    /// Execute a passed proposal.
    ///
    /// Returns the `ExecutionReceipt` on success.
    /// The caller must interpret the receipt's `action` field and apply the
    /// actual state change (e.g., update a parameter, activate a protocol version).
    pub fn execute_proposal(
        &mut self,
        proposal_id: u64,
        current_block: u64,
    ) -> Result<ExecutionReceipt, ExecutionError> {
        let proposal = self.proposals.get_mut(&proposal_id)
            .ok_or(ExecutionError::ActionFailed("proposal not found".into()))?;

        // Dispatch based on action type and validate prerequisites
        match &proposal.action {
            None => return Err(ExecutionError::NoAction),
            Some(ProposalAction::ProtocolUpgrade { activation_height, .. }) => {
                if current_block < *activation_height {
                    return Err(ExecutionError::TimelockNotExpired);
                }
            }
            _ => {}
        }

        let message = format!(
            "Proposal {} executed at block {} by governance",
            proposal_id, current_block
        );
        proposal.execute(current_block, true, message)
    }

    /// Get proposals that are Passed and ready to execute (timelock elapsed).
    pub fn executable_proposals(&self, current_block: u64) -> Vec<&Proposal> {
        self.proposals.values()
            .filter(|p| {
                p.status == ProposalStatus::Passed
                    && current_block >= p.execution_at
                    && p.action.is_some()
            })
            .collect()
    }

    /// Get execution receipts for all executed proposals.
    pub fn execution_history(&self) -> Vec<&ExecutionReceipt> {
        self.proposals.values()
            .filter_map(|p| p.execution_receipt.as_ref())
            .collect()
    }

    /// Get voting power for address
    pub fn get_voting_power(&self, voter: &Address, current_block: u64) -> u128 {
        self.voter_positions
            .get(voter)
            .map(|pos| pos.calculate_voting_power(current_block))
            .unwrap_or(0)
    }

    /// Cast vote on proposal
    pub fn vote(
        &mut self,
        proposal_id: u64,
        voter: Address,
        option: VoteOption,
        current_block: u64,
    ) -> Result<(), GovernanceError> {
        let power = self.get_voting_power(&voter, current_block);
        if power == 0 {
            return Err(GovernanceError::InsufficientPower);
        }
        
        let proposal = self.proposals.get_mut(&proposal_id)
            .ok_or(GovernanceError::ProposalNotFound)?;
        
        proposal.cast_vote(voter, option, power, current_block)
    }

    /// Update all proposal statuses
    pub fn update_statuses(&mut self, current_block: u64) {
        for proposal in self.proposals.values_mut() {
            proposal.update_status(current_block, self.total_voting_power);
        }
    }

    /// Get active proposals
    pub fn active_proposals(&self) -> Vec<&Proposal> {
        self.proposals.values()
            .filter(|p| p.status == ProposalStatus::Active)
            .collect()
    }
}

impl Default for GovernanceManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_voting_power_calculation() {
        let mut position = VoterPosition {
            balances: HashMap::new(),
            stake_starts: HashMap::new(),
        };
        
        // Add stake to D1 (Genesis)
        position.balances.insert(PoolType::Genesis, 1_000_000);
        position.stake_starts.insert(PoolType::Genesis, 0);
        
        // At block 0, unlock is minimal
        let power_0 = position.calculate_voting_power(0);
        
        // After 100k blocks (~1 year), unlock increases
        let power_100k = position.calculate_voting_power(100_000);
        
        assert!(power_100k > power_0);
    }

    #[test]
    fn test_proposal_thresholds() {
        assert_eq!(ProposalType::Parameter.approval_threshold(), 0.50);
        assert_eq!(ProposalType::Constitutional.approval_threshold(), 0.80);
        assert_eq!(ProposalType::Emergency.approval_threshold(), 0.90);
    }

    #[test]
    fn test_vote_casting() {
        let proposer = Address::from_bytes([0; 32]);
        let voter = Address::from_bytes([1; 32]);
        
        let mut proposal = Proposal::new(
            1,
            ProposalType::Parameter,
            "Test".to_string(),
            "Description".to_string(),
            proposer,
            0,
        );
        
        proposal.status = ProposalStatus::Active;
        proposal.voting_starts = 0;
        proposal.voting_ends = 100_000;
        
        // Cast vote
        let result = proposal.cast_vote(voter, VoteOption::For, 1000, 50);
        assert!(result.is_ok());
        assert_eq!(proposal.votes_for, 1000);
        
        // Can't vote twice
        let result2 = proposal.cast_vote(voter, VoteOption::Against, 500, 60);
        assert_eq!(result2, Err(GovernanceError::AlreadyVoted));
    }

    #[test]
    fn test_proposal_execute_parameter_change() {
        let proposer = Address::from_bytes([0; 32]);

        let action = ProposalAction::ChangeParameter {
            key: "mempool.max_tx_per_block".into(),
            old_value: "1000".into(),
            new_value: "2000".into(),
        };

        let mut proposal = Proposal::new_with_action(
            1,
            ProposalType::Parameter,
            "Increase block tx limit".to_string(),
            "Double the max transactions per block".to_string(),
            proposer,
            0,
            Some(action),
        );

        // Force into Passed state with elapsed timelock
        proposal.status = ProposalStatus::Passed;
        proposal.execution_at = 0; // already elapsed

        let receipt = proposal.execute(100, true, "applied".into()).unwrap();
        assert_eq!(receipt.proposal_id, 1);
        assert!(receipt.success);
        assert_eq!(proposal.status, ProposalStatus::Executed);
    }

    #[test]
    fn test_proposal_execute_requires_passed_status() {
        let proposer = Address::from_bytes([0; 32]);

        let action = ProposalAction::ChangeParameter {
            key: "k".into(),
            old_value: "1".into(),
            new_value: "2".into(),
        };

        let mut proposal = Proposal::new_with_action(
            2,
            ProposalType::Parameter,
            "T".to_string(),
            "D".to_string(),
            proposer,
            0,
            Some(action),
        );

        // Active — cannot execute
        proposal.status = ProposalStatus::Active;
        proposal.execution_at = 0;
        assert_eq!(
            proposal.execute(100, true, "".into()).unwrap_err(),
            ExecutionError::NotPassed,
        );
    }

    #[test]
    fn test_proposal_execute_timelock_not_expired() {
        let proposer = Address::from_bytes([0; 32]);

        let action = ProposalAction::ChangeParameter {
            key: "k".into(),
            old_value: "1".into(),
            new_value: "2".into(),
        };

        let mut proposal = Proposal::new_with_action(
            3,
            ProposalType::Parameter,
            "T".to_string(),
            "D".to_string(),
            proposer,
            0,
            Some(action),
        );

        proposal.status = ProposalStatus::Passed;
        proposal.execution_at = 50_000;

        // Block 100 — timelock has not elapsed
        assert_eq!(
            proposal.execute(100, true, "".into()).unwrap_err(),
            ExecutionError::TimelockNotExpired,
        );
    }

    #[test]
    fn test_protocol_upgrade_proposal() {
        let proposer = Address::from_bytes([0; 32]);

        let action = ProposalAction::ProtocolUpgrade {
            target_version: 3,
            activation_height: 500_000,
            description: "Enable encrypted transport (Noise XX)".into(),
        };

        let proposal = Proposal::new_with_action(
            4,
            ProposalType::Upgrade,
            "Protocol V3 Upgrade".to_string(),
            "Activate encrypted peer connections".to_string(),
            proposer,
            0,
            Some(action),
        );

        assert!(proposal.action.is_some());
        assert_eq!(proposal.status, ProposalStatus::Pending);
    }
}

