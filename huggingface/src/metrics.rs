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

