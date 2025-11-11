// Marketplace State Management with Database Persistence
// Web4 PoUW marketplace integrated into blockchain state

use coinject_core::{Address, Balance, Hash, ProblemType, Solution};
use redb::{Database, ReadableTable, TableDefinition};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// Table definitions for marketplace state
const PROBLEMS_TABLE: TableDefinition<&[u8; 32], &[u8]> = TableDefinition::new("marketplace_problems");
const PROBLEM_INDEX_TABLE: TableDefinition<&[u8; 32], &[u8]> = TableDefinition::new("marketplace_index");
const ESCROW_TABLE: TableDefinition<&[u8; 32], u128> = TableDefinition::new("marketplace_escrow");

/// Problem status in marketplace
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProblemStatus {
    Open,
    Solved,
    Expired,
    Cancelled,
}

/// Problem submission with all metadata
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProblemSubmission {
    pub problem_id: Hash,
    pub problem: ProblemType,
    pub submitter: Address,
    pub bounty: Balance,
    pub min_work_score: f64,
    pub submitted_at: i64,
    pub expires_at: i64,
    pub status: ProblemStatus,
    pub solution: Option<Solution>,
    pub solver: Option<Address>,
}

/// Marketplace state with database persistence
pub struct MarketplaceState {
    db: Arc<Database>,
}

impl MarketplaceState {
    /// Create marketplace state from existing database
    pub fn from_db(db: Arc<Database>) -> Result<Self, MarketplaceError> {
        // Initialize marketplace tables
        let write_txn = db.begin_write()?;
        {
            let _ = write_txn.open_table(PROBLEMS_TABLE)?;
            let _ = write_txn.open_table(PROBLEM_INDEX_TABLE)?;
            let _ = write_txn.open_table(ESCROW_TABLE)?;
        }
        write_txn.commit()?;

        Ok(MarketplaceState { db })
    }

    /// Submit a new problem with bounty (escrow funds)
    pub fn submit_problem(
        &self,
        problem: ProblemType,
        submitter: Address,
        bounty: Balance,
        min_work_score: f64,
        expiration_days: u64,
    ) -> Result<Hash, MarketplaceError> {
        // Generate problem ID
        let problem_data = bincode::serialize(&problem)?;
        let problem_id = Hash::new(&problem_data);

        // Check for duplicates
        if self.problem_exists(&problem_id)? {
            return Err(MarketplaceError::DuplicateProblem);
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let expires_at = now + (expiration_days as i64 * 86400);

        let submission = ProblemSubmission {
            problem_id,
            problem,
            submitter,
            bounty,
            min_work_score,
            submitted_at: now,
            expires_at,
            status: ProblemStatus::Open,
            solution: None,
            solver: None,
        };

        // Serialize and store
        let submission_bytes = bincode::serialize(&submission)?;

        let write_txn = self.db.begin_write()?;
        {
            // Store problem
            let mut problems_table = write_txn.open_table(PROBLEMS_TABLE)?;
            problems_table.insert(problem_id.as_bytes(), submission_bytes.as_slice())?;

            // Store in index (by submitter)
            let index_key = Self::make_index_key(&submitter, &problem_id);
            let mut index_table = write_txn.open_table(PROBLEM_INDEX_TABLE)?;
            index_table.insert(&index_key, &problem_id.as_bytes()[..])?;

            // Escrow the bounty
            let mut escrow_table = write_txn.open_table(ESCROW_TABLE)?;
            escrow_table.insert(problem_id.as_bytes(), bounty)?;
        }
        write_txn.commit()?;

        Ok(problem_id)
    }

    /// Submit solution for a problem
    pub fn submit_solution(
        &self,
        problem_id: Hash,
        solver: Address,
        solution: Solution,
    ) -> Result<(), MarketplaceError> {
        let mut submission = self.get_problem(&problem_id)?
            .ok_or(MarketplaceError::ProblemNotFound)?;

        // Check status
        if submission.status != ProblemStatus::Open {
            return Err(MarketplaceError::ProblemNotOpen);
        }

        // Check expiration
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        if now > submission.expires_at {
            submission.status = ProblemStatus::Expired;
            self.update_problem(&submission)?;
            return Err(MarketplaceError::ProblemExpired);
        }

        // Verify solution
        if !solution.verify(&submission.problem) {
            return Err(MarketplaceError::InvalidSolution);
        }

        // Calculate quality score
        let quality = solution.quality(&submission.problem);
        let work_score_estimate = quality * 100.0; // Simplified

        if work_score_estimate < submission.min_work_score {
            return Err(MarketplaceError::InsufficientWorkScore);
        }

        // Update problem
        submission.solution = Some(solution);
        submission.solver = Some(solver);
        submission.status = ProblemStatus::Solved;

        self.update_problem(&submission)?;

        Ok(())
    }

    /// Claim bounty for solved problem
    pub fn claim_bounty(&self, problem_id: Hash) -> Result<(Address, Balance), MarketplaceError> {
        let submission = self.get_problem(&problem_id)?
            .ok_or(MarketplaceError::ProblemNotFound)?;

        if submission.status != ProblemStatus::Solved {
            return Err(MarketplaceError::ProblemNotSolved);
        }

        let solver = submission.solver.ok_or(MarketplaceError::NoSolver)?;

        // Release escrow
        let write_txn = self.db.begin_write()?;
        let bounty = {
            let mut escrow_table = write_txn.open_table(ESCROW_TABLE)?;
            let bounty = escrow_table.remove(problem_id.as_bytes())?
                .ok_or(MarketplaceError::BountyNotFound)?
                .value();
            bounty
        };
        write_txn.commit()?;

        Ok((solver, bounty))
    }

    /// Cancel problem and refund bounty
    pub fn cancel_problem(&self, problem_id: Hash, requester: Address) -> Result<Balance, MarketplaceError> {
        let mut submission = self.get_problem(&problem_id)?
            .ok_or(MarketplaceError::ProblemNotFound)?;

        // Only submitter can cancel
        if submission.submitter != requester {
            return Err(MarketplaceError::Unauthorized);
        }

        // Can only cancel open problems
        if submission.status != ProblemStatus::Open {
            return Err(MarketplaceError::CannotCancel);
        }

        // Refund bounty
        let write_txn = self.db.begin_write()?;
        let bounty = {
            let mut escrow_table = write_txn.open_table(ESCROW_TABLE)?;
            let bounty = escrow_table.remove(problem_id.as_bytes())?
                .ok_or(MarketplaceError::BountyNotFound)?
                .value();
            bounty
        };
        write_txn.commit()?;

        submission.status = ProblemStatus::Cancelled;
        self.update_problem(&submission)?;

        Ok(bounty)
    }

    /// Get problem by ID
    pub fn get_problem(&self, problem_id: &Hash) -> Result<Option<ProblemSubmission>, MarketplaceError> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(PROBLEMS_TABLE)?;

        match table.get(problem_id.as_bytes())? {
            Some(data) => {
                let submission: ProblemSubmission = bincode::deserialize(data.value())?;
                Ok(Some(submission))
            }
            None => Ok(None),
        }
    }

    /// Check if problem exists
    fn problem_exists(&self, problem_id: &Hash) -> Result<bool, MarketplaceError> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(PROBLEMS_TABLE)?;
        Ok(table.get(problem_id.as_bytes())?.is_some())
    }

    /// Update problem in database
    fn update_problem(&self, submission: &ProblemSubmission) -> Result<(), MarketplaceError> {
        let submission_bytes = bincode::serialize(submission)?;

        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(PROBLEMS_TABLE)?;
            table.insert(submission.problem_id.as_bytes(), submission_bytes.as_slice())?;
        }
        write_txn.commit()?;

        Ok(())
    }

    /// Make composite index key (address + problem_id)
    fn make_index_key(address: &Address, problem_id: &Hash) -> [u8; 32] {
        let mut key = [0u8; 32];
        // Use first 16 bytes of address, last 16 bytes of problem_id
        key[..16].copy_from_slice(&address.as_bytes()[..16]);
        key[16..].copy_from_slice(&problem_id.as_bytes()[..16]);
        key
    }

    /// Get all open problems (for mining selection)
    pub fn get_open_problems(&self) -> Result<Vec<ProblemSubmission>, MarketplaceError> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(PROBLEMS_TABLE)?;

        let mut problems = Vec::new();
        for item in table.iter()? {
            let (_, data) = item?;
            let submission: ProblemSubmission = bincode::deserialize(data.value())?;
            if submission.status == ProblemStatus::Open {
                problems.push(submission);
            }
        }

        Ok(problems)
    }

    /// Get marketplace statistics
    pub fn get_stats(&self) -> Result<MarketplaceStats, MarketplaceError> {
        let read_txn = self.db.begin_read()?;
        let problems_table = read_txn.open_table(PROBLEMS_TABLE)?;
        let escrow_table = read_txn.open_table(ESCROW_TABLE)?;

        let mut stats = MarketplaceStats {
            total_problems: 0,
            open_problems: 0,
            solved_problems: 0,
            expired_problems: 0,
            cancelled_problems: 0,
            total_bounty_pool: 0,
        };

        for item in problems_table.iter()? {
            let (_, data) = item?;
            let submission: ProblemSubmission = bincode::deserialize(data.value())?;
            stats.total_problems += 1;
            match submission.status {
                ProblemStatus::Open => stats.open_problems += 1,
                ProblemStatus::Solved => stats.solved_problems += 1,
                ProblemStatus::Expired => stats.expired_problems += 1,
                ProblemStatus::Cancelled => stats.cancelled_problems += 1,
            }
        }

        // Sum all escrowed bounties
        for item in escrow_table.iter()? {
            let (_, bounty) = item?;
            stats.total_bounty_pool += bounty.value();
        }

        Ok(stats)
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
    #[error("Duplicate problem")]
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
    #[error("Bounty not found")]
    BountyNotFound,
    #[error("Unauthorized")]
    Unauthorized,
    #[error("Cannot cancel problem")]
    CannotCancel,
    #[error("Database error: {0}")]
    Database(#[from] redb::Error),
    #[error("Database transaction error: {0}")]
    Transaction(#[from] redb::TransactionError),
    #[error("Database table error: {0}")]
    Table(#[from] redb::TableError),
    #[error("Database storage error: {0}")]
    Storage(#[from] redb::StorageError),
    #[error("Database commit error: {0}")]
    Commit(#[from] redb::CommitError),
    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),
}
