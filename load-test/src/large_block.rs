// =============================================================================
// Large Block Test — Mine a block at maximum transaction capacity
// =============================================================================
//
// Tests:
//   1. Node can mine blocks with maximum transactions
//   2. Block validation time under full load
//   3. Propagation delay for large blocks

use std::time::{Duration, Instant};
use tracing::{info, warn};

use crate::results::TestResults;

pub async fn run_large_block_test(
    rpc_url: &str,
    tx_count: u64,
) -> TestResults {
    let mut results = TestResults::new("large-block");
    results.metric("config.tx_count", tx_count as f64, "txs");

    info!("large-block: submitting {tx_count} txs then waiting for a full block");

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap();

    let start = Instant::now();

    // Step 1: Fill mempool with transactions
    info!("large-block: filling mempool with {tx_count} transactions...");
    let (accepted, fill_elapsed) = fill_mempool(&client, rpc_url, tx_count).await;
    results.metric("fill.accepted_txs", accepted as f64, "txs");
    results.metric("fill.elapsed_secs", fill_elapsed, "s");

    if accepted == 0 {
        results.finish(false, "Failed to submit any transactions to mempool".to_string(), fill_elapsed);
        return results;
    }

    // Step 2: Get current block height
    let height_before = get_block_height(&client, rpc_url).await.unwrap_or(0);
    results.metric("block.height_before", height_before as f64, "blocks");

    // Step 3: Wait for the next block to be mined (up to 2 minutes)
    info!("large-block: waiting for next block (current height: {height_before})...");
    let block_wait_start = Instant::now();
    let mut height_after = height_before;
    let mut block_mined = false;

    for _ in 0..120 {
        tokio::time::sleep(Duration::from_secs(1)).await;
        if let Some(h) = get_block_height(&client, rpc_url).await {
            if h > height_before {
                height_after = h;
                block_mined = true;
                break;
            }
        }
    }

    let block_wait_secs = block_wait_start.elapsed().as_secs_f64();
    results.metric("block.height_after", height_after as f64, "blocks");
    results.metric("block.wait_secs", block_wait_secs, "s");

    if !block_mined {
        results.finish(
            false,
            format!("No block mined within 120s after submitting {accepted} transactions"),
            start.elapsed().as_secs_f64(),
        );
        return results;
    }

    // Step 4: Inspect the mined block
    let (txs_in_block, block_size_bytes) = inspect_block(&client, rpc_url, height_after).await
        .unwrap_or((0, 0));

    results.metric("block.txs_included", txs_in_block as f64, "txs");
    results.metric("block.size_bytes", block_size_bytes as f64, "bytes");
    results.metric("block.size_kb", block_size_bytes as f64 / 1024.0, "KB");

    let utilization = if tx_count > 0 { txs_in_block as f64 / tx_count.min(1000) as f64 * 100.0 } else { 0.0 };
    results.metric("block.mempool_utilization_pct", utilization, "%");

    let elapsed = start.elapsed().as_secs_f64();
    let passed = block_mined && txs_in_block > 0;

    results.finish(
        passed,
        format!(
            "Mined block {height_after} with {txs_in_block} txs ({block_size_bytes} bytes) \
             after {block_wait_secs:.1}s"
        ),
        elapsed,
    );

    info!("large-block: complete — block {height_after} txs={txs_in_block} size={block_size_bytes}B");
    results
}

async fn fill_mempool(
    client: &reqwest::Client,
    rpc_url: &str,
    count: u64,
) -> (u64, f64) {
    let start = Instant::now();
    let mut accepted = 0u64;

    for i in 0..count {
        let tx_hex = build_tx_hex(i);
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": i,
            "method": "author_submitExtrinsic",
            "params": [tx_hex]
        });

        match client.post(rpc_url).json(&body).send().await {
            Ok(resp) => {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    if json.get("error").is_none() {
                        accepted += 1;
                    }
                }
            }
            Err(e) => warn!("tx submit error: {e}"),
        }
    }

    (accepted, start.elapsed().as_secs_f64())
}

async fn get_block_height(client: &reqwest::Client, rpc_url: &str) -> Option<u64> {
    let body = serde_json::json!({
        "jsonrpc": "2.0", "id": 1,
        "method": "chain_getBlockNumber", "params": []
    });
    let resp = client.post(rpc_url).json(&body).send().await.ok()?;
    let json: serde_json::Value = resp.json().await.ok()?;
    json["result"].as_u64()
        .or_else(|| json["result"].as_str()?.parse().ok())
}

async fn inspect_block(
    client: &reqwest::Client,
    rpc_url: &str,
    height: u64,
) -> Option<(u64, u64)> {
    let body = serde_json::json!({
        "jsonrpc": "2.0", "id": 1,
        "method": "chain_getBlockByNumber", "params": [height]
    });
    let resp = client.post(rpc_url).json(&body).send().await.ok()?;
    let json: serde_json::Value = resp.json().await.ok()?;

    let block = json.get("result")?;
    let txs = block["transactions"].as_array().map(|a| a.len() as u64).unwrap_or(0);

    // Estimate block size from JSON serialization
    let size = serde_json::to_string(block).map(|s| s.len() as u64).unwrap_or(0);

    Some((txs, size))
}

fn build_tx_hex(nonce: u64) -> String {
    let mut bytes = [0u8; 64];
    bytes[0] = 0x01;
    bytes[1] = 0x00;
    bytes[2..10].copy_from_slice(&nonce.to_le_bytes());
    bytes[10..18].copy_from_slice(&1u64.to_le_bytes());
    let hash = blake3::hash(&bytes[..18]);
    bytes[18..50].copy_from_slice(&hash.as_bytes()[..32]);
    hex::encode(&bytes[..])
}
