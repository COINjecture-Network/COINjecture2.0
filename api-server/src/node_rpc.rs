//! JSON-RPC client for communicating with the COINjecture node.
//!
//! The API server proxies peer management requests to the node's
//! existing JSON-RPC server (default: localhost:9933).

use bytes::Bytes;
use reqwest::Client;
use serde_json::{json, Value};
use std::fmt;
use std::time::Duration;

pub struct NodeRpcClient {
    url: String,
    http: Client,
    /// Longer timeout for browser-originated JSON-RPC forwarded through `POST /node-rpc`.
    http_proxy: Client,
}

#[derive(Debug)]
pub enum NodeRpcError {
    Unavailable(String),
    RequestFailed(String),
    InvalidResponse(String),
}

impl fmt::Display for NodeRpcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable(msg) => write!(f, "Node unavailable: {msg}"),
            Self::RequestFailed(msg) => write!(f, "RPC failed: {msg}"),
            Self::InvalidResponse(msg) => write!(f, "Invalid response: {msg}"),
        }
    }
}

impl NodeRpcClient {
    pub fn new(url: &str) -> Self {
        Self {
            url: url.trim_end_matches('/').to_string(),
            http: Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap_or_default(),
            // Block submission (`chain_submitBlock`) can be large + slow; browser → /node-rpc → node.
            http_proxy: Client::builder()
                .timeout(Duration::from_secs(300))
                .build()
                .unwrap_or_default(),
        }
    }

    /// Forward a raw JSON-RPC POST body to the node; returns upstream status + body bytes.
    pub async fn forward_jsonrpc_body(&self, body: Bytes) -> Result<(u16, Bytes), NodeRpcError> {
        let resp = self
            .http_proxy
            .post(&self.url)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body)
            .send()
            .await
            .map_err(|e| NodeRpcError::Unavailable(e.to_string()))?;

        let status = resp.status().as_u16();
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| NodeRpcError::RequestFailed(e.to_string()))?;
        Ok((status, bytes))
    }

    /// Send a JSON-RPC 2.0 request and return the result field.
    async fn call(&self, method: &str, params: Value) -> Result<Value, NodeRpcError> {
        let body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        });

        let resp = self
            .http
            .post(&self.url)
            .json(&body)
            .send()
            .await
            .map_err(|e| NodeRpcError::Unavailable(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(NodeRpcError::RequestFailed(format!(
                "HTTP {}",
                resp.status()
            )));
        }

        let data: Value = resp
            .json()
            .await
            .map_err(|e| NodeRpcError::InvalidResponse(e.to_string()))?;

        if let Some(error) = data.get("error") {
            return Err(NodeRpcError::RequestFailed(error.to_string()));
        }

        Ok(data.get("result").cloned().unwrap_or(Value::Null))
    }

    /// Get network info from the node.
    pub async fn get_network_info(&self) -> Result<Value, NodeRpcError> {
        self.call("network_getInfo", json!([])).await
    }

    /// Get chain info from the node.
    pub async fn get_chain_info(&self) -> Result<Value, NodeRpcError> {
        self.call("chain_getInfo", json!([])).await
    }

    /// Get the latest block from the node.
    pub async fn get_latest_block(&self) -> Result<Value, NodeRpcError> {
        self.call("chain_getLatestBlock", json!([])).await
    }

    /// Get a block by height.
    pub async fn get_block_by_height(&self, height: u64) -> Result<Value, NodeRpcError> {
        self.call("chain_getBlock", json!([height])).await
    }
}
