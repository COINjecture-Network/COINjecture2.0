// Metrics Collection
// Collects and calculates all performance metrics for Hugging Face Dataset

use crate::client::DatasetRecord;
use crate::energy::{EnergyConfig, EnergyMeasurer};
use crate::serialize::{serialize_problem, serialize_solution, extract_problem_from_submission};
use coinject_state::ProblemSubmission;
use coinject_consensus::WorkScoreCalculator;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use crate::SyncConfig;

/// Metrics collector
pub struct MetricsCollector {
    energy_measurer: EnergyMeasurer,
    work_calculator: WorkScoreCalculator,
}

impl MetricsCollector {
    /// Create new metrics collector
    pub fn new(energy_config: EnergyConfig) -> Self {
        MetricsCollector {
            energy_measurer: EnergyMeasurer::new(energy_config),
            work_calculator: WorkScoreCalculator::new(),
        }
    }

    /// Measure energy for solve and verify operations
    pub fn measure_energy(
        &self,
        solve_time: Duration,
        verify_time: Duration,
    ) -> Result<(f64, f64), MetricsError> {
        let measurement = self.energy_measurer.measure_solve_verify_energy(solve_time, verify_time)?;
        Ok((measurement.solve_energy_joules, measurement.verify_energy_joules))
    }

    /// Collect problem record (when problem is submitted)
    pub fn collect_problem_record(
        &self,
        submission: &ProblemSubmission,
        block_height: u64,
        config: &SyncConfig,
    ) -> Result<DatasetRecord, MetricsError> {
        let problem = extract_problem_from_submission(
            &submission.submission_mode,
            submission.problem_reveal.as_ref(),
        )?;

        let problem_data = if let Some(ref p) = problem {
            serialize_problem(p)?
        } else {
            serde_json::json!({}) // Private problem not revealed
        };

        let problem_type = match &submission.submission_mode {
            coinject_core::SubmissionMode::Public { problem } => {
                format!("{:?}", problem).split('{').next().unwrap_or("Unknown").to_string()
            }
            coinject_core::SubmissionMode::Private { .. } => "Private".to_string(),
        };

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        Ok(DatasetRecord {
            problem_id: hex::encode(submission.problem_id.as_bytes()),
            problem_type,
            problem_data,
            problem_complexity: submission.min_work_score, // Using min_work_score as complexity proxy
            bounty: submission.bounty,
            submitter: if config.include_submitter_address {
                Some(hex::encode(submission.submitter.as_bytes()))
            } else {
                None
            },
            solver: None,
            solution_data: None,
            time_asymmetry: None,
            space_asymmetry: None,
            solve_energy_joules: None,
            verify_energy_joules: None,
            total_energy_joules: None,
            energy_per_operation: None,
            energy_asymmetry: None,
            energy_efficiency: None,
            solution_quality: None,
            work_score: None,
            block_height,
            timestamp,
            status: format!("{:?}", submission.status),
            energy_measurement_method: format!("{:?}", self.energy_measurer.config.method),
            submission_mode: match &submission.submission_mode {
                coinject_core::SubmissionMode::Public { .. } => "public".to_string(),
                coinject_core::SubmissionMode::Private { .. } => "private".to_string(),
            },
        })
    }

    /// Collect solution record (when solution is submitted)
    pub fn collect_solution_record(
        &self,
        submission: &ProblemSubmission,
        block_height: u64,
        solve_time: Duration,
        verify_time: Duration,
        solve_memory: usize,
        verify_memory: usize,
        solve_energy: f64,
        verify_energy: f64,
        config: &SyncConfig,
    ) -> Result<DatasetRecord, MetricsError> {
        // Extract problem
        let problem = extract_problem_from_submission(
            &submission.submission_mode,
            submission.problem_reveal.as_ref(),
        )?;

        let problem_data = if let Some(ref p) = problem {
            serialize_problem(p)?
        } else {
            serde_json::json!({})
        };

        let problem_type = match &submission.submission_mode {
            coinject_core::SubmissionMode::Public { problem } => {
                format!("{:?}", problem).split('{').next().unwrap_or("Unknown").to_string()
            }
            coinject_core::SubmissionMode::Private { .. } => "Private".to_string(),
        };

        // Serialize solution
        let solution_data = submission.solution.as_ref()
            .map(|s| serialize_solution(s))
            .transpose()?;

        // Calculate metrics
        let time_asymmetry = solve_time.as_secs_f64() / verify_time.as_secs_f64().max(0.001);
        let space_asymmetry = (solve_memory as f64 / verify_memory as f64).sqrt();
        let total_energy = solve_energy + verify_energy;
        let energy_asymmetry = if verify_energy > 0.0 {
            solve_energy / verify_energy
        } else {
            0.0
        };

        // Estimate energy per operation (simplified)
        let operations_estimate = solve_time.as_secs_f64() * 1_000_000_000.0; // Rough estimate
        let energy_per_operation = if operations_estimate > 0.0 {
            solve_energy / operations_estimate
        } else {
            0.0
        };

        let energy_efficiency = 1.0 / (energy_per_operation + 1.0);

        // Calculate solution quality and work score if we have the problem
        let (solution_quality, work_score) = if let (Some(ref problem), Some(ref solution)) = (problem.as_ref(), submission.solution.as_ref()) {
            let quality = solution.quality(problem);
            let work_score = self.work_calculator.calculate(
                problem,
                solution,
                solve_time,
                verify_time,
                solve_memory,
                verify_memory,
                energy_per_operation,
            );
            (Some(quality), Some(work_score))
        } else {
            (None, None)
        };

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        Ok(DatasetRecord {
            problem_id: hex::encode(submission.problem_id.as_bytes()),
            problem_type,
            problem_data,
            problem_complexity: problem.as_ref().map(|p| p.difficulty_weight()).unwrap_or(submission.min_work_score),
            bounty: submission.bounty,
            submitter: if config.include_submitter_address {
                Some(hex::encode(submission.submitter.as_bytes()))
            } else {
                None
            },
            solver: if config.include_solver_address {
                submission.solver.map(|s| hex::encode(s.as_bytes()))
            } else {
                None
            },
            solution_data,
            time_asymmetry: Some(time_asymmetry),
            space_asymmetry: Some(space_asymmetry),
            solve_energy_joules: Some(solve_energy),
            verify_energy_joules: Some(verify_energy),
            total_energy_joules: Some(total_energy),
            energy_per_operation: Some(energy_per_operation),
            energy_asymmetry: Some(energy_asymmetry),
            energy_efficiency: Some(energy_efficiency),
            solution_quality,
            work_score,
            block_height,
            timestamp,
            status: format!("{:?}", submission.status),
            energy_measurement_method: format!("{:?}", self.energy_measurer.config.method),
            submission_mode: match &submission.submission_mode {
                coinject_core::SubmissionMode::Public { .. } => "public".to_string(),
                coinject_core::SubmissionMode::Private { .. } => "private".to_string(),
            },
        })
    }

    /// Collect consensus block record (for mined or validated blocks)
    pub fn collect_consensus_block_record(
        &self,
        block: &coinject_core::Block,
        is_mined: bool,
    ) -> Result<DatasetRecord, MetricsError> {
        use coinject_core::Transaction;

        // Extract consensus block data
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Serialize all transactions with their details
        let transactions_json: Vec<serde_json::Value> = block.transactions
            .iter()
            .map(|tx| {
                // Serialize transaction to JSON
                serde_json::to_value(tx).unwrap_or_else(|_| {
                    serde_json::json!({
                        "error": "Failed to serialize transaction",
                        "hash": hex::encode(tx.hash().as_bytes())
                    })
                })
            })
            .collect();

        // Extract marketplace transactions (problem/solution submissions)
        let mut marketplace_problems = Vec::new();
        let mut marketplace_solutions = Vec::new();
        
        for tx in &block.transactions {
            if let Transaction::Marketplace(marketplace_tx) = tx {
                match &marketplace_tx.operation {
                    coinject_core::MarketplaceOperation::SubmitProblem { problem, bounty, min_work_score, expiration_days } => {
                        let problem_json = serialize_problem(problem)
                            .unwrap_or_else(|_| serde_json::json!({}));
                        marketplace_problems.push(serde_json::json!({
                            "problem": problem_json,
                            "bounty": bounty,
                            "min_work_score": min_work_score,
                            "expiration_days": expiration_days,
                            "submitter": hex::encode(marketplace_tx.from.as_bytes()),
                            "tx_hash": hex::encode(tx.hash().as_bytes()),
                        }));
                    }
                    coinject_core::MarketplaceOperation::SubmitSolution { problem_id, solution } => {
                        let solution_json = serialize_solution(solution)
                            .unwrap_or_else(|_| serde_json::json!({}));
                        marketplace_solutions.push(serde_json::json!({
                            "problem_id": hex::encode(problem_id.as_bytes()),
                            "solution": solution_json,
                            "solver": hex::encode(marketplace_tx.from.as_bytes()),
                            "tx_hash": hex::encode(tx.hash().as_bytes()),
                        }));
                    }
                    _ => {} // ClaimBounty, CancelProblem don't need special handling
                }
            }
        }

        // Serialize solution reveal
        let solution_reveal_json = serde_json::json!({
            "problem": serialize_problem(&block.solution_reveal.problem).unwrap_or_else(|_| serde_json::json!({})),
            "solution": serialize_solution(&block.solution_reveal.solution).unwrap_or_else(|_| serde_json::json!({})),
            "commitment_hash": hex::encode(block.solution_reveal.commitment.hash.as_bytes()),
            "problem_hash": hex::encode(block.solution_reveal.commitment.problem_hash.as_bytes()),
        });

        // Calculate time asymmetry from PoUW metrics (if available)
        let time_asymmetry = if block.header.verify_time_ms > 0 {
            Some(block.header.solve_time_ms as f64 / block.header.verify_time_ms as f64)
        } else {
            Some(block.header.time_asymmetry_ratio)
        };

        // Calculate energy asymmetry
        let energy_asymmetry = if block.header.energy_estimate_joules > 0.0 {
            // For consensus blocks, we use the estimated energy
            // Energy asymmetry would be solve_energy / verify_energy, but we only have total estimate
            // We can estimate based on time asymmetry
            time_asymmetry.map(|ta| ta * 0.1) // Rough estimate: verify is ~10% of solve energy
        } else {
            None
        };

        // Build comprehensive problem_data with ALL block information
        let problem_data = serde_json::json!({
            // Block header - all fields
            "version": block.header.version,
            "height": block.header.height,
            "prev_hash": hex::encode(block.header.prev_hash.as_bytes()),
            "timestamp": block.header.timestamp,
            "transactions_root": hex::encode(block.header.transactions_root.as_bytes()),
            "solutions_root": hex::encode(block.header.solutions_root.as_bytes()),
            "commitment": {
                "hash": hex::encode(block.header.commitment.hash.as_bytes()),
                "problem_hash": hex::encode(block.header.commitment.problem_hash.as_bytes()),
            },
            "work_score": block.header.work_score,
            "miner": hex::encode(block.header.miner.as_bytes()),
            "nonce": block.header.nonce,
            
            // PoUW Transparency Metrics (WEB4)
            "solve_time_ms": block.header.solve_time_ms,
            "verify_time_ms": block.header.verify_time_ms,
            "time_asymmetry_ratio": block.header.time_asymmetry_ratio,
            "solution_quality": block.header.solution_quality,
            "complexity_weight": block.header.complexity_weight,
            "energy_estimate_joules": block.header.energy_estimate_joules,
            
            // Coinbase transaction
            "coinbase": {
                "reward": block.coinbase.reward,
                "height": block.coinbase.height,
                "to": hex::encode(block.coinbase.to.as_bytes()),
            },
            
            // All transactions with full details
            "transactions": transactions_json,
            "transactions_count": block.transactions.len(),
            
            // Marketplace data extracted from transactions
            "marketplace_problems": marketplace_problems,
            "marketplace_solutions": marketplace_solutions,
            
            // Solution reveal
            "solution_reveal": solution_reveal_json,
        });

        // Build comprehensive solution_data
        let solution_data = serde_json::json!({
            "hash": hex::encode(block.hash().as_bytes()),
            "header_hash": hex::encode(block.header.hash().as_bytes()),
            "timestamp": block.header.timestamp,
            "total_fees": block.total_fees(),
        });

        Ok(DatasetRecord {
            problem_id: format!("consensus_block_{}", block.header.height),
            problem_type: "ConsensusBlock".to_string(),
            problem_data,
            problem_complexity: block.header.work_score,
            bounty: block.coinbase.reward,
            submitter: Some(hex::encode(block.header.miner.as_bytes())),
            solver: if is_mined { Some(hex::encode(block.header.miner.as_bytes())) } else { None },
            solution_data: Some(solution_data),
            // Use PoUW metrics from block header
            time_asymmetry,
            space_asymmetry: None, // Not available in block header
            solve_energy_joules: if block.header.energy_estimate_joules > 0.0 {
                // Estimate solve energy as 90% of total (verify is fast)
                Some(block.header.energy_estimate_joules * 0.9)
            } else {
                None
            },
            verify_energy_joules: if block.header.energy_estimate_joules > 0.0 {
                // Estimate verify energy as 10% of total
                Some(block.header.energy_estimate_joules * 0.1)
            } else {
                None
            },
            total_energy_joules: if block.header.energy_estimate_joules > 0.0 {
                Some(block.header.energy_estimate_joules)
            } else {
                None
            },
            energy_per_operation: None, // Would need operation count
            energy_asymmetry,
            energy_efficiency: if block.header.energy_estimate_joules > 0.0 {
                // Efficiency = work_score / energy
                Some(block.header.work_score / block.header.energy_estimate_joules)
            } else {
                None
            },
            solution_quality: Some(block.header.solution_quality),
            work_score: Some(block.header.work_score),
            block_height: block.header.height,
            timestamp,
            status: if is_mined { "Mined".to_string() } else { "Validated".to_string() },
            energy_measurement_method: format!("{:?}", self.energy_measurer.config.method),
            submission_mode: "consensus".to_string(),
        })
    }
}

/// Metrics collection errors
#[derive(Debug, thiserror::Error)]
pub enum MetricsError {
    #[error("Serialization error: {0}")]
    Serialization(#[from] crate::serialize::SerializationError),
    #[error("Energy measurement error: {0}")]
    Energy(#[from] crate::energy::EnergyError),
    #[error("Problem extraction error: {0}")]
    ProblemExtraction(String),
}

