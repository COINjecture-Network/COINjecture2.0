// Hugging Face API Client
// Handles communication with Hugging Face Dataset API

use serde::{Deserialize, Serialize};
use serde_json::Value;
use base64::{Engine as _, engine::general_purpose::STANDARD};

/// Hugging Face client configuration
#[derive(Debug, Clone)]
pub struct HuggingFaceConfig {
    pub token: String,
    pub dataset_name: String,
    pub dataset_config: Option<String>,
    pub api_base: String,
}

impl Default for HuggingFaceConfig {
    fn default() -> Self {
        HuggingFaceConfig {
            token: String::new(),
            dataset_name: String::new(),
            dataset_config: None,
            api_base: "https://huggingface.co/api".to_string(),
        }
    }
}

/// Hugging Face API client
pub struct HuggingFaceClient {
    config: HuggingFaceConfig,
    client: reqwest::Client,
    buffer: Vec<Value>,
    buffer_size: usize,
}

/// Dataset record structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetRecord {
    pub problem_id: String,
    pub problem_type: String,
    pub problem_data: Value,
    pub problem_complexity: f64,
    pub bounty: u128,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub submitter: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub solver: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub solution_data: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_asymmetry: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub space_asymmetry: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub solve_energy_joules: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verify_energy_joules: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_energy_joules: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub energy_per_operation: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub energy_asymmetry: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub energy_efficiency: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub solution_quality: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub work_score: Option<f64>,
    pub block_height: u64,
    pub timestamp: i64,
    pub status: String,
    pub energy_measurement_method: String,
    pub submission_mode: String,
}

impl HuggingFaceClient {
    /// Create new Hugging Face client
    pub fn new(config: HuggingFaceConfig) -> Result<Self, ClientError> {
        if config.token.is_empty() {
            return Err(ClientError::InvalidConfig("Hugging Face token is required".to_string()));
        }
        if config.dataset_name.is_empty() {
            return Err(ClientError::InvalidConfig("Dataset name is required".to_string()));
        }

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| ClientError::HttpClient(e.to_string()))?;

        Ok(HuggingFaceClient {
            config,
            client,
            buffer: Vec::new(),
            buffer_size: 10,
        })
    }

    /// Push a single record (buffered, flushed when buffer is full)
    pub async fn push_record(&mut self, record: DatasetRecord) -> Result<(), ClientError> {
        let value = serde_json::to_value(&record)
            .map_err(|e| ClientError::Serialization(e.to_string()))?;

        self.buffer.push(value);
        eprintln!("📊 Hugging Face: Buffered record (buffer size: {}/{})", self.buffer.len(), self.buffer_size);

        if self.buffer.len() >= self.buffer_size {
            eprintln!("📤 Hugging Face: Buffer full, flushing {} records...", self.buffer.len());
            self.flush().await?;
        }

        Ok(())
    }

    /// Flush buffered records to Hugging Face
    pub async fn flush(&mut self) -> Result<(), ClientError> {
        if self.buffer.is_empty() {
            return Ok(());
        }

        // Push data to Hugging Face using HTTP API
        tracing::info!(
            "Pushing {} records to Hugging Face dataset: {}",
            self.buffer.len(),
            self.config.dataset_name
        );

        // Use Hugging Face Hub API commit endpoint to upload dataset data
        // The new Hub API uses: POST https://huggingface.co/api/datasets/{repo_id}/commit/{revision}
        // The old /upload endpoint is deprecated
        let repo_id = &self.config.dataset_name;
        
        // Create JSONL content (one JSON object per line) - this is the standard format for datasets
        let jsonl_content: String = self.buffer
            .iter()
            .map(|record| serde_json::to_string(record).unwrap_or_default())
            .collect::<Vec<_>>()
            .join("\n");
        
        // Generate filename with timestamp to avoid conflicts
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let filename = format!("data_{}.jsonl", timestamp);
        let path_in_repo = format!("data/{}", filename);
        
        // Base64 encode the content for the commit API
        let content_base64 = STANDARD.encode(jsonl_content.as_bytes());
        
        // Hub API commit endpoint for uploading files to datasets
        // Format: POST {api_base}/datasets/{repo_id}/commit/main
        let url = format!("{}/datasets/{}/commit/main", self.config.api_base, repo_id);
        
        eprintln!("📤 Hugging Face: Uploading {} records as {} to dataset {}", self.buffer.len(), path_in_repo, repo_id);
        eprintln!("   URL: {}", url);
        eprintln!("   Content length: {} bytes (base64: {} bytes)", jsonl_content.len(), content_base64.len());

        // Create NDJSON payload (newline-delimited JSON)
        // Line 1: Commit header with metadata
        // Line 2+: File operations
        let commit_message = format!("Add {} consensus block records", self.buffer.len());
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

        // Build NDJSON payload (each JSON object on its own line)
        let ndjson_payload = format!(
            "{}\n{}",
            serde_json::to_string(&header_line).unwrap(),
            serde_json::to_string(&file_operation).unwrap()
        );

        eprintln!("📤 Hugging Face: NDJSON payload size: {} bytes", ndjson_payload.len());

        // Make HTTP request with authentication
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

        tracing::info!("Successfully pushed {} records to Hugging Face", self.buffer.len());
        eprintln!("✅ Hugging Face: Successfully pushed {} records to dataset {}", self.buffer.len(), self.config.dataset_name);
        self.buffer.clear();
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

