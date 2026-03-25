// =============================================================================
// Multi-Dimensional Governance
// voting_power = Σ(balance_n × D_n × unlock_n(τ))
// =============================================================================
// Rewards long-term holders across multiple dimensions

use crate::pools::PoolType;
use crate::staking::delta_critical;
use coinject_core::Address;
use coinject_core::ETA; // Import from core (single source of truth)
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Proposal threshold: total_supply × Δ_critical = 23.1%
pub fn proposal_threshold_pct() -> f64 {
    delta_critical()
}

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
            ProposalType::Parameter => 10_000,       // ~1 day
            ProposalType::Treasury => 20_000,        // ~2 days
            ProposalType::Upgrade => 50_000,         // ~5 days
            ProposalType::Constitutional => 100_000, // ~10 days
            ProposalType::Emergency => 0,            // Immediate
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
        let power: f64 = self
            .balances
            .iter()
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
        let start = self
            .stake_starts
            .get(&pool_type)
            .copied()
            .unwrap_or(current_block);
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
    /// Create new proposal
    pub fn new(
        id: u64,
        proposal_type: ProposalType,
        title: String,
        description: String,
        proposer: Address,
        current_block: u64,
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
        }
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

        self.votes.insert(
            voter,
            Vote {
                voter,
                option,
                power,
                cast_at: current_block,
            },
        );

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

    /// Create new proposal
    pub fn create_proposal(
        &mut self,
        proposal_type: ProposalType,
        title: String,
        description: String,
        proposer: Address,
        current_block: u64,
    ) -> Result<u64, GovernanceError> {
        // Check proposer has enough voting power
        let proposer_power = self.get_voting_power(&proposer, current_block);
        if proposer_power < self.proposal_threshold() {
            return Err(GovernanceError::InsufficientPower);
        }

        let id = self.next_id;
        self.next_id += 1;

        let proposal = Proposal::new(
            id,
            proposal_type,
            title,
            description,
            proposer,
            current_block,
        );

        self.proposals.insert(id, proposal);
        Ok(id)
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

        let proposal = self
            .proposals
            .get_mut(&proposal_id)
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
        self.proposals
            .values()
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
}
