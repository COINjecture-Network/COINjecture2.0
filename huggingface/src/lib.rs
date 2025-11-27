// Hugging Face Dataset Integration
// Real-time sync of marketplace problem and solution data to Hugging Face

pub mod client;
pub mod energy;
pub mod metrics;
pub mod serialize;

pub use client::{HuggingFaceClient, HuggingFaceConfig};
pub use energy::{EnergyMeasurement, EnergyMeasurementMethod, EnergyConfig};
pub use metrics::MetricsCollector;
pub use serialize::{serialize_problem, serialize_solution};

use coinject_state::ProblemSubmission;
use std::time::Duration;

/// Main Hugging Face sync service
pub struct HuggingFaceSync {
    client: tokio::sync::Mutex<HuggingFaceClient>,
    metrics_collector: tokio::sync::Mutex<MetricsCollector>,
    config: SyncConfig,
}

/// Configuration for Hugging Face sync
#[derive(Debug, Clone)]
pub struct SyncConfig {
    pub enabled: bool,
    pub include_submitter_address: bool,
    pub include_solver_address: bool,
    pub batch_size: usize,
    pub batch_interval: Duration,
    /// Flush buffer after this many unique blocks (default: 50)
    pub flush_interval_blocks: usize,
}

impl Default for SyncConfig {
    fn default() -> Self {
        SyncConfig {
            enabled: true,
            include_submitter_address: true,
            include_solver_address: true,
            batch_size: 10,
            batch_interval: Duration::from_secs(5),
            flush_interval_blocks: 50,
        }
    }
}

impl HuggingFaceSync {
    /// Create new Hugging Face sync service
    pub fn new(
        hf_config: HuggingFaceConfig,
        energy_config: energy::EnergyConfig,
        sync_config: SyncConfig,
    ) -> Result<Self, SyncError> {
        let mut client = HuggingFaceClient::new(hf_config)?;
        // Apply configured flush interval
        client.set_flush_interval_blocks(sync_config.flush_interval_blocks as u64);
        let metrics_collector = MetricsCollector::new(energy_config);

        Ok(HuggingFaceSync {
            client: tokio::sync::Mutex::new(client),
            metrics_collector: tokio::sync::Mutex::new(metrics_collector),
            config: sync_config,
        })
    }

    /// Push problem submission to Hugging Face
    pub async fn push_problem_submission(
        &self,
        submission: &ProblemSubmission,
        block_height: u64,
    ) -> Result<(), SyncError> {
        if !self.config.enabled {
            return Ok(());
        }

        let collector = self.metrics_collector.lock().await;
        let record = collector.collect_problem_record(
            submission,
            block_height,
            &self.config,
        )?;
        drop(collector);

        let mut client = self.client.lock().await;
        client.push_record(record).await?;
        Ok(())
    }

    /// Push solution submission to Hugging Face
    pub async fn push_solution_submission(
        &self,
        submission: &ProblemSubmission,
        block_height: u64,
        solve_time: Duration,
        verify_time: Duration,
        solve_memory: usize,
        verify_memory: usize,
    ) -> Result<(), SyncError> {
        if !self.config.enabled {
            return Ok(());
        }

        // Measure energy during verification
        let mut collector = self.metrics_collector.lock().await;
        let (solve_energy, verify_energy) = collector.measure_energy(solve_time, verify_time)?;

        let record = collector.collect_solution_record(
            submission,
            block_height,
            solve_time,
            verify_time,
            solve_memory,
            verify_memory,
            solve_energy,
            verify_energy,
            &self.config,
        )?;
        drop(collector);

        let mut client = self.client.lock().await;
        client.push_record(record).await?;
        Ok(())
    }

    /// Push consensus block to Hugging Face (for mined or validated blocks)
    pub async fn push_consensus_block(
        &self,
        block: &coinject_core::Block,
        is_mined: bool,
    ) -> Result<(), SyncError> {
        if !self.config.enabled {
            println!("⚠️  Hugging Face sync is disabled");
            return Ok(());
        }

        eprintln!("📦 Hugging Face: Collecting consensus block data for block {} (mined: {})", block.header.height, is_mined);
        let collector = self.metrics_collector.lock().await;
        let record = match collector.collect_consensus_block_record(block, is_mined) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("❌ Hugging Face: Failed to collect consensus block record: {}", e);
                return Err(e.into());
            }
        };
        drop(collector);

        eprintln!("📦 Hugging Face: Record collected, pushing to buffer...");
        let mut client = self.client.lock().await;
        match client.push_record(record).await {
            Ok(()) => {
                eprintln!("✅ Hugging Face: Record pushed to buffer successfully");
                Ok(())
            }
            Err(e) => {
                eprintln!("❌ Hugging Face: Failed to push record to buffer: {}", e);
                Err(e.into())
            }
        }
    }

    /// Force flush any buffered records
    pub async fn flush(&self) -> Result<(), SyncError> {
        if !self.config.enabled {
            return Ok(());
        }

        let mut client = self.client.lock().await;
        client.force_flush().await?;
        Ok(())
    }
}

/// Sync errors
#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("Hugging Face client error: {0}")]
    Client(#[from] client::ClientError),
    #[error("Metrics collection error: {0}")]
    Metrics(#[from] metrics::MetricsError),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serialize::SerializationError),
    #[error("Energy measurement error: {0}")]
    Energy(#[from] energy::EnergyError),
}

