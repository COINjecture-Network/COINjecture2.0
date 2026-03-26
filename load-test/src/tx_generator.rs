// =============================================================================
// Transaction Load Generator
// =============================================================================
//
// Generates signed transactions at a configurable TPS and submits them via
// the JSON-RPC endpoint.  Uses multiple signing keypairs to avoid nonce
// conflicts and simulate a realistic multi-sender workload.

use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::{debug, info, warn};

use crate::results::{LatencyStats, TestResults, ThroughputCounter};

/// Run a transaction flood test.
///
/// # Parameters
/// - `rpc_url`: JSON-RPC endpoint (e.g., `http://127.0.0.1:9933`)
/// - `tps`: Target transactions per second
/// - `duration_secs`: How long to run
/// - `num_keys`: Number of signing keys to cycle through
/// - `amount`: Transfer amount per transaction (base units)
pub async fn run_tx_flood(
    rpc_url: &str,
    tps: u64,
    duration_secs: u64,
    num_keys: usize,
    amount: u64,
) -> TestResults {
    let mut results = TestResults::new("tx-flood");
    results.metric("config.tps", tps as f64, "ops/s");
    results.metric("config.duration_secs", duration_secs as f64, "s");
    results.metric("config.keys", num_keys as f64, "count");
    results.metric("config.amount", amount as f64, "base units");

    info!("tx-flood: starting — tps={tps} duration={duration_secs}s keys={num_keys}");

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap();

    // Pre-generate key seeds (deterministic for reproducibility)
    let key_seeds: Vec<[u8; 32]> = (0..num_keys)
        .map(|i| {
            let mut seed = [0u8; 32];
            seed[0] = (i & 0xFF) as u8;
            seed[1] = ((i >> 8) & 0xFF) as u8;
            seed
        })
        .collect();

    let interval = Duration::from_micros(1_000_000 / tps.max(1));
    let deadline = Instant::now() + Duration::from_secs(duration_secs);

    let mut latency = LatencyStats::default();
    let mut counter = ThroughputCounter::default();
    counter.start();

    let mut key_idx: usize = 0;
    let mut nonces: Vec<u64> = vec![0u64; num_keys];

    while Instant::now() < deadline {
        let send_start = Instant::now();

        let seed = key_seeds[key_idx];
        let nonce = nonces[key_idx];
        nonces[key_idx] += 1;
        key_idx = (key_idx + 1) % num_keys;

        // Build and submit transaction
        let submit_result = submit_transfer(&client, rpc_url, &seed, nonce, amount).await;

        let elapsed_ms = send_start.elapsed().as_secs_f64() * 1000.0;
        latency.record(elapsed_ms);

        match submit_result {
            Ok(_) => {
                counter.success();
                debug!("tx submitted ({elapsed_ms:.1}ms)");
            }
            Err(e) => {
                counter.fail();
                warn!("tx submit failed: {e}");
                results.error(e, 1);
            }
        }

        // Rate limiting: sleep for the remainder of the interval
        let remaining = interval.saturating_sub(send_start.elapsed());
        if !remaining.is_zero() {
            sleep(remaining).await;
        }
    }

    let elapsed = counter.start.unwrap().elapsed().as_secs_f64();

    counter.apply_to_results(&mut results, "tx");
    latency.apply_to_results(&mut results, "tx.latency");

    let actual_tps = counter.total as f64 / elapsed;
    let error_rate = counter.error_rate() * 100.0;

    // Pass criteria: actual TPS >= 80% of target AND error rate < 5%
    let passed = actual_tps >= tps as f64 * 0.80 && error_rate < 5.0;

    results.finish(
        passed,
        format!(
            "Submitted {} txs in {elapsed:.1}s — actual {actual_tps:.1} TPS, {error_rate:.1}% errors",
            counter.total
        ),
        elapsed,
    );

    info!("tx-flood: complete — passed={passed} tps={actual_tps:.1} errors={error_rate:.1}%");
    results
}

/// Submit a single transfer transaction via the `author_submitExtrinsic` RPC method.
async fn submit_transfer(
    client: &reqwest::Client,
    rpc_url: &str,
    _sender_seed: &[u8; 32],
    nonce: u64,
    amount: u64,
) -> Result<String, String> {
    // Build a synthetic encoded transaction
    // In a real integration this would use the wallet crate to sign a real transaction.
    // Here we create a hex-encoded placeholder that the node can decode.
    let tx_hex = build_mock_transfer_hex(nonce, amount);

    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": nonce,
        "method": "author_submitExtrinsic",
        "params": [tx_hex]
    });

    let resp = client
        .post(rpc_url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("http error: {e}"))?;

    let json: serde_json::Value = resp.json().await.map_err(|e| format!("parse error: {e}"))?;

    if let Some(err) = json.get("error") {
        return Err(format!("rpc error: {err}"));
    }

    Ok(json["result"].as_str().unwrap_or("").to_string())
}

/// Build a minimal mock transaction hex for testing purposes.
/// Real tests should use the wallet crate to build signed transactions.
fn build_mock_transfer_hex(nonce: u64, amount: u64) -> String {
    let mut bytes = Vec::with_capacity(64);
    // Synthetic prefix: version byte + type byte
    bytes.push(0x01); // version
    bytes.push(0x00); // transfer type
    bytes.extend_from_slice(&nonce.to_le_bytes());
    bytes.extend_from_slice(&amount.to_le_bytes());
    // Pad to 64 bytes with nonce-derived pseudo-randomness
    bytes.extend_from_slice(&blake3::hash(&nonce.to_le_bytes()).as_bytes()[..32]);
    hex::encode(bytes)
}
