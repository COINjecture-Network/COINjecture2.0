// JSON-RPC client for Network B
// Provides a clean interface for wallet to communicate with the blockchain node

use anyhow::{anyhow, Result};
use coinject_core::{Balance, Block, BlockHeader};
use coinject_rpc::{AccountInfo, ChainInfo, ProblemInfo, TransactionStatus};
use serde::{Deserialize, Serialize};
use serde_json::json;

/// JSON-RPC request structure
#[derive(Debug, Serialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    method: String,
    params: serde_json::Value,
    id: u64,
}

/// JSON-RPC response structure
#[derive(Debug, Deserialize)]
struct JsonRpcResponse<T> {
    jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
    id: u64,
}

/// JSON-RPC error structure
#[derive(Debug, Deserialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

/// RPC client for interacting with Network B node
pub struct RpcClient {
    url: String,
    client: reqwest::Client,
    next_id: std::sync::atomic::AtomicU64,
}

impl RpcClient {
    /// Create a new RPC client
    pub fn new(url: &str) -> Self {
        RpcClient {
            url: url.to_string(),
            client: reqwest::Client::new(),
            next_id: std::sync::atomic::AtomicU64::new(1),
        }
    }

    /// Make a JSON-RPC call
    async fn call<T: for<'de> Deserialize<'de>>(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<T> {
        let id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
            id,
        };

        let response = self
            .client
            .post(&self.url)
            .json(&request)
            .send()
            .await
            .map_err(|e| anyhow!("Failed to send RPC request: {}", e))?;

        let response: JsonRpcResponse<T> = response
            .json()
            .await
            .map_err(|e| anyhow!("Failed to parse RPC response: {}", e))?;

        if let Some(error) = response.error {
            return Err(anyhow!(
                "RPC error {}: {}",
                error.code,
                error.message
            ));
        }

        response
            .result
            .ok_or_else(|| anyhow!("RPC response missing result"))
    }

    // ========================================
    // Account methods
    // ========================================

    /// Get account balance
    pub async fn get_balance(&self, address: &str) -> Result<Balance> {
        self.call("account_getBalance", json!([address])).await
    }

    /// Get account nonce
    pub async fn get_nonce(&self, address: &str) -> Result<u64> {
        self.call("account_getNonce", json!([address])).await
    }

    /// Get account information
    pub async fn get_account_info(&self, address: &str) -> Result<AccountInfo> {
        self.call("account_getInfo", json!([address])).await
    }

    // ========================================
    // Transaction methods
    // ========================================

    /// Submit a transaction
    pub async fn submit_transaction(&self, tx_hex: &str) -> Result<String> {
        self.call("transaction_submit", json!([tx_hex])).await
    }

    /// Get transaction status
    pub async fn get_transaction_status(&self, tx_hash: &str) -> Result<TransactionStatus> {
        self.call("transaction_getStatus", json!([tx_hash]))
            .await
    }

    // ========================================
    // Chain methods
    // ========================================

    /// Get block by height
    pub async fn get_block(&self, height: u64) -> Result<Option<Block>> {
        self.call("chain_getBlock", json!([height])).await
    }

    /// Get latest block
    pub async fn get_latest_block(&self) -> Result<Option<Block>> {
        self.call("chain_getLatestBlock", json!([])).await
    }

    /// Get block header by height
    pub async fn get_block_header(&self, height: u64) -> Result<Option<BlockHeader>> {
        self.call("chain_getBlockHeader", json!([height])).await
    }

    /// Get chain information
    pub async fn get_chain_info(&self) -> Result<ChainInfo> {
        self.call("chain_getInfo", json!([])).await
    }

    // ========================================
    // Marketplace methods
    // ========================================

    /// Get open problems from marketplace
    pub async fn get_open_problems(&self) -> Result<Vec<ProblemInfo>> {
        self.call("marketplace_getOpenProblems", json!([])).await
    }

    /// Get problem by ID
    pub async fn get_problem(&self, problem_id: &str) -> Result<Option<ProblemInfo>> {
        self.call("marketplace_getProblem", json!([problem_id]))
            .await
    }

    /// Get marketplace statistics
    pub async fn get_marketplace_stats(&self) -> Result<serde_json::Value> {
        self.call("marketplace_getStats", json!([])).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = RpcClient::new("http://127.0.0.1:9944");
        assert_eq!(client.url, "http://127.0.0.1:9944");
    }
}
