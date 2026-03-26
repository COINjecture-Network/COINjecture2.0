// Problem Submission Marketplace with Privacy Support
// Users submit NP-hard problems with escrowed bounties
// Supports both public and private (commitment-based) submissions

use coinject_core::{
    unix_now_secs_i64, Address, Balance, Hash, ProblemReveal, Solution, SubmissionMode,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Problem submission with bounty
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProblemSubmission {
    /// Unique problem ID
    pub problem_id: Hash,

    /// Two-mode submission: Public (full problem visible) or Private (commitment only)
    pub submission_mode: SubmissionMode,

    /// Optional reveal for private bounties
    pub problem_reveal: Option<ProblemReveal>,

    /// Submitter's address
    pub submitter: Address,
    /// Bounty amount (escrowed)
    pub bounty: Balance,
    /// Minimum work score required
    pub min_work_score: f64,
    /// Submission timestamp
    pub submitted_at: i64,
    /// Expiration timestamp
    pub expires_at: i64,
    /// Current status
    pub status: ProblemStatus,
    /// Solution (if solved)
    pub solution: Option<Solution>,
    /// Solver's address (if solved)
    pub solver: Option<Address>,
}

/// Problem status in marketplace
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProblemStatus {
    /// Open for submissions
    Open,
    /// Solution submitted, pending verification
    PendingVerification,
    /// Solved and verified
    Solved,
    /// Expired without solution
    Expired,
    /// Cancelled by submitter
    Cancelled,
}

/// Solution submission for marketplace problem
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SolutionSubmission {
    pub problem_id: Hash,
    pub solver: Address,
    pub solution: Solution,
    pub submitted_at: i64,
}

/// Problem marketplace managing user-submitted problems
pub struct ProblemMarketplace {
    /// Active problem submissions by ID
    problems: HashMap<Hash, ProblemSubmission>,
    /// Problems by submitter
    submitter_index: HashMap<Address, Vec<Hash>>,
    /// Escrowed balances (problem_id -> bounty amount)
    escrow: HashMap<Hash, Balance>,
}

impl ProblemMarketplace {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        ProblemMarketplace {
            problems: HashMap::new(),
            submitter_index: HashMap::new(),
            escrow: HashMap::new(),
        }
    }

    /// Submit a public problem with bounty (backward compatible helper)
    /// For private problems, use `submit_problem()` directly with `SubmissionMode::Private`
    pub fn submit_public_problem(
        &mut self,
        problem: coinject_core::ProblemType,
        submitter: Address,
        bounty: Balance,
        min_work_score: f64,
        expiration_days: u64,
    ) -> Result<Hash, MarketplaceError> {
        let mode = SubmissionMode::Public { problem };
        self.submit_problem(mode, submitter, bounty, min_work_score, expiration_days)
    }

    /// Submit a new problem with bounty
    /// Supports both Public (full problem visible) and Private (commitment only) modes
    pub fn submit_problem(
        &mut self,
        mode: SubmissionMode,
        submitter: Address,
        bounty: Balance,
        min_work_score: f64,
        expiration_days: u64,
    ) -> Result<Hash, MarketplaceError> {
        // Validate bounty
        if bounty == 0 {
            return Err(MarketplaceError::InvalidBounty);
        }

        if min_work_score <= 0.0 {
            return Err(MarketplaceError::InvalidWorkScore);
        }

        // Validate mode-specific requirements
        match &mode {
            SubmissionMode::Public { problem: _ } => {
                // Public mode: Current validation logic
                // No additional checks needed
            }
            SubmissionMode::Private {
                problem_commitment,
                zk_wellformed_proof,
                public_params,
            } => {
                // Private mode: Verify ZK proof
                if !zk_wellformed_proof.verify(problem_commitment, public_params) {
                    return Err(MarketplaceError::InvalidProof);
                }

                // Verify complexity estimate matches min_work_score
                if public_params.complexity_estimate < min_work_score {
                    return Err(MarketplaceError::InvalidParameters);
                }
            }
        }

        // Generate problem_id from mode
        let problem_id = match &mode {
            SubmissionMode::Public { problem } => {
                let problem_data = bincode::serialize(problem)
                    .map_err(|_| MarketplaceError::SerializationError)?;
                Hash::new(&problem_data)
            }
            SubmissionMode::Private {
                problem_commitment, ..
            } => *problem_commitment,
        };

        // Check for duplicates
        if self.problems.contains_key(&problem_id) {
            return Err(MarketplaceError::DuplicateProblem);
        }

        let now = unix_now_secs_i64();

        let expires_at = now + (expiration_days as i64 * 86400);

        let submission = ProblemSubmission {
            problem_id,
            submission_mode: mode,
            problem_reveal: None,
            submitter,
            bounty,
            min_work_score,
            submitted_at: now,
            expires_at,
            status: ProblemStatus::Open,
            solution: None,
            solver: None,
        };

        // Escrow the bounty
        self.escrow.insert(problem_id, bounty);

        // Store submission
        self.problems.insert(problem_id, submission);

        // Index by submitter
        self.submitter_index
            .entry(submitter)
            .or_default()
            .push(problem_id);

        println!(
            "Problem submitted: {:?} with bounty {} (expires in {} days)",
            problem_id, bounty, expiration_days
        );

        Ok(problem_id)
    }

    /// Submit solution for a problem
    /// Handles both public and private (revealed) submissions
    pub fn submit_solution(
        &mut self,
        problem_id: Hash,
        solver: Address,
        solution: Solution,
    ) -> Result<(), MarketplaceError> {
        let problem = self
            .problems
            .get_mut(&problem_id)
            .ok_or(MarketplaceError::ProblemNotFound)?;

        // Check status
        if problem.status != ProblemStatus::Open {
            return Err(MarketplaceError::ProblemNotOpen);
        }

        // Check expiration
        let now = unix_now_secs_i64();

        if now > problem.expires_at {
            problem.status = ProblemStatus::Expired;
            return Err(MarketplaceError::ProblemExpired);
        }

        // Get problem for verification
        let problem_type = match &problem.submission_mode {
            SubmissionMode::Public { problem } => problem,
            SubmissionMode::Private { .. } => {
                // Require problem to be revealed before accepting solutions
                problem
                    .problem_reveal
                    .as_ref()
                    .map(|r| &r.problem)
                    .ok_or(MarketplaceError::ProblemNotRevealed)?
            }
        };

        // Verify solution against revealed problem
        if !solution.verify(problem_type) {
            return Err(MarketplaceError::InvalidSolution);
        }

        // Calculate quality score
        let quality = solution.quality(problem_type);
        let work_score_estimate = quality * 100.0; // Simplified

        if work_score_estimate < problem.min_work_score {
            return Err(MarketplaceError::InsufficientWorkScore);
        }

        // Update problem
        problem.solution = Some(solution);
        problem.solver = Some(solver);
        problem.status = ProblemStatus::Solved;

        println!(
            "Solution accepted for problem {:?} by {:?}",
            problem_id, solver
        );

        Ok(())
    }

    /// Reveal problem for private bounty
    /// Allows submitter to reveal the actual problem after solution or expiration
    pub fn reveal_problem(
        &mut self,
        problem_id: Hash,
        reveal: ProblemReveal,
    ) -> Result<(), MarketplaceError> {
        let problem = self
            .problems
            .get_mut(&problem_id)
            .ok_or(MarketplaceError::ProblemNotFound)?;

        // Verify this is a private submission
        let commitment = match &problem.submission_mode {
            SubmissionMode::Private {
                problem_commitment, ..
            } => problem_commitment,
            SubmissionMode::Public { .. } => {
                return Err(MarketplaceError::NotPrivateSubmission);
            }
        };

        // Verify reveal matches commitment
        if !reveal.verify(commitment) {
            return Err(MarketplaceError::RevealMismatch);
        }

        // Store reveal
        problem.problem_reveal = Some(reveal);

        println!("Problem revealed: {:?}", problem_id);

        Ok(())
    }

    /// Claim bounty after solution verification
    pub fn claim_bounty(
        &mut self,
        problem_id: Hash,
    ) -> Result<(Address, Balance), MarketplaceError> {
        let problem = self
            .problems
            .get(&problem_id)
            .ok_or(MarketplaceError::ProblemNotFound)?;

        // Check status
        if problem.status != ProblemStatus::Solved {
            return Err(MarketplaceError::ProblemNotSolved);
        }

        let solver = problem.solver.ok_or(MarketplaceError::NoSolver)?;
        let bounty = self
            .escrow
            .remove(&problem_id)
            .ok_or(MarketplaceError::BountyNotFound)?;

        println!("Bounty claimed: {} tokens for solver {:?}", bounty, solver);

        Ok((solver, bounty))
    }

    /// Cancel problem and refund bounty
    pub fn cancel_problem(
        &mut self,
        problem_id: Hash,
        requester: Address,
    ) -> Result<Balance, MarketplaceError> {
        let problem = self
            .problems
            .get_mut(&problem_id)
            .ok_or(MarketplaceError::ProblemNotFound)?;

        // Only submitter can cancel
        if problem.submitter != requester {
            return Err(MarketplaceError::Unauthorized);
        }

        // Can only cancel open problems
        if problem.status != ProblemStatus::Open {
            return Err(MarketplaceError::CannotCancel);
        }

        // Refund bounty
        let bounty = self
            .escrow
            .remove(&problem_id)
            .ok_or(MarketplaceError::BountyNotFound)?;

        problem.status = ProblemStatus::Cancelled;

        println!(
            "Problem cancelled: {:?}, refunding {} tokens",
            problem_id, bounty
        );

        Ok(bounty)
    }

    /// Get problem by ID
    pub fn get_problem(&self, problem_id: &Hash) -> Option<&ProblemSubmission> {
        self.problems.get(problem_id)
    }

    /// Get all open problems
    pub fn get_open_problems(&self) -> Vec<&ProblemSubmission> {
        self.problems
            .values()
            .filter(|p| p.status == ProblemStatus::Open)
            .collect()
    }

    /// Get problems by submitter
    pub fn get_problems_by_submitter(&self, submitter: &Address) -> Vec<&ProblemSubmission> {
        self.submitter_index
            .get(submitter)
            .map(|ids| ids.iter().filter_map(|id| self.problems.get(id)).collect())
            .unwrap_or_default()
    }

    /// Expire old problems and refund bounties
    pub fn expire_old_problems(&mut self) -> Vec<(Address, Balance)> {
        let now = unix_now_secs_i64();

        let mut refunds = Vec::new();

        for (problem_id, problem) in self.problems.iter_mut() {
            if problem.status == ProblemStatus::Open && now > problem.expires_at {
                if let Some(bounty) = self.escrow.remove(problem_id) {
                    refunds.push((problem.submitter, bounty));
                    problem.status = ProblemStatus::Expired;
                    println!(
                        "Problem expired: {:?}, refunding {} tokens",
                        problem_id, bounty
                    );
                }
            }
        }

        refunds
    }

    /// Get marketplace statistics
    pub fn get_stats(&self) -> MarketplaceStats {
        let mut stats = MarketplaceStats {
            total_problems: self.problems.len(),
            open_problems: 0,
            solved_problems: 0,
            expired_problems: 0,
            cancelled_problems: 0,
            total_bounty_pool: 0,
        };

        for problem in self.problems.values() {
            match problem.status {
                ProblemStatus::Open => stats.open_problems += 1,
                ProblemStatus::Solved | ProblemStatus::PendingVerification => {
                    stats.solved_problems += 1
                }
                ProblemStatus::Expired => stats.expired_problems += 1,
                ProblemStatus::Cancelled => stats.cancelled_problems += 1,
            }
        }

        stats.total_bounty_pool = self.escrow.values().sum();

        stats
    }
}

/// Marketplace statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceStats {
    pub total_problems: usize,
    pub open_problems: usize,
    pub solved_problems: usize,
    pub expired_problems: usize,
    pub cancelled_problems: usize,
    pub total_bounty_pool: Balance,
}

/// Marketplace errors
#[derive(Debug, thiserror::Error)]
pub enum MarketplaceError {
    #[error("Invalid bounty amount")]
    InvalidBounty,
    #[error("Invalid work score requirement")]
    InvalidWorkScore,
    #[error("Duplicate problem submission")]
    DuplicateProblem,
    #[error("Problem not found")]
    ProblemNotFound,
    #[error("Problem is not open")]
    ProblemNotOpen,
    #[error("Problem expired")]
    ProblemExpired,
    #[error("Invalid solution")]
    InvalidSolution,
    #[error("Insufficient work score")]
    InsufficientWorkScore,
    #[error("Problem not solved")]
    ProblemNotSolved,
    #[error("No solver assigned")]
    NoSolver,
    #[error("Bounty not found in escrow")]
    BountyNotFound,
    #[error("Unauthorized action")]
    Unauthorized,
    #[error("Cannot cancel problem")]
    CannotCancel,
    #[error("Serialization error")]
    SerializationError,
    #[error("Invalid proof")]
    InvalidProof,
    #[error("Invalid parameters")]
    InvalidParameters,
    #[error("Problem not revealed yet")]
    ProblemNotRevealed,
    #[error("Reveal does not match commitment")]
    RevealMismatch,
    #[error("Not a private submission")]
    NotPrivateSubmission,
}

#[cfg(test)]
mod tests {
    use super::*;
    use coinject_core::{Address, Solution};

    #[test]
    fn test_submit_problem() {
        let mut marketplace = ProblemMarketplace::new();

        let problem = coinject_core::ProblemType::SubsetSum {
            numbers: vec![1, 2, 3, 4, 5],
            target: 9,
        };

        let submitter = Address::from_bytes([1u8; 32]);
        let bounty = 1000;

        let result = marketplace.submit_public_problem(problem, submitter, bounty, 10.0, 7);
        assert!(result.is_ok());

        let problem_id = result.unwrap();
        let submission = marketplace.get_problem(&problem_id);
        assert!(submission.is_some());
        assert_eq!(submission.unwrap().bounty, bounty);
    }

    #[test]
    fn test_submit_solution() {
        let mut marketplace = ProblemMarketplace::new();

        let problem = coinject_core::ProblemType::SubsetSum {
            numbers: vec![1, 2, 3, 4, 5],
            target: 9,
        };

        let submitter = Address::from_bytes([1u8; 32]);
        let problem_id = marketplace
            .submit_public_problem(problem, submitter, 1000, 1.0, 7)
            .unwrap();

        let solver = Address::from_bytes([2u8; 32]);
        let solution = Solution::SubsetSum(vec![1, 2, 3]); // 2 + 3 + 4 = 9

        let result = marketplace.submit_solution(problem_id, solver, solution);
        assert!(result.is_ok());

        let submission = marketplace.get_problem(&problem_id).unwrap();
        assert_eq!(submission.status, ProblemStatus::Solved);
        assert_eq!(submission.solver, Some(solver));
    }

    #[test]
    fn test_claim_bounty() {
        let mut marketplace = ProblemMarketplace::new();

        let problem = coinject_core::ProblemType::SubsetSum {
            numbers: vec![1, 2, 3, 4, 5],
            target: 9,
        };

        let submitter = Address::from_bytes([1u8; 32]);
        let problem_id = marketplace
            .submit_public_problem(problem, submitter, 1000, 1.0, 7)
            .unwrap();

        let solver = Address::from_bytes([2u8; 32]);
        let solution = Solution::SubsetSum(vec![1, 2, 3]);

        marketplace
            .submit_solution(problem_id, solver, solution)
            .unwrap();

        let (claimed_solver, bounty) = marketplace.claim_bounty(problem_id).unwrap();
        assert_eq!(claimed_solver, solver);
        assert_eq!(bounty, 1000);
    }

    #[test]
    fn test_cancel_problem() {
        let mut marketplace = ProblemMarketplace::new();

        let problem = coinject_core::ProblemType::SubsetSum {
            numbers: vec![1, 2, 3, 4, 5],
            target: 9,
        };

        let submitter = Address::from_bytes([1u8; 32]);
        let problem_id = marketplace
            .submit_public_problem(problem, submitter, 1000, 10.0, 7)
            .unwrap();

        let bounty = marketplace.cancel_problem(problem_id, submitter).unwrap();
        assert_eq!(bounty, 1000);

        let submission = marketplace.get_problem(&problem_id).unwrap();
        assert_eq!(submission.status, ProblemStatus::Cancelled);
    }

    #[test]
    fn test_marketplace_stats() {
        let mut marketplace = ProblemMarketplace::new();

        let problem1 = coinject_core::ProblemType::SubsetSum {
            numbers: vec![1, 2, 3],
            target: 6,
        };

        let problem2 = coinject_core::ProblemType::SubsetSum {
            numbers: vec![4, 5, 6],
            target: 11,
        };

        let submitter = Address::from_bytes([1u8; 32]);
        marketplace
            .submit_public_problem(problem1, submitter, 1000, 10.0, 7)
            .unwrap();
        marketplace
            .submit_public_problem(problem2, submitter, 2000, 10.0, 7)
            .unwrap();

        let stats = marketplace.get_stats();
        assert_eq!(stats.total_problems, 2);
        assert_eq!(stats.open_problems, 2);
        assert_eq!(stats.total_bounty_pool, 3000);
    }
}
