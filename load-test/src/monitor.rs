// =============================================================================
// Load Test Monitor — Memory, Disk, and Node Health Tracking
// =============================================================================

use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::{info, warn};

use crate::results::TestResults;

/// Snapshot of node health metrics.
#[derive(Debug, Clone)]
pub struct HealthSnapshot {
    pub ts: Instant,
    pub block_height: u64,
    pub mempool_size: u64,
    pub peer_count: u32,
    pub rpc_ok: bool,
    /// Process RSS memory in bytes (0 if unavailable)
    pub memory_rss_bytes: u64,
}

/// Monitor that polls an RPC endpoint at regular intervals.
pub struct NodeMonitor {
    rpc_url: String,
    client: reqwest::Client,
    snapshots: Vec<HealthSnapshot>,
    start: Instant,
}

impl NodeMonitor {
    pub fn new(rpc_url: &str) -> Self {
        NodeMonitor {
            rpc_url: rpc_url.to_string(),
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .expect("http client init failed"),
            snapshots: Vec::new(),
            start: Instant::now(),
        }
    }

    /// Poll once and record a snapshot.
    pub async fn poll(&mut self) {
        let snap = self.poll_once().await;
        if !snap.rpc_ok {
            warn!("health poll: RPC unresponsive");
        } else {
            info!(
                height = snap.block_height,
                mempool = snap.mempool_size,
                peers = snap.peer_count,
                "health snapshot"
            );
        }
        self.snapshots.push(snap);
    }

    async fn poll_once(&self) -> HealthSnapshot {
        let ts = Instant::now();

        // --- block height ---
        let height = self.rpc_get_u64("chain_getBlockNumber").await.unwrap_or(0);

        // --- mempool size ---
        let mempool_size = self.rpc_get_u64("mempool_size").await.unwrap_or(0);

        // --- peer count ---
        let peer_count = self.rpc_get_u64("net_peerCount").await.unwrap_or(0) as u32;

        let rpc_ok = height > 0 || mempool_size == 0; // rough liveness check

        HealthSnapshot {
            ts,
            block_height: height,
            mempool_size,
            peer_count,
            rpc_ok,
            memory_rss_bytes: read_process_rss(),
        }
    }

    /// Returns true if any snapshot shows the node was unresponsive.
    pub fn had_outage(&self) -> bool {
        self.snapshots.iter().any(|s| !s.rpc_ok)
    }

    /// Detect memory leaks: returns true if memory grew by more than `threshold_pct` (0.0–1.0)
    /// over the monitoring period.
    pub fn memory_leaked(&self, threshold_pct: f64) -> bool {
        let non_zero: Vec<u64> = self
            .snapshots
            .iter()
            .map(|s| s.memory_rss_bytes)
            .filter(|&m| m > 0)
            .collect();

        if non_zero.len() < 2 {
            return false; // not enough data
        }

        let first = *non_zero.first().unwrap() as f64;
        let last = *non_zero.last().unwrap() as f64;

        (last - first) / first > threshold_pct
    }

    /// Peak memory usage seen during the test.
    pub fn peak_memory_bytes(&self) -> u64 {
        self.snapshots
            .iter()
            .map(|s| s.memory_rss_bytes)
            .max()
            .unwrap_or(0)
    }

    /// Final block height (last snapshot).
    pub fn final_height(&self) -> u64 {
        self.snapshots.last().map(|s| s.block_height).unwrap_or(0)
    }

    /// Apply collected metrics to a TestResults struct.
    pub fn apply_to_results(&self, results: &mut TestResults) {
        results.metric(
            "monitor.peak_memory_mb",
            self.peak_memory_bytes() as f64 / (1024.0 * 1024.0),
            "MB",
        );
        results.metric("monitor.final_height", self.final_height() as f64, "blocks");
        results.metric(
            "monitor.snapshots_taken",
            self.snapshots.len() as f64,
            "samples",
        );
        results.metric(
            "monitor.outage_count",
            self.snapshots.iter().filter(|s| !s.rpc_ok).count() as f64,
            "events",
        );

        if self.memory_leaked(0.50) {
            results.note("WARNING: process memory grew >50% — possible memory leak");
        }
        if self.had_outage() {
            results.note("WARNING: node was unresponsive during at least one poll");
        }
    }

    async fn rpc_get_u64(&self, method: &str) -> Option<u64> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": []
        });

        let resp = self
            .client
            .post(&self.rpc_url)
            .json(&body)
            .send()
            .await
            .ok()?;

        let json: serde_json::Value = resp.json().await.ok()?;
        json["result"]
            .as_u64()
            .or_else(|| json["result"].as_str()?.parse().ok())
    }
}

/// Run monitor polling in the background for `duration` seconds with `interval` between polls.
/// Returns all collected snapshots when the duration elapses.
pub async fn run_monitor_background(
    rpc_url: String,
    duration_secs: u64,
    interval_secs: u64,
) -> NodeMonitor {
    let mut monitor = NodeMonitor::new(&rpc_url);
    let deadline = Instant::now() + Duration::from_secs(duration_secs);

    while Instant::now() < deadline {
        monitor.poll().await;
        let remaining = deadline.saturating_duration_since(Instant::now());
        let sleep_dur = Duration::from_secs(interval_secs).min(remaining);
        if sleep_dur.is_zero() {
            break;
        }
        sleep(sleep_dur).await;
    }

    monitor
}

/// Read the current process RSS from /proc/self/status (Linux) or return 0.
fn read_process_rss() -> u64 {
    #[cfg(target_os = "linux")]
    {
        if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
            for line in status.lines() {
                if line.starts_with("VmRSS:") {
                    let kb: u64 = line
                        .split_whitespace()
                        .nth(1)
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(0);
                    return kb * 1024;
                }
            }
        }
    }
    0
}
