// =============================================================================
// Long-Running Stability Test & Recovery Test
// =============================================================================
//
// Stability test: run the node under moderate load for an extended period while
//   monitoring memory growth, block production, and RPC responsiveness.
//
// Recovery test: verify that a node restarts cleanly after a crash and resumes
//   from the correct chain state.

use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::{info, warn};

use crate::monitor::{run_monitor_background, NodeMonitor};
use crate::results::{PhaseResult, TestResults};
use std::collections::HashMap;

// ─── Stability Test ───────────────────────────────────────────────────────────

pub async fn run_stability_test(
    rpc_url: &str,
    duration_secs: u64,
    tps: u64,
    sample_interval_secs: u64,
) -> TestResults {
    let mut results = TestResults::new("stability");
    results.metric("config.duration_secs", duration_secs as f64, "s");
    results.metric("config.tps", tps as f64, "ops/s");
    results.metric(
        "config.sample_interval_secs",
        sample_interval_secs as f64,
        "s",
    );

    info!("stability: starting {duration_secs}s test at {tps} TPS");

    let start = Instant::now();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap();

    // Phase 1: Baseline — just measure health for first 30s
    info!("stability: phase 1 — baseline measurement");
    let baseline = measure_node_health(&client, rpc_url).await;
    results.metric("baseline.height", baseline.0 as f64, "blocks");
    results.metric("baseline.mempool", baseline.1 as f64, "txs");
    results.metric("baseline.peers", baseline.2 as f64, "peers");

    // Phase 2: Load — run tx generator + monitor in parallel
    info!("stability: phase 2 — sustained load");
    let rpc_owned = rpc_url.to_string();
    let monitor_handle = tokio::spawn(async move {
        run_monitor_background(rpc_owned, duration_secs, sample_interval_secs).await
    });

    // Submit transactions at the configured TPS for the full duration
    let interval = Duration::from_micros(if tps > 0 { 1_000_000 / tps } else { 1_000_000 });
    let deadline = Instant::now() + Duration::from_secs(duration_secs);
    let mut tx_ok = 0u64;
    let mut tx_err = 0u64;
    let mut nonce = 0u64;

    while Instant::now() < deadline {
        let t0 = Instant::now();
        let tx_hex = build_stability_tx(nonce);
        nonce += 1;

        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": nonce,
            "method": "author_submitExtrinsic",
            "params": [tx_hex]
        });

        match client.post(rpc_url).json(&body).send().await {
            Ok(resp) => {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    if json.get("error").is_none() {
                        tx_ok += 1;
                    } else {
                        tx_err += 1;
                    }
                } else {
                    tx_err += 1;
                }
            }
            Err(_) => {
                tx_err += 1;
            }
        }

        let remaining = interval.saturating_sub(t0.elapsed());
        if !remaining.is_zero() {
            sleep(remaining).await;
        }
    }

    // Phase 3: Post-load — collect final metrics
    info!("stability: phase 3 — post-load measurement");
    let final_state = measure_node_health(&client, rpc_url).await;
    results.metric("final.height", final_state.0 as f64, "blocks");
    results.metric("final.mempool", final_state.1 as f64, "txs");
    results.metric("final.peers", final_state.2 as f64, "peers");

    let blocks_produced = final_state.0.saturating_sub(baseline.0);
    results.metric("produced.blocks", blocks_produced as f64, "blocks");
    results.metric("tx.submitted_ok", tx_ok as f64, "txs");
    results.metric("tx.submitted_err", tx_err as f64, "txs");

    // Collect monitor results
    let monitor = monitor_handle
        .await
        .unwrap_or_else(|_| NodeMonitor::new(rpc_url));
    monitor.apply_to_results(&mut results);

    let elapsed = start.elapsed().as_secs_f64();
    let tx_error_rate = if tx_ok + tx_err > 0 {
        tx_err as f64 / (tx_ok + tx_err) as f64 * 100.0
    } else {
        0.0
    };

    let passed = !monitor.had_outage()
        && !monitor.memory_leaked(1.0) // fail if memory grew > 100%
        && tx_error_rate < 20.0
        && final_state.0 > baseline.0; // at least one block was produced

    results.finish(
        passed,
        format!(
            "Ran {elapsed:.0}s under {tps} TPS — {blocks_produced} blocks produced, \
             {tx_ok} txs ok, {tx_error_rate:.1}% tx errors"
        ),
        elapsed,
    );

    info!("stability: complete — blocks={blocks_produced} tx_ok={tx_ok} tx_err_rate={tx_error_rate:.1}%");
    results
}

// ─── Recovery Test ────────────────────────────────────────────────────────────

pub async fn run_recovery_test(
    rpc_url: &str,
    restart_wait_secs: u64,
    restart_cmd: Option<String>,
) -> TestResults {
    let mut results = TestResults::new("recovery");
    results.metric("config.restart_wait_secs", restart_wait_secs as f64, "s");

    info!("recovery: testing node crash recovery");

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();

    let start = Instant::now();
    let mut phases = Vec::new();

    // Phase 1: Pre-crash state
    let pre_state = measure_node_health(&client, rpc_url).await;
    results.metric("pre.height", pre_state.0 as f64, "blocks");
    results.metric("pre.peers", pre_state.2 as f64, "peers");

    phases.push(PhaseResult {
        name: "pre-crash".into(),
        passed: pre_state.0 > 0,
        elapsed_secs: 0.1,
        summary: format!("Height: {}, Peers: {}", pre_state.0, pre_state.2),
        metrics: HashMap::new(),
    });

    if pre_state.0 == 0 {
        results.error(
            "Node appears to be down before recovery test started".to_string(),
            1,
        );
        results.finish(
            false,
            "Node not responsive before recovery test".to_string(),
            0.1,
        );
        results.phases = phases;
        return results;
    }

    // Phase 2: Simulate crash detection / wait for restart
    info!("recovery: waiting {restart_wait_secs}s (simulating restart window)");
    let phase2_start = Instant::now();

    if let Some(cmd) = &restart_cmd {
        info!("recovery: executing restart command: {cmd}");
        // Execute restart command
        let status = tokio::process::Command::new("sh")
            .args(["-c", cmd])
            .status()
            .await;
        match status {
            Ok(s) if s.success() => info!("recovery: restart command succeeded"),
            Ok(s) => warn!("recovery: restart command exited with {s}"),
            Err(e) => warn!("recovery: restart command failed: {e}"),
        }
    } else {
        results.note(
            "No restart command provided — assuming manual restart or monitoring only".to_string(),
        );
    }

    sleep(Duration::from_secs(restart_wait_secs)).await;

    phases.push(PhaseResult {
        name: "restart-wait".into(),
        passed: true,
        elapsed_secs: phase2_start.elapsed().as_secs_f64(),
        summary: format!("Waited {restart_wait_secs}s for restart"),
        metrics: HashMap::new(),
    });

    // Phase 3: Post-restart health check
    info!("recovery: checking node health after restart");
    let phase3_start = Instant::now();
    let mut post_state = (0u64, 0u64, 0u32);
    let mut recovered = false;

    // Poll for up to 60 seconds for the node to come back
    for _ in 0..60 {
        post_state = measure_node_health(&client, rpc_url).await;
        if post_state.0 >= pre_state.0 {
            recovered = true;
            break;
        }
        sleep(Duration::from_secs(1)).await;
    }

    results.metric("post.height", post_state.0 as f64, "blocks");
    results.metric("post.peers", post_state.2 as f64, "peers");
    results.metric(
        "recovery.height_delta",
        post_state.0.saturating_sub(pre_state.0) as f64,
        "blocks",
    );

    phases.push(PhaseResult {
        name: "post-restart".into(),
        passed: recovered,
        elapsed_secs: phase3_start.elapsed().as_secs_f64(),
        summary: if recovered {
            format!(
                "Node recovered at height {} (was {})",
                post_state.0, pre_state.0
            )
        } else {
            format!(
                "Node did NOT recover — height {} < pre-crash {}",
                post_state.0, pre_state.0
            )
        },
        metrics: HashMap::new(),
    });

    if !recovered {
        results.error(
            format!(
                "Node height {} after restart is below pre-crash height {}",
                post_state.0, pre_state.0
            ),
            1,
        );
    }

    let elapsed = start.elapsed().as_secs_f64();
    results.phases = phases;

    results.finish(
        recovered,
        if recovered {
            format!(
                "Node recovered successfully — height {}->{}",
                pre_state.0, post_state.0
            )
        } else {
            format!(
                "Node recovery FAILED — height {}->{}",
                pre_state.0, post_state.0
            )
        },
        elapsed,
    );

    info!("recovery: complete — recovered={recovered}");
    results
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Returns (block_height, mempool_size, peer_count).
async fn measure_node_health(client: &reqwest::Client, rpc_url: &str) -> (u64, u64, u32) {
    let height = rpc_u64(client, rpc_url, "chain_getBlockNumber")
        .await
        .unwrap_or(0);
    let mempool = rpc_u64(client, rpc_url, "mempool_size").await.unwrap_or(0);
    let peers = rpc_u64(client, rpc_url, "net_peerCount").await.unwrap_or(0) as u32;
    (height, mempool, peers)
}

async fn rpc_u64(client: &reqwest::Client, rpc_url: &str, method: &str) -> Option<u64> {
    let body = serde_json::json!({
        "jsonrpc": "2.0", "id": 1,
        "method": method, "params": []
    });
    let resp = client.post(rpc_url).json(&body).send().await.ok()?;
    let json: serde_json::Value = resp.json().await.ok()?;
    json["result"]
        .as_u64()
        .or_else(|| json["result"].as_str()?.parse().ok())
}

fn build_stability_tx(nonce: u64) -> String {
    let mut bytes = [0u8; 64];
    bytes[0] = 0x01;
    bytes[2..10].copy_from_slice(&nonce.to_le_bytes());
    bytes[10..18].copy_from_slice(&1u64.to_le_bytes());
    let h = blake3::hash(&bytes[..18]);
    bytes[18..50].copy_from_slice(&h.as_bytes()[..32]);
    hex::encode(&bytes[..])
}
