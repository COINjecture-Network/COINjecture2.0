// =============================================================================
// Concurrent RPC Load Test
// =============================================================================
//
// Blasts all available RPC endpoints simultaneously to test:
//   1. RPC server throughput under concurrent load
//   2. Response latency under contention
//   3. Correct error responses (not panics)

use futures::future::join_all;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

use crate::results::{LatencyStats, TestResults};

/// All RPC methods to test with their expected parameters.
const RPC_ENDPOINTS: &[(&str, &str)] = &[
    ("chain_getBlockNumber", r#"[]"#),
    ("chain_getBlockHash", r#"[0]"#),
    ("chain_getBlock", r#"[]"#),
    ("chain_getBestBlock", r#"[]"#),
    ("net_peerCount", r#"[]"#),
    ("net_version", r#"[]"#),
    ("system_health", r#"[]"#),
    ("system_name", r#"[]"#),
    ("system_version", r#"[]"#),
    ("mempool_size", r#"[]"#),
    ("mempool_pendingTxs", r#"[]"#),
    ("rpc_methods", r#"[]"#),
];

pub async fn run_rpc_blast(rpc_url: &str, concurrency: usize, duration_secs: u64) -> TestResults {
    let mut results = TestResults::new("rpc-blast");
    results.metric("config.concurrency", concurrency as f64, "workers");
    results.metric("config.duration_secs", duration_secs as f64, "s");
    results.metric("config.endpoints", RPC_ENDPOINTS.len() as f64, "methods");

    info!("rpc-blast: starting — concurrency={concurrency} duration={duration_secs}s");

    let client = std::sync::Arc::new(
        reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .pool_max_idle_per_host(concurrency)
            .build()
            .unwrap(),
    );

    let rpc = std::sync::Arc::new(rpc_url.to_string());
    let deadline = Instant::now() + Duration::from_secs(duration_secs);
    let deadline = std::sync::Arc::new(deadline);

    // Launch `concurrency` workers, each cycling through all endpoints
    let mut handles = Vec::new();
    for worker_id in 0..concurrency {
        let client = client.clone();
        let rpc = rpc.clone();
        let deadline = deadline.clone();

        handles.push(tokio::spawn(async move {
            rpc_worker(client, rpc, worker_id, deadline).await
        }));
    }

    let worker_results = join_all(handles).await;

    let mut total_ok = 0u64;
    let mut total_err = 0u64;
    let mut per_method_ok: std::collections::HashMap<String, u64> = Default::default();
    let mut latency = LatencyStats::default();

    for r in worker_results {
        match r {
            Ok((ok, err, method_ok, lats)) => {
                total_ok += ok;
                total_err += err;
                for (m, c) in method_ok {
                    *per_method_ok.entry(m).or_default() += c;
                }
                for l in lats {
                    latency.record(l);
                }
            }
            Err(e) => {
                warn!("rpc worker panicked: {e}");
                total_err += 1;
            }
        }
    }

    let elapsed = Instant::now()
        .duration_since(*deadline - Duration::from_secs(duration_secs))
        .as_secs_f64()
        .min(duration_secs as f64 + 1.0);

    let total = total_ok + total_err;
    let rps = total_ok as f64 / elapsed;
    let error_rate = if total > 0 {
        total_err as f64 / total as f64 * 100.0
    } else {
        0.0
    };

    results.metric("rpc.total_requests", total as f64, "reqs");
    results.metric("rpc.successful", total_ok as f64, "reqs");
    results.metric("rpc.errors", total_err as f64, "reqs");
    results.metric("rpc.requests_per_second", rps, "req/s");
    results.metric("rpc.error_rate", error_rate, "%");

    latency.apply_to_results(&mut results, "rpc.latency");

    // Per-method breakdown
    for (method, count) in &per_method_ok {
        results.metric(format!("method.{method}"), *count as f64, "reqs");
    }

    let passed = error_rate < 10.0 && total_ok > 0;

    results.finish(
        passed,
        format!("{total} requests in {elapsed:.1}s — {rps:.0} req/s, {error_rate:.1}% errors"),
        elapsed,
    );

    info!("rpc-blast: complete — rps={rps:.0} errors={error_rate:.1}%");
    results
}

async fn rpc_worker(
    client: std::sync::Arc<reqwest::Client>,
    rpc_url: std::sync::Arc<String>,
    worker_id: usize,
    deadline: std::sync::Arc<Instant>,
) -> (u64, u64, std::collections::HashMap<String, u64>, Vec<f64>) {
    let mut ok = 0u64;
    let mut err = 0u64;
    let mut method_ok: std::collections::HashMap<String, u64> = Default::default();
    let mut latencies = Vec::new();

    let mut method_idx = worker_id % RPC_ENDPOINTS.len();

    while Instant::now() < *deadline {
        let (method, params_str) = RPC_ENDPOINTS[method_idx];
        method_idx = (method_idx + 1) % RPC_ENDPOINTS.len();

        let params: serde_json::Value =
            serde_json::from_str(params_str).unwrap_or(serde_json::json!([]));
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": ok,
            "method": method,
            "params": params
        });

        let t0 = Instant::now();
        match client.post(rpc_url.as_ref()).json(&body).send().await {
            Ok(resp) => {
                let ms = t0.elapsed().as_secs_f64() * 1000.0;
                latencies.push(ms);

                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    if json.get("error").is_none() {
                        ok += 1;
                        *method_ok.entry(method.to_string()).or_default() += 1;
                        debug!("rpc {method} ok ({ms:.1}ms)");
                    } else {
                        err += 1;
                        debug!("rpc {method} error");
                    }
                } else {
                    err += 1;
                }
            }
            Err(e) => {
                warn!("rpc {method} connection error: {e}");
                err += 1;
            }
        }
    }

    (ok, err, method_ok, latencies)
}
