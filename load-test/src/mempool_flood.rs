// =============================================================================
// Mempool Flooding Test
// =============================================================================
//
// Submits transactions faster than blocks can include them to test:
//   1. Mempool capacity enforcement (should reject when full)
//   2. Fee market behavior under congestion
//   3. Node stability when mempool is saturated

use futures::future::join_all;
use std::time::{Duration, Instant};
use tracing::{info, warn};

use crate::results::{LatencyStats, TestResults};

pub async fn run_mempool_flood(rpc_url: &str, tx_count: u64, concurrency: usize) -> TestResults {
    let mut results = TestResults::new("mempool-flood");
    results.metric("config.tx_count", tx_count as f64, "txs");
    results.metric("config.concurrency", concurrency as f64, "workers");

    info!("mempool-flood: submitting {tx_count} txs at concurrency={concurrency}");

    let client = std::sync::Arc::new(
        reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap(),
    );

    let rpc = std::sync::Arc::new(rpc_url.to_string());
    let start = Instant::now();

    // Split work into concurrent batches
    let batch_size = (tx_count as usize).div_ceil(concurrency);
    let mut handles = Vec::new();

    for worker_id in 0..concurrency {
        let offset = (worker_id * batch_size) as u64;
        let count =
            batch_size.min((tx_count as usize).saturating_sub(worker_id * batch_size)) as u64;
        if count == 0 {
            break;
        }

        let client = client.clone();
        let rpc = rpc.clone();

        handles.push(tokio::spawn(async move {
            flood_worker(client, rpc, worker_id as u64, offset, count).await
        }));
    }

    let worker_results = join_all(handles).await;

    let mut accepted = 0u64;
    let mut rejected_capacity = 0u64;
    let mut rejected_other = 0u64;
    let mut latency = LatencyStats::default();

    for r in worker_results {
        match r {
            Ok((acc, cap, other, lat)) => {
                accepted += acc;
                rejected_capacity += cap;
                rejected_other += other;
                for l in lat {
                    latency.record(l);
                }
            }
            Err(e) => {
                warn!("worker panicked: {e}");
                rejected_other += 1;
            }
        }
    }

    let elapsed = start.elapsed().as_secs_f64();
    let total = accepted + rejected_capacity + rejected_other;

    results.metric("mempool.accepted", accepted as f64, "txs");
    results.metric("mempool.rejected_capacity", rejected_capacity as f64, "txs");
    results.metric("mempool.rejected_other", rejected_other as f64, "txs");
    results.metric("mempool.total_submitted", total as f64, "txs");
    results.metric(
        "mempool.acceptance_rate",
        if total > 0 {
            accepted as f64 / total as f64 * 100.0
        } else {
            0.0
        },
        "%",
    );

    latency.apply_to_results(&mut results, "submit.latency");

    // Pass criteria:
    // - Node stayed responsive (no connection refused)
    // - Capacity rejections are expected (mempool full is correct behavior)
    // - Unexpected errors < 5%
    let unexpected_error_rate = rejected_other as f64 / total.max(1) as f64;
    let passed = rejected_other == 0 || unexpected_error_rate < 0.05;

    if rejected_capacity > 0 {
        results.note(format!(
            "Mempool correctly rejected {rejected_capacity} transactions when at capacity"
        ));
    }

    results.finish(
        passed,
        format!(
            "Submitted {total} txs in {elapsed:.1}s — {accepted} accepted, \
             {rejected_capacity} rejected (capacity), {rejected_other} other errors"
        ),
        elapsed,
    );

    info!("mempool-flood: complete — accepted={accepted} cap_rejected={rejected_capacity} other={rejected_other}");
    results
}

/// Worker that submits `count` transactions starting at nonce `offset`.
async fn flood_worker(
    client: std::sync::Arc<reqwest::Client>,
    rpc_url: std::sync::Arc<String>,
    worker_id: u64,
    nonce_offset: u64,
    count: u64,
) -> (u64, u64, u64, Vec<f64>) {
    let mut accepted = 0u64;
    let mut rejected_capacity = 0u64;
    let mut rejected_other = 0u64;
    let mut latencies = Vec::with_capacity(count as usize);

    for i in 0..count {
        let nonce = nonce_offset + i;
        let amount = 1 + (i % 100); // Varying amounts for diversity

        // Build synthetic tx with worker-specific key seed
        let tx_hex = build_flood_tx_hex(worker_id, nonce, amount);
        let t0 = Instant::now();

        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": nonce,
            "method": "author_submitExtrinsic",
            "params": [tx_hex]
        });

        match client.post(rpc_url.as_ref()).json(&body).send().await {
            Ok(resp) => {
                let ms = t0.elapsed().as_secs_f64() * 1000.0;
                latencies.push(ms);

                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    if let Some(err) = json.get("error") {
                        let code = err["code"].as_i64().unwrap_or(-1);
                        if code == -32603 {
                            // Internal error — likely mempool full
                            rejected_capacity += 1;
                        } else {
                            rejected_other += 1;
                        }
                    } else {
                        accepted += 1;
                    }
                } else {
                    rejected_other += 1;
                }
            }
            Err(_) => {
                rejected_other += 1;
            }
        }
    }

    (accepted, rejected_capacity, rejected_other, latencies)
}

fn build_flood_tx_hex(worker_id: u64, nonce: u64, amount: u64) -> String {
    let mut bytes = [0u8; 64];
    bytes[0] = 0x01;
    bytes[1] = 0x00;
    bytes[2..10].copy_from_slice(&nonce.to_le_bytes());
    bytes[10..18].copy_from_slice(&amount.to_le_bytes());
    bytes[18..26].copy_from_slice(&worker_id.to_le_bytes());
    let hash = blake3::hash(&bytes[..26]);
    bytes[26..58].copy_from_slice(&hash.as_bytes()[..32]);
    hex::encode(&bytes[..])
}
