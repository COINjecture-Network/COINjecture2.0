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
    if msg.contains("mining disabled")
        || msg.contains("Mining work not available")
        || msg.contains("Miner is producing a block")
        || msg.contains("Mining template not ready")
    {
        return true;
    }
    code == -32602
}

fn jsonrpc_method_from_request_body(body: &[u8]) -> Option<String> {
    let v: Value = serde_json::from_slice(body).ok()?;
    v.get("method")?.as_str().map(str::to_string)
}

/// Prefer the mining-enabled upstream for methods that mutate mempool / mine blocks.
fn prefer_mining_upstream(method: Option<&str>) -> bool {
    matches!(
        method,
        Some("chain_getMiningWork") | Some("transaction_submit") | Some("chain_submitBlock")
    )
}

pub struct NodeRpcClient {
    urls: Vec<String>,
    /// Tried before `urls` for miner-facing RPC (deduped).
    mining_urls: Vec<String>,
    /// Small JSON-RPC (`chain_getInfo`, `network_getInfo`) — must not sit near the heavy read timeout.
    http_light: Client,
    /// Large JSON-RPC (`chain_getBlock`, `chain_getLatestBlock`).
    http_heavy: Client,
    /// Browser-originated `POST /node-rpc` (may include huge `chain_submitBlock`).
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
    /// Retries after transport errors. Keep small: each attempt can cost the **full** HTTP read
    /// timeout (below), so 20×90s was pathological when an upstream was wedged.
    const TRANSPORT_RETRIES: usize = 4;
    const RETRY_DELAY_MS: u64 = 400;
    const LIGHT_RPC_TIMEOUT_SECS: u64 = 22;
    const HEAVY_RPC_TIMEOUT_SECS: u64 = 90;
    /// Per-upstream cap for `chain_getMiningWork` so wedged miners fail over before the browser gives up.
    const MINING_WORK_PROXY_TIMEOUT_SECS: u64 = 35;

    fn parse_url_list(raw: &str) -> Vec<String> {
        raw.split(',')
            .map(str::trim)
            .filter(|u| !u.is_empty())
            .map(|u| u.trim_end_matches('/').to_string())
            .collect()
    }

    fn merge_mining_urls(mining_urls: &[String], urls: &[String]) -> Vec<String> {
        let mut merged = mining_urls.to_vec();
        for u in urls {
            if !merged.iter().any(|m| m == u) {
                merged.push(u.clone());
            }
        }
        merged
    }

    pub fn new(url: &str) -> Self {
        Self::with_mining_url(url, None)
    }

    pub fn with_mining_url(url: &str, mining_url: Option<&str>) -> Self {
        let urls = Self::parse_url_list(url);
        let mining_urls = mining_url
            .map(Self::parse_url_list)
            .unwrap_or_default();
        Self {
            urls,
            mining_urls,
            http_light: Client::builder()
                .connect_timeout(Duration::from_secs(5))
                .timeout(Duration::from_secs(Self::LIGHT_RPC_TIMEOUT_SECS))
                .pool_max_idle_per_host(8)
                .build()
                .unwrap_or_default(),
            http_heavy: Client::builder()
                .connect_timeout(Duration::from_secs(8))
                .timeout(Duration::from_secs(Self::HEAVY_RPC_TIMEOUT_SECS))
                .pool_max_idle_per_host(8)
                .build()
                .unwrap_or_default(),
            // Block submission (`chain_submitBlock`) can be large + slow; browser → /node-rpc → node.
            http_proxy: Client::builder()
                .connect_timeout(Duration::from_secs(8))
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

        let prefer_miner = prefer_mining_upstream(request_method.as_deref());
        let upstreams = if prefer_miner {
            Self::merge_mining_urls(&self.mining_urls, &self.urls)
        } else {
            self.urls.clone()
        };

        for url in &upstreams {
            let resp = match if prefer_miner {
                self.send_proxy_once_with_timeout(
                    url,
                    body.clone(),
                    x_forwarded_for,
                    Duration::from_secs(Self::MINING_WORK_PROXY_TIMEOUT_SECS),
                )
                .await
            } else {
                self.send_proxy_with_retry(url, body.clone(), x_forwarded_for)
                    .await
            } {
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

            if status == 200 && upstreams.len() > 1 {
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
    async fn call_on(
        &self,
        client: &Client,
        method: &str,
        params: Value,
    ) -> Result<Value, NodeRpcError> {
        if self.urls.is_empty() {
            return Err(NodeRpcError::Unavailable(
                "no upstream RPC URLs configured".to_string(),
            ));
        }

        let mut errors = Vec::new();
        let upstreams = if prefer_mining_upstream(Some(method)) {
            Self::merge_mining_urls(&self.mining_urls, &self.urls)
        } else {
            self.urls.clone()
        };

        for url in &upstreams {
            let resp = match self
                .send_json_with_retry(client, url, method, params.clone())
                .await
            {
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
        self.call_on(&self.http_light, "network_getInfo", json!([]))
            .await
    }

    /// Get chain info from the node.
    ///
    /// `peer_count` uses the **maximum** across all `NODE_RPC_URL` upstreams. In a star
    /// topology (node1–node3 dial bootnode only), followers report 1 peer while the bootnode
    /// reports the full fleet — the UI should reflect network size, not one miner's link count.
    pub async fn get_chain_info(&self) -> Result<Value, NodeRpcError> {
        let mut info = self
            .call_on(&self.http_light, "chain_getInfo", json!([]))
            .await?;
        if let Some(max_peers) = self.max_peer_count_across_upstreams().await {
            if let Some(obj) = info.as_object_mut() {
                obj.insert("peer_count".to_string(), json!(max_peers));
            }
        }
        Ok(info)
    }

    /// Best-effort max `peer_count` from every configured upstream (parallel, light timeout).
    async fn max_peer_count_across_upstreams(&self) -> Option<u64> {
        if self.urls.is_empty() {
            return None;
        }
        let client = self.http_light.clone();
        let mut tasks = tokio::task::JoinSet::new();
        for url in self.urls.clone() {
            let client = client.clone();
            tasks.spawn(async move {
                let body = json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "chain_getInfo",
                    "params": [],
                });
                let resp = client.post(&url).json(&body).send().await.ok()?;
                if !resp.status().is_success() {
                    return None;
                }
                let data: Value = resp.json().await.ok()?;
                data.pointer("/result/peer_count")?.as_u64()
            });
        }
        let mut max_peers: Option<u64> = None;
        while let Some(joined) = tasks.join_next().await {
            if let Ok(Some(n)) = joined {
                max_peers = Some(max_peers.map_or(n, |m| m.max(n)));
            }
        }
        max_peers
    }

    /// Get the latest block from the node.
    pub async fn get_latest_block(&self) -> Result<Value, NodeRpcError> {
        self.call_on(&self.http_heavy, "chain_getLatestBlock", json!([]))
            .await
    }

    /// Get a block by height.
    pub async fn get_block_by_height(&self, height: u64) -> Result<Value, NodeRpcError> {
        self.call_on(&self.http_heavy, "chain_getBlock", json!([height]))
            .await
    }

    async fn send_proxy_once_with_timeout(
        &self,
        url: &str,
        body: Bytes,
        x_forwarded_for: Option<&str>,
        timeout: Duration,
    ) -> Result<reqwest::Response, NodeRpcError> {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(8))
            .timeout(timeout)
            .build()
            .unwrap_or_else(|_| self.http_proxy.clone());
        let mut req = client
            .post(url)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body);
        if let Some(ip) = x_forwarded_for.filter(|s| !s.trim().is_empty()) {
            req = req.header("X-Forwarded-For", ip.trim());
        }
        req.send()
            .await
            .map_err(|e| NodeRpcError::Unavailable(e.to_string()))
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
        client: &Client,
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

            match client.post(url).json(&body).send().await {
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
