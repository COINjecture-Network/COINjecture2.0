// Hugging Face API Client
// Handles communication with Hugging Face Dataset API

use serde::{Deserialize, Serialize, Serializer, Deserializer};
use serde_json::Value;
use base64::{Engine as _, engine::general_purpose::STANDARD};

use std::collections::HashMap;

/// Serialize u128 as string to avoid JSON precision loss
fn serialize_u128_as_string<S>(value: &u128, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&value.to_string())
}

/// Deserialize u128 from string
fn deserialize_u128_from_string<'de, D>(deserializer: D) -> Result<u128, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    s.parse().map_err(serde::de::Error::custom)
}

/// Hugging Face client configuration
#[derive(Debug, Clone)]
pub struct HuggingFaceConfig {
    pub token: String,
    pub dataset_prefix: String, // e.g., "COINjecture" - will append problem type
    pub dataset_config: Option<String>,
    pub api_base: String,
}

impl Default for HuggingFaceConfig {
    fn default() -> Self {
        HuggingFaceConfig {
            token: String::new(),
            dataset_prefix: String::new(),
            dataset_config: None,
            api_base: "https://huggingface.co/api".to_string(),
        }
    }
}

/// Hugging Face API client with per-problem-type routing
pub struct HuggingFaceClient {
    config: HuggingFaceConfig,
    client: reqwest::Client,
    buffers: HashMap<String, Vec<Value>>, // problem_type -> buffer
    buffer_size: usize,
    blocks_since_flush: u64, // Track blocks since last flush for block-based flushing
    last_block_height: Option<u64>, // Track last block height we saw to detect new blocks
    flush_interval_blocks: u64, // Flush every N blocks
}

/// Dataset record structure - INSTITUTIONAL GRADE v3.0
/// Comprehensive metrics for academic research and transparency
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetRecord {
    // ═══════════════════════════════════════════════════════════════════════════
    // PRIMARY CONTENT - Problem and Solution (most important!)
    // ═══════════════════════════════════════════════════════════════════════════
    pub problem_id: String,
    pub problem_type: String,
    pub problem_data: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub solution_data: Option<Value>,

    // ═══════════════════════════════════════════════════════════════════════════
    // BLOCK IDENTITY - Unique identification and chain linkage
    // ═══════════════════════════════════════════════════════════════════════════
    pub block_height: u64,
    pub timestamp: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_hash: Option<String>,          // NEW: Hash of this block
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prev_block_hash: Option<String>,     // NEW: Hash of previous block (chain linkage)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub submitter: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub solver: Option<String>,

    // ═══════════════════════════════════════════════════════════════════════════
    // PERFORMANCE METRICS - Key results
    // ═══════════════════════════════════════════════════════════════════════════
    #[serde(skip_serializing_if = "Option::is_none")]
    pub work_score: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub solution_quality: Option<f64>,
    pub problem_complexity: f64,
    #[serde(serialize_with = "serialize_u128_as_string", deserialize_with = "deserialize_u128_from_string")]
    pub bounty: u128,

    // ═══════════════════════════════════════════════════════════════════════════
    // TIMING METRICS - Solve/verify performance (microsecond precision)
    // ═══════════════════════════════════════════════════════════════════════════
    #[serde(skip_serializing_if = "Option::is_none")]
    pub solve_time_us: Option<u64>,          // NEW: Solve time in microseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verify_time_us: Option<u64>,         // NEW: Verify time in microseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_time_seconds: Option<f64>,     // NEW: Time since previous block
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mining_attempts: Option<u64>,        // NEW: Nonce attempts before success

    // ═══════════════════════════════════════════════════════════════════════════
    // ASYMMETRY METRICS - NP-hardness verification (solve >> verify)
    // ═══════════════════════════════════════════════════════════════════════════
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_asymmetry: Option<f64>,         // solve_time / verify_time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub space_asymmetry: Option<f64>,        // solve_memory / verify_memory
    #[serde(skip_serializing_if = "Option::is_none")]
    pub energy_asymmetry: Option<f64>,       // solve_energy / verify_energy

    // ═══════════════════════════════════════════════════════════════════════════
    // MEMORY METRICS - Space complexity tracking
    // ═══════════════════════════════════════════════════════════════════════════
    #[serde(skip_serializing_if = "Option::is_none")]
    pub solve_memory_bytes: Option<u64>,     // NEW: Memory used during solve
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verify_memory_bytes: Option<u64>,    // NEW: Memory used during verify
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peak_memory_bytes: Option<u64>,      // NEW: Peak memory during block

    // ═══════════════════════════════════════════════════════════════════════════
    // ENERGY MEASUREMENTS - Power consumption tracking
    // ═══════════════════════════════════════════════════════════════════════════
    #[serde(skip_serializing_if = "Option::is_none")]
    pub solve_energy_joules: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verify_energy_joules: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_energy_joules: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub energy_per_operation: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub energy_efficiency: Option<f64>,

    // ═══════════════════════════════════════════════════════════════════════════
    // NETWORK METRICS - P2P network state at block time
    // ═══════════════════════════════════════════════════════════════════════════
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peer_count: Option<u32>,             // NEW: Connected peers when block received
    #[serde(skip_serializing_if = "Option::is_none")]
    pub propagation_time_ms: Option<u64>,    // NEW: Time to receive block from network
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sync_lag_blocks: Option<i64>,        // NEW: Blocks behind network tip

    // ═══════════════════════════════════════════════════════════════════════════
    // DIFFICULTY & MINING METRICS
    // ═══════════════════════════════════════════════════════════════════════════
    #[serde(skip_serializing_if = "Option::is_none")]
    pub difficulty_target: Option<u32>,      // NEW: Leading zeros required
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nonce: Option<u64>,                  // NEW: Winning nonce value
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash_rate_estimate: Option<f64>,     // NEW: Estimated H/s (nonce/solve_time)

    // ═══════════════════════════════════════════════════════════════════════════
    // CHAIN METRICS - Cumulative blockchain state
    // ═══════════════════════════════════════════════════════════════════════════
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chain_work: Option<f64>,             // NEW: Cumulative work score
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_count: Option<u32>,      // NEW: Transactions in this block
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_size_bytes: Option<u64>,       // NEW: Total block size

    // ═══════════════════════════════════════════════════════════════════════════
    // ECONOMIC METRICS - Tokenomics tracking
    // ═══════════════════════════════════════════════════════════════════════════
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_reward: Option<String>,        // NEW: Total coinbase reward (as string for u128)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_fees: Option<String>,          // NEW: Sum of transaction fees (as string)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pool_distributions: Option<Value>,   // NEW: Token distribution to each pool

    // ═══════════════════════════════════════════════════════════════════════════
    // HARDWARE METRICS - Mining infrastructure transparency
    // ═══════════════════════════════════════════════════════════════════════════
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_model: Option<String>,           // NEW: CPU model string
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_cores: Option<u32>,              // NEW: Number of CPU cores
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_threads: Option<u32>,            // NEW: Number of threads used
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ram_total_bytes: Option<u64>,        // NEW: Total system RAM
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os_info: Option<String>,             // NEW: Operating system info

    // ═══════════════════════════════════════════════════════════════════════════
    // METADATA & PROVENANCE
    // ═══════════════════════════════════════════════════════════════════════════
    pub status: String,
    pub submission_mode: String,
    pub energy_measurement_method: String,

    // INSTITUTIONAL-GRADE DATA PROVENANCE (v3.0)
    pub metrics_source: String,              // "block_header_actual", "node_measured", "estimated"
    pub measurement_confidence: String,      // "very_high", "high", "medium", "low"
    pub data_version: String,                // "v3.0" - institutional-grade comprehensive
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_version: Option<String>,        // NEW: Node software version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,             // NEW: PeerId of recording node
}

impl HuggingFaceClient {
    /// Create new Hugging Face client
    pub fn new(config: HuggingFaceConfig) -> Result<Self, ClientError> {
        if config.token.is_empty() {
            return Err(ClientError::InvalidConfig("Hugging Face token is required".to_string()));
        }
        if config.dataset_prefix.is_empty() {
            return Err(ClientError::InvalidConfig("Dataset prefix is required (e.g., 'COINjecture')".to_string()));
        }

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| ClientError::HttpClient(e.to_string()))?;

        Ok(HuggingFaceClient {
            config,
            client,
            buffers: HashMap::new(),
            buffer_size: 10,
            blocks_since_flush: 0,
            last_block_height: None,
            flush_interval_blocks: 10, // Flush every 10 blocks for near-real-time data streaming
        })
    }

    /// Push a single record (buffered, flushed every N blocks)
    /// Routes to problem-type-specific buffer
    pub async fn push_record(&mut self, record: DatasetRecord) -> Result<(), ClientError> {
        let problem_type = record.problem_type.clone();
        let block_height = record.block_height;
        let value = serde_json::to_value(&record)
            .map_err(|e| ClientError::Serialization(e.to_string()))?;

        // Get or create buffer for this problem type
        let buffer = self.buffers.entry(problem_type.clone()).or_insert_with(Vec::new);
        buffer.push(value);

        // Track blocks since last flush - increment only when we see a new block
        let is_new_block = match self.last_block_height {
            None => {
                // First record - initialize
                self.last_block_height = Some(block_height);
                self.blocks_since_flush = 1;
                true
            }
            Some(last_height) => {
                if block_height > last_height {
                    // New block detected - increment counter
                    self.last_block_height = Some(block_height);
                    self.blocks_since_flush += 1;
                    true
                } else {
                    // Same block or older - don't increment (might be multiple records per block)
                    false
                }
            }
        };

        let total_records: usize = self.buffers.values().map(|b| b.len()).sum();
        if is_new_block {
            eprintln!("📊 Hugging Face: Buffered {} record (block {}, {} blocks since flush, {} total records)",
                problem_type, block_height, self.blocks_since_flush, total_records);
        } else {
            eprintln!("📊 Hugging Face: Buffered {} record (block {}, {} total records)",
                problem_type, block_height, total_records);
        }

        // Check if we should flush based on block count
        if self.blocks_since_flush >= self.flush_interval_blocks {
            eprintln!("📤 Hugging Face: Flush interval reached ({} blocks), flushing all buffered records...", self.blocks_since_flush);
            self.flush().await?;
            // Note: flush() will reset blocks_since_flush, but we also reset last_block_height here
            self.last_block_height = None;
        }

        Ok(())
    }

    /// Set the flush interval in blocks
    pub fn set_flush_interval_blocks(&mut self, interval: u64) {
        self.flush_interval_blocks = interval;
    }

    /// Flush a specific problem type's buffer to its corresponding dataset
    async fn flush_problem_type(&mut self, problem_type: &str) -> Result<(), ClientError> {
        // Get the buffer for this problem type
        let buffer = match self.buffers.get_mut(problem_type) {
            Some(buf) if !buf.is_empty() => buf,
            _ => return Ok(()), // No buffer or empty buffer
        };

        // Use unified dataset name directly (single continuous dataset for all problem types)
        // If dataset_prefix contains "/", use it as full dataset name, otherwise append problem type
        let dataset_name = if self.config.dataset_prefix.contains('/') {
            // Full dataset name provided (e.g., "COINjecture/NP_Solutions")
            self.config.dataset_prefix.clone()
        } else {
            // Legacy: prefix only, append problem type
            format!("{}/{}_Solutions", self.config.dataset_prefix, problem_type.trim())
        };

        tracing::info!(
            "Pushing {} {} records to Hugging Face dataset: {}",
            buffer.len(),
            problem_type,
            dataset_name
        );

        // Create JSONL content
        let jsonl_content: String = buffer
            .iter()
            .map(|record| serde_json::to_string(record).unwrap_or_default())
            .collect::<Vec<_>>()
            .join("\n");

        // Generate filename with timestamp
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let filename = format!("data_{}.jsonl", timestamp);
        let path_in_repo = format!("data/{}", filename);

        // Base64 encode the content
        let content_base64 = STANDARD.encode(jsonl_content.as_bytes());

        // Hub API commit endpoint
        let url = format!("{}/datasets/{}/commit/main", self.config.api_base, dataset_name);

        eprintln!("📤 Hugging Face: Uploading {} {} records as {} to dataset {}",
            buffer.len(), problem_type, path_in_repo, dataset_name);
        eprintln!("   URL: {}", url);
        eprintln!("   Content length: {} bytes (base64: {} bytes)", jsonl_content.len(), content_base64.len());

        // Create NDJSON payload
        let commit_message = format!("Add {} {} records", buffer.len(), problem_type);
        let header_line = serde_json::json!({
            "key": "header",
            "value": {"summary": commit_message}
        });
        let file_operation = serde_json::json!({
            "key": "file",
            "value": {
                "content": content_base64,
                "path": path_in_repo,
                "encoding": "base64"
            }
        });

        let ndjson_payload = format!(
            "{}\n{}",
            serde_json::to_string(&header_line).unwrap(),
            serde_json::to_string(&file_operation).unwrap()
        );

        eprintln!("📤 Hugging Face: NDJSON payload size: {} bytes", ndjson_payload.len());

        // Make HTTP request
        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.token))
            .header("Content-Type", "application/x-ndjson")
            .body(ndjson_payload)
            .send()
            .await
            .map_err(|e| {
                eprintln!("❌ Hugging Face network error: {}", e);
                ClientError::Network(e.to_string())
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            eprintln!("❌ Hugging Face API error: HTTP {} - {}", status, error_text);
            return Err(ClientError::Api(format!(
                "HTTP {}: {}",
                status,
                error_text
            )));
        }

        tracing::info!("Successfully pushed {} {} records to Hugging Face", buffer.len(), problem_type);
        eprintln!("✅ Hugging Face: Successfully pushed {} {} records to dataset {}",
            buffer.len(), problem_type, dataset_name);

        // Clear the buffer after successful upload
        buffer.clear();
        Ok(())
    }

    /// Flush all buffered records to Hugging Face (all problem types)
    pub async fn flush(&mut self) -> Result<(), ClientError> {
        if self.buffers.is_empty() {
            return Ok(());
        }

        // Reset block counter and last block height when flushing
        self.blocks_since_flush = 0;
        self.last_block_height = None;

        // Check if using unified dataset (dataset_prefix contains "/")
        let use_unified = self.config.dataset_prefix.contains('/');
        
        if use_unified {
            // Combine all problem types into one unified dataset
            let mut all_records = Vec::new();
            let mut total_count = 0;
            
            // Collect all records from all buffers
            for (problem_type, buffer) in &self.buffers {
                total_count += buffer.len();
                for record in buffer {
                    all_records.push(record.clone());
                }
            }
            
            if all_records.is_empty() {
                return Ok(());
            }
            
            // Use unified dataset name
            let dataset_name = self.config.dataset_prefix.clone();
            
            // Create JSONL content from all records
            let jsonl_content: String = all_records
                .iter()
                .map(|record| serde_json::to_string(record).unwrap_or_default())
                .collect::<Vec<_>>()
                .join("\n");
            
            // Generate filename with timestamp
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            let filename = format!("data_{}.jsonl", timestamp);
            let path_in_repo = format!("data/{}", filename);
            
            // Base64 encode the content
            let content_base64 = STANDARD.encode(jsonl_content.as_bytes());
            
            // Hub API commit endpoint
            let url = format!("{}/datasets/{}/commit/main", self.config.api_base, dataset_name);
            
            eprintln!("📤 Hugging Face: Uploading {} total records (all problem types) as {} to unified dataset {}",
                total_count, path_in_repo, dataset_name);
            
            // Create NDJSON payload
            let commit_message = format!("Add {} records (unified dataset)", total_count);
            let header_line = serde_json::json!({
                "key": "header",
                "value": {"summary": commit_message}
            });
            let file_operation = serde_json::json!({
                "key": "file",
                "value": {
                    "content": content_base64,
                    "path": path_in_repo,
                    "encoding": "base64"
                }
            });
            
            let ndjson_payload = format!(
                "{}\n{}",
                serde_json::to_string(&header_line).unwrap(),
                serde_json::to_string(&file_operation).unwrap()
            );
            
            // Make HTTP request
            let response = self.client
                .post(&url)
                .header("Authorization", format!("Bearer {}", self.config.token))
                .header("Content-Type", "application/x-ndjson")
                .body(ndjson_payload)
                .send()
                .await
                .map_err(|e| {
                    eprintln!("❌ Hugging Face network error: {}", e);
                    ClientError::Network(e.to_string())
                })?;
            
            if !response.status().is_success() {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                eprintln!("❌ Hugging Face API error: HTTP {} - {}", status, error_text);
                return Err(ClientError::Api(format!(
                    "HTTP {}: {}",
                    status,
                    error_text
                )));
            }
            
            eprintln!("✅ Hugging Face: Successfully pushed {} total records to unified dataset {}",
                total_count, dataset_name);
            
            // Clear all buffers after successful upload
            self.buffers.clear();
        } else {
            // Legacy: Flush each problem type to separate datasets
            let problem_types: Vec<String> = self.buffers.keys().cloned().collect();
            for problem_type in problem_types {
                self.flush_problem_type(&problem_type).await?;
            }
        }

        Ok(())
    }

    /// Force flush any remaining records
    pub async fn force_flush(&mut self) -> Result<(), ClientError> {
        self.flush().await
    }
}

/// Client errors
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
    #[error("HTTP client error: {0}")]
    HttpClient(String),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("API error: {0}")]
    Api(String),
    #[error("Network error: {0}")]
    Network(String),
}

