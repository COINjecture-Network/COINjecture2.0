//! JSON-RPC client for communicating with the COINjecture node.
//!
//! The API server proxies peer management requests to the node's
//! existing JSON-RPC server (default: localhost:9933).

use bytes::Bytes;
use reqwest::Client;
use serde_json::{json, Value};
use std::fmt;
use std::time::Duration;
use tokio::time::sleep;

/// `POST /node-rpc` returns the first upstream HTTP response. If that body is JSON-RPC `error`
/// for `chain_getMiningWork` on a follower (mining disabled), try the next `NODE_RPC_URL` so
/// browsers see the same failover as [`NodeRpcClient::call`].
///
/// Also retries on JSON-RPC **Invalid params** (-32602) for that method only: some public RPC
/// stacks surface version skew or strict param parsing that way; the mining-enabled bootnode may
/// still accept the same body.
fn try_next_upstream_after_jsonrpc_error(request_method: Option<&str>, response: &Value) -> bool {
    if request_method != Some("chain_getMiningWork") {
        return false;
    }
    let Some(err) = response.get("error") else {
        return false;
    };
    let code = err.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
    let msg = err.get("message").and_then(|m| m.as_str()).unwrap_or("");
    if msg.contains("mining disabled") || msg.contains("Mining work not available") {
        return true;
    }
    code == -32602
}

fn jsonrpc_method_from_request_body(body: &[u8]) -> Option<String> {
    let v: Value = serde_json::from_slice(body).ok()?;
    v.get("method")?.as_str().map(str::to_string)
}

pub struct NodeRpcClient {
    urls: Vec<String>,
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
    /// Cover brief Docker DNS / bootnode restarts without surfacing 503 to browsers (which may
    /// label cross-origin failures as "access control checks").
    const TRANSPORT_RETRIES: usize = 20;
    const RETRY_DELAY_MS: u64 = 400;

    pub fn new(url: &str) -> Self {
        let urls = url
            .split(',')
            .map(str::trim)
            .filter(|u| !u.is_empty())
            .map(|u| u.trim_end_matches('/').to_string())
            .collect::<Vec<_>>();
        Self {
            urls,
            // Busy mining nodes can take 10s+ for chain_getInfo / chain_getBlock; 5s caused /chain/info height=null.
            // Connection pool re-enabled (8 idle); TRANSPORT_RETRIES handles stale-connection errors
            // from Docker DNS restarts without surfacing errors to browsers.
            http: Client::builder()
                .timeout(Duration::from_secs(90))
                .pool_max_idle_per_host(8)
                .build()
                .unwrap_or_default(),
            // Block submission (`chain_submitBlock`) can be large + slow; browser → /node-rpc → node.
            http_proxy: Client::builder()
                .timeout(Duration::from_secs(300))
                .pool_max_idle_per_host(8)
                .build()
                .unwrap_or_default(),
        }
    }

    /// Forward a raw JSON-RPC POST body to the node; returns upstream status + body bytes.
    ///
    /// `x_forwarded_for` should be the browser (or edge) client IP when the API proxies browser
    /// traffic — the node's JSON-RPC rate limiter keys on `X-Forwarded-For` / `X-Real-IP`.
    /// Without this, every proxied request appears as the same peer and shares one ~100 RPM bucket.
    pub async fn forward_jsonrpc_body(
        &self,
        body: Bytes,
        x_forwarded_for: Option<&str>,
    ) -> Result<(u16, Bytes), NodeRpcError> {
        if self.urls.is_empty() {
            return Err(NodeRpcError::Unavailable(
                "no upstream RPC URLs configured".to_string(),
            ));
        }

        let mut errors = Vec::new();
        let request_method = jsonrpc_method_from_request_body(&body);

        for url in &self.urls {
            let resp = match self
                .send_proxy_with_retry(url, body.clone(), x_forwarded_for)
                .await
            {
                Ok(resp) => resp,
                Err(e) => {
                    errors.push(format!("{url}: {e}"));
                    continue;
                }
            };

            let status = resp.status().as_u16();
            let bytes = resp
                .bytes()
                .await
                .map_err(|e| NodeRpcError::RequestFailed(format!("{url}: {e}")))?;

            if status == 200 && self.urls.len() > 1 {
                if let Ok(val) = serde_json::from_slice::<Value>(&bytes) {
                    if val.get("error").is_some()
                        && try_next_upstream_after_jsonrpc_error(request_method.as_deref(), &val)
                    {
                        let hint = val
                            .pointer("/error/message")
                            .and_then(|m| m.as_str())
                            .unwrap_or("jsonrpc error");
                        errors.push(format!("{url}: {hint}"));
                        continue;
                    }
                }
            }

            return Ok((status, bytes));
        }

        Err(NodeRpcError::Unavailable(format!(
            "all upstream RPCs failed: {}",
            errors.join("; ")
        )))
    }

    /// Send a JSON-RPC 2.0 request and return the result field.
    async fn call(&self, method: &str, params: Value) -> Result<Value, NodeRpcError> {
        if self.urls.is_empty() {
            return Err(NodeRpcError::Unavailable(
                "no upstream RPC URLs configured".to_string(),
            ));
        }

        let mut errors = Vec::new();

        for url in &self.urls {
            let resp = match self.send_json_with_retry(url, method, params.clone()).await {
                Ok(resp) => resp,
                Err(e) => {
                    errors.push(format!("{url}: {e}"));
                    continue;
                }
            };

            if !resp.status().is_success() {
                errors.push(format!("{url}: HTTP {}", resp.status()));
                continue;
            }

            let data: Value = resp
                .json()
                .await
                .map_err(|e| NodeRpcError::InvalidResponse(format!("{url}: {e}")))?;

            if let Some(error) = data.get("error") {
                errors.push(format!("{url}: {error}"));
                continue;
            }

            return Ok(data.get("result").cloned().unwrap_or(Value::Null));
        }

        Err(NodeRpcError::Unavailable(format!(
            "all upstream RPCs failed: {}",
            errors.join("; ")
        )))
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

    async fn send_proxy_with_retry(
        &self,
        url: &str,
        body: Bytes,
        x_forwarded_for: Option<&str>,
    ) -> Result<reqwest::Response, NodeRpcError> {
        let mut last_error = None;

        for attempt in 0..Self::TRANSPORT_RETRIES {
            let mut req = self
                .http_proxy
                .post(url)
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(body.clone());
            if let Some(ip) = x_forwarded_for.filter(|s| !s.trim().is_empty()) {
                req = req.header("X-Forwarded-For", ip.trim());
            }
            match req.send().await {
                Ok(resp) => return Ok(resp),
                Err(e) => {
                    last_error = Some(e.to_string());
                    if attempt + 1 < Self::TRANSPORT_RETRIES {
                        sleep(Duration::from_millis(Self::RETRY_DELAY_MS)).await;
                    }
                }
            }
        }

        Err(NodeRpcError::Unavailable(
            last_error.unwrap_or_else(|| "unknown transport error".to_string()),
        ))
    }

    async fn send_json_with_retry(
        &self,
        url: &str,
        method: &str,
        params: Value,
    ) -> Result<reqwest::Response, NodeRpcError> {
        let mut last_error = None;

        for attempt in 0..Self::TRANSPORT_RETRIES {
            let body = json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": method,
                "params": params.clone(),
            });

            match self.http.post(url).json(&body).send().await {
                Ok(resp) => return Ok(resp),
                Err(e) => {
                    last_error = Some(e.to_string());
                    if attempt + 1 < Self::TRANSPORT_RETRIES {
                        sleep(Duration::from_millis(Self::RETRY_DELAY_MS)).await;
                    }
                }
            }
        }

        Err(NodeRpcError::Unavailable(
            last_error.unwrap_or_else(|| "unknown transport error".to_string()),
        ))
    }
}
