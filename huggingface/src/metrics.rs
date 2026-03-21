// Metrics Collection - INSTITUTIONAL GRADE v3.0
// Comprehensive metrics collection for academic research and transparency
// Collects block, network, hardware, and economic metrics

use crate::client::DatasetRecord;
use crate::energy::{EnergyConfig, EnergyMeasurer};
use crate::serialize::{serialize_problem, serialize_solution, extract_problem_from_submission};
use coinject_state::ProblemSubmission;
use coinject_consensus::WorkScoreCalculator;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use crate::SyncConfig;
use sysinfo::{System, RefreshKind, CpuRefreshKind, MemoryRefreshKind};

/// Network context for institutional-grade metrics
#[derive(Debug, Clone, Default)]
pub struct NetworkContext {
    pub peer_count: u32,
    pub sync_lag_blocks: i64,
    pub propagation_time_ms: Option<u64>,
}

/// Hardware context for institutional-grade metrics  
#[derive(Debug, Clone, Default)]
pub struct HardwareContext {
    pub cpu_model: Option<String>,
    pub cpu_cores: u32,
    pub cpu_threads: u32,
    pub ram_total_bytes: u64,
    pub os_info: Option<String>,
}

impl HardwareContext {
    /// Detect hardware context from system
    pub fn detect() -> Self {
        let mut sys = System::new_with_specifics(
            RefreshKind::new()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything())
        );
        sys.refresh_all();
        
        let cpu_model = sys.cpus().first().map(|c| c.brand().to_string());
        let cpu_cores = sys.physical_core_count().unwrap_or(0) as u32;
        let cpu_threads = sys.cpus().len() as u32;
        let ram_total_bytes = sys.total_memory();
        let os_info = Some(format!("{} {}", System::name().unwrap_or_default(), System::os_version().unwrap_or_default()));
        
        HardwareContext {
            cpu_model,
            cpu_cores,
            cpu_threads,
            ram_total_bytes,
            os_info,
        }
    }
}

/// Metrics collector with institutional-grade context
pub struct MetricsCollector {
    energy_measurer: EnergyMeasurer,
    work_calculator: WorkScoreCalculator,
    hardware_context: HardwareContext,
    node_version: String,
    node_id: Option<String>,
}

impl MetricsCollector {
    /// Create new metrics collector with hardware detection
    pub fn new(energy_config: EnergyConfig) -> Self {
        let hardware_context = HardwareContext::detect();
        tracing::info!("📊 Hardware detected: {} cores, {} threads, {:.1} GB RAM",
            hardware_context.cpu_cores,
            hardware_context.cpu_threads,
            hardware_context.ram_total_bytes as f64 / 1_073_741_824.0
        );
        
        MetricsCollector {
            energy_measurer: EnergyMeasurer::new(energy_config),
            work_calculator: WorkScoreCalculator::new(),
            hardware_context,
            node_version: env!("CARGO_PKG_VERSION").to_string(),
            node_id: None,
        }
    }
    
    /// Set the node's PeerId for attribution
    pub fn set_node_id(&mut self, peer_id: String) {
        self.node_id = Some(peer_id);
    }

    /// Measure energy for solve and verify operations
    pub fn measure_energy(
        &mut self,
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
            // Primary content
            problem_id: hex::encode(submission.problem_id.as_bytes()),
            problem_type,
            problem_data,
            solution_data: None,

            // Block identity
            block_height,
            timestamp,
            block_hash: None,
            prev_block_hash: None,
            submitter: if config.include_submitter_address {
                Some(hex::encode(submission.submitter.as_bytes()))
            } else {
                None
            },
            solver: None,

            // Performance metrics
            work_score: None,
            solution_quality: None,
            problem_complexity: submission.min_work_score,
            bounty: submission.bounty,

            // Timing metrics
            solve_time_us: None,
            verify_time_us: None,
            block_time_seconds: None,
            mining_attempts: None,

            // Asymmetry metrics
            time_asymmetry: None,
            space_asymmetry: None,
            energy_asymmetry: None,

            // Memory metrics
            solve_memory_bytes: None,
            verify_memory_bytes: None,
            peak_memory_bytes: None,

            // Energy metrics
            solve_energy_joules: None,
            verify_energy_joules: None,
            total_energy_joules: None,
            energy_per_operation: None,
            energy_efficiency: None,

            // Network metrics
            peer_count: None,
            propagation_time_ms: None,
            sync_lag_blocks: None,

            // Difficulty & mining
            difficulty_target: None,
            nonce: None,
            hash_rate_estimate: None,

            // Chain metrics
            chain_work: None,
            transaction_count: None,
            block_size_bytes: None,

            // Economic metrics
            block_reward: None,
            total_fees: None,
            pool_distributions: None,

            // Hardware metrics
            cpu_model: self.hardware_context.cpu_model.clone(),
            cpu_cores: Some(self.hardware_context.cpu_cores),
            cpu_threads: Some(self.hardware_context.cpu_threads),
            ram_total_bytes: Some(self.hardware_context.ram_total_bytes),
            os_info: self.hardware_context.os_info.clone(),

            // Metadata
            status: format!("{:?}", submission.status),
            submission_mode: match &submission.submission_mode {
                coinject_core::SubmissionMode::Public { .. } => "public".to_string(),
                coinject_core::SubmissionMode::Private { .. } => "private".to_string(),
            },
            energy_measurement_method: format!("{:?}", self.energy_measurer.config.method),

            // Data provenance (v3.0)
            metrics_source: "not_applicable".to_string(),
            measurement_confidence: "not_applicable".to_string(),
            data_version: "v3.0".to_string(),
            node_version: Some(self.node_version.clone()),
            node_id: self.node_id.clone(),
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
            let work_score = self.work_calculator.calculate_from_solution(
                problem,
                solution,
                solve_time,
                verify_time,
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
            // Primary content
            problem_id: hex::encode(submission.problem_id.as_bytes()),
            problem_type,
            problem_data,
            solution_data,

            // Block identity
            block_height,
            timestamp,
            block_hash: None,
            prev_block_hash: None,
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

            // Performance metrics
            work_score,
            solution_quality,
            problem_complexity: problem.as_ref().map(|p| p.difficulty_weight()).unwrap_or(submission.min_work_score),
            bounty: submission.bounty,

            // Timing metrics
            solve_time_us: Some(solve_time.as_micros() as u64),
            verify_time_us: Some(verify_time.as_micros() as u64),
            block_time_seconds: None,
            mining_attempts: None,

            // Asymmetry metrics
            time_asymmetry: Some(time_asymmetry),
            space_asymmetry: Some(space_asymmetry),
            energy_asymmetry: Some(energy_asymmetry),

            // Memory metrics
            solve_memory_bytes: Some(solve_memory as u64),
            verify_memory_bytes: Some(verify_memory as u64),
            peak_memory_bytes: None,

            // Energy metrics
            solve_energy_joules: Some(solve_energy),
            verify_energy_joules: Some(verify_energy),
            total_energy_joules: Some(total_energy),
            energy_per_operation: Some(energy_per_operation),
            energy_efficiency: Some(energy_efficiency),

            // Network metrics
            peer_count: None,
            propagation_time_ms: None,
            sync_lag_blocks: None,

            // Difficulty & mining
            difficulty_target: None,
            nonce: None,
            hash_rate_estimate: None,

            // Chain metrics
            chain_work: None,
            transaction_count: None,
            block_size_bytes: None,

            // Economic metrics
            block_reward: None,
            total_fees: None,
            pool_distributions: None,

            // Hardware metrics
            cpu_model: self.hardware_context.cpu_model.clone(),
            cpu_cores: Some(self.hardware_context.cpu_cores),
            cpu_threads: Some(self.hardware_context.cpu_threads),
            ram_total_bytes: Some(self.hardware_context.ram_total_bytes),
            os_info: self.hardware_context.os_info.clone(),

            // Metadata
            status: format!("{:?}", submission.status),
            submission_mode: match &submission.submission_mode {
                coinject_core::SubmissionMode::Public { .. } => "public".to_string(),
                coinject_core::SubmissionMode::Private { .. } => "private".to_string(),
            },
            energy_measurement_method: format!("{:?}", self.energy_measurer.config.method),

            // Data provenance (v3.0)
            metrics_source: "measured_marketplace".to_string(),
            measurement_confidence: "medium".to_string(),
            data_version: "v3.0".to_string(),
            node_version: Some(self.node_version.clone()),
            node_id: self.node_id.clone(),
        })
    }

    /// Collect consensus block record (for mined or validated blocks)
    /// INSTITUTIONAL GRADE v3.0 - comprehensive metrics collection
    pub fn collect_consensus_block_record(
        &self,
        block: &coinject_core::Block,
        is_mined: bool,
    ) -> Result<DatasetRecord, MetricsError> {
        self.collect_consensus_block_record_with_context(block, is_mined, None)
    }
    
    /// Collect consensus block record with network context
    /// INSTITUTIONAL GRADE v3.0 - comprehensive metrics collection
    pub fn collect_consensus_block_record_with_context(
        &self,
        block: &coinject_core::Block,
        is_mined: bool,
        network_ctx: Option<&NetworkContext>,
    ) -> Result<DatasetRecord, MetricsError> {
        // Extract consensus block data
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Extract mining problem and solution from the block
        let problem = &block.solution_reveal.problem;
        let solution = &block.solution_reveal.solution;

        // Serialize the problem and solution
        let problem_data = serialize_problem(problem)?;
        let solution_data = serialize_solution(solution)?;

        // Determine problem type
        let problem_type = format!("{:?}", problem).split('{').next().unwrap_or("Unknown").to_string();

        // ═══════════════════════════════════════════════════════════════════════════
        // BLOCK IDENTITY METRICS
        // ═══════════════════════════════════════════════════════════════════════════
        let block_hash = Some(hex::encode(block.hash().as_bytes()));
        let prev_block_hash = Some(hex::encode(block.header.prev_hash.as_bytes()));

        // ═══════════════════════════════════════════════════════════════════════════
        // TIMING METRICS (microsecond precision from block header)
        // ═══════════════════════════════════════════════════════════════════════════
        let solve_time_us = block.header.solve_time_us;
        let verify_time_us = block.header.verify_time_us;
        let solve_time = Duration::from_micros(solve_time_us);
        let _verify_time = Duration::from_micros(verify_time_us);
        let time_asymmetry = block.header.time_asymmetry_ratio;

        // ═══════════════════════════════════════════════════════════════════════════
        // PERFORMANCE METRICS (from block header - institutional grade)
        // ═══════════════════════════════════════════════════════════════════════════
        let problem_complexity = block.header.complexity_weight;
        let solution_quality = block.header.solution_quality;
        let work_score = block.header.work_score;

        // Space asymmetry (estimated from time - TODO: add memory tracking to header)
        let space_asymmetry = time_asymmetry.sqrt();

        // ═══════════════════════════════════════════════════════════════════════════
        // ENERGY METRICS (from block header)
        // ═══════════════════════════════════════════════════════════════════════════
        let total_energy = block.header.energy_estimate_joules;
        
        // Energy split based on time asymmetry
        let solve_energy = total_energy * (time_asymmetry / (time_asymmetry + 1.0));
        let verify_energy = total_energy - solve_energy;

        let energy_asymmetry = if verify_energy > 0.0 {
            solve_energy / verify_energy
        } else {
            time_asymmetry
        };

        let operations_estimate = solve_time.as_secs_f64() * 1_000_000_000.0;
        let energy_per_operation = if operations_estimate > 0.0 {
            solve_energy / operations_estimate
        } else {
            0.0
        };
        let energy_efficiency = 1.0 / (energy_per_operation + 1.0);

        // ═══════════════════════════════════════════════════════════════════════════
        // DIFFICULTY & MINING METRICS
        // ═══════════════════════════════════════════════════════════════════════════
        let nonce = block.header.nonce;
        let hash_rate_estimate = if solve_time_us > 0 {
            Some((nonce as f64) / (solve_time_us as f64 / 1_000_000.0))
        } else {
            None
        };

        // ═══════════════════════════════════════════════════════════════════════════
        // CHAIN METRICS
        // ═══════════════════════════════════════════════════════════════════════════
        let transaction_count = block.transactions.len() as u32;
        let block_size_bytes = bincode::serialize(block).map(|b| b.len() as u64).ok();
        let block_reward = Some(block.coinbase.reward.to_string());
        let total_fees = Some(block.total_fees().to_string());

        // Pool distributions (would come from tokenomics if available)
        // Currently not stored in CoinbaseTransaction - set to None
        let pool_distributions: Option<serde_json::Value> = None;

        // ═══════════════════════════════════════════════════════════════════════════
        // NETWORK METRICS (from context if provided)
        // ═══════════════════════════════════════════════════════════════════════════
        let (peer_count, sync_lag_blocks, propagation_time_ms) = match network_ctx {
            Some(ctx) => (Some(ctx.peer_count), Some(ctx.sync_lag_blocks), ctx.propagation_time_ms),
            None => (None, None, None),
        };

        // ═══════════════════════════════════════════════════════════════════════════
        // HARDWARE METRICS (from detected context)
        // ═══════════════════════════════════════════════════════════════════════════
        let hw = &self.hardware_context;

        Ok(DatasetRecord {
            // Primary content
            problem_id: format!("mining_block_{}", block.header.height),
            problem_type,
            problem_data,
            solution_data: Some(solution_data),

            // Block identity
            block_height: block.header.height,
            timestamp,
            block_hash,
            prev_block_hash,
            submitter: Some(hex::encode(block.header.miner.as_bytes())),
            solver: if is_mined { Some(hex::encode(block.header.miner.as_bytes())) } else { None },

            // Performance metrics
            work_score: Some(work_score),
            solution_quality: Some(solution_quality),
            problem_complexity,
            bounty: block.coinbase.reward,

            // Timing metrics
            solve_time_us: Some(solve_time_us),
            verify_time_us: Some(verify_time_us),
            block_time_seconds: None, // Requires previous block timestamp
            mining_attempts: Some(nonce), // Nonce is proxy for attempts

            // Asymmetry metrics
            time_asymmetry: Some(time_asymmetry),
            space_asymmetry: Some(space_asymmetry),
            energy_asymmetry: Some(energy_asymmetry),

            // Memory metrics (TODO: implement actual tracking)
            solve_memory_bytes: None,
            verify_memory_bytes: None,
            peak_memory_bytes: None,

            // Energy metrics
            solve_energy_joules: Some(solve_energy),
            verify_energy_joules: Some(verify_energy),
            total_energy_joules: Some(total_energy),
            energy_per_operation: Some(energy_per_operation),
            energy_efficiency: Some(energy_efficiency),

            // Network metrics
            peer_count,
            propagation_time_ms,
            sync_lag_blocks,

            // Difficulty & mining
            difficulty_target: Some(4), // TODO: get from config
            nonce: Some(nonce),
            hash_rate_estimate,

            // Chain metrics
            chain_work: None, // Requires cumulative calculation
            transaction_count: Some(transaction_count),
            block_size_bytes,

            // Economic metrics
            block_reward,
            total_fees,
            pool_distributions,

            // Hardware metrics
            cpu_model: hw.cpu_model.clone(),
            cpu_cores: Some(hw.cpu_cores),
            cpu_threads: Some(hw.cpu_threads),
            ram_total_bytes: Some(hw.ram_total_bytes),
            os_info: hw.os_info.clone(),

            // Metadata
            status: if is_mined { "Mined".to_string() } else { "Validated".to_string() },
            submission_mode: "mining".to_string(),
            energy_measurement_method: format!("{:?}", self.energy_measurer.config.method),

            // Institutional-grade data provenance (v3.0)
            metrics_source: "block_header_actual".to_string(),
            measurement_confidence: "high".to_string(),
            data_version: "v3.0".to_string(),
            node_version: Some(self.node_version.clone()),
            node_id: self.node_id.clone(),
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

