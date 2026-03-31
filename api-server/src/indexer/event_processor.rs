//! Processes confirmed block events into database writes.

use crate::sse::EventBroadcaster;
use crate::supabase::SupabaseClient;
use serde_json::{json, Value};
use std::sync::Arc;

pub struct EventProcessor {
    pub supabase: Arc<SupabaseClient>,
    pub broadcaster: Arc<EventBroadcaster>,
}

pub struct ProcessResult {
    pub height: u64,
    pub trades_finalized: usize,
    pub orders_updated: usize,
}

impl EventProcessor {
    /// Process a confirmed block — finalize trades and update marketplace state.
    pub async fn process_block(&self, block: &Value) -> Result<ProcessResult, String> {
        let height = block_height(block)?;
        let block_hash = block_hash(block);
        let parent_hash = parent_hash(block);
        let txs = transactions(block);
        let tx_count = txs.len();

        let block_row = json!({
            "height": height,
            "hash": block_hash.clone(),
            "parent_hash": parent_hash.clone(),
            "block_timestamp": block_timestamp(block),
            "miner": miner(block),
            "tx_count": tx_count,
            "work_score": work_score(block),
            "raw_header": raw_header(block),
            "raw_block": block,
        });

        self.supabase
            .upsert_rows("blocks", "height", json!([block_row]))
            .await
            .map_err(|e| format!("failed to upsert block {height}: {e}"))?;

        self.supabase
            .delete_rows(&format!("marketplace_block_events?block_height=eq.{height}"))
            .await
            .map_err(|e| format!("failed to clear block events at {height}: {e}"))?;
        self.supabase
            .delete_rows(&format!("solution_sets?block_height=eq.{height}"))
            .await
            .map_err(|e| format!("failed to clear solution sets at {height}: {e}"))?;
        self.supabase
            .delete_rows(&format!("block_transactions?block_height=eq.{height}"))
            .await
            .map_err(|e| format!("failed to clear block transactions at {height}: {e}"))?;

        let mut tx_rows = Vec::with_capacity(tx_count);
        let mut event_rows = Vec::new();
        let mut tx_hashes = Vec::with_capacity(tx_count);

        for (tx_index, tx) in txs.iter().enumerate() {
            let tx_hash = tx_hash(tx, &block_hash, height, tx_index);
            let tx_type = tx_type(tx);
            let signer = tx_signer(tx);
            tx_hashes.push(tx_hash.clone());

            tx_rows.push(json!({
                "block_height": height,
                "tx_index": tx_index,
                "tx_hash": tx_hash,
                "tx_type": tx_type,
                "signer": signer,
                "payload": tx,
            }));

            event_rows.extend(extract_marketplace_events(
                height,
                tx_index,
                &tx_hash,
                signer.as_deref(),
                tx,
            ));
        }

        if !tx_rows.is_empty() {
            self.supabase
                .insert_rows("block_transactions", Value::Array(tx_rows))
                .await
                .map_err(|e| format!("failed to insert block transactions at {height}: {e}"))?;
        }

        if !event_rows.is_empty() {
            self.supabase
                .insert_rows("marketplace_block_events", Value::Array(event_rows.clone()))
                .await
                .map_err(|e| format!("failed to insert marketplace events at {height}: {e}"))?;
        }

        if let Some(solution_set_row) = extract_solution_set_row(block, height, &block_hash) {
            self.supabase
                .insert_rows("solution_sets", Value::Array(vec![solution_set_row]))
                .await
                .map_err(|e| format!("failed to insert solution set at {height}: {e}"))?;
        }

        let trades_finalized = self.finalize_matching_trades(height, &tx_hashes).await?;
        self.refresh_dataset_products(height, tx_count, &event_rows, block).await?;

        // Record indexer metrics
        metrics::gauge!("coinjecture_indexer_height").set(height as f64);
        metrics::counter!("coinjecture_blocks_indexed_total").increment(1);

        tracing::info!(height, tx_count, trades_finalized, "Block indexed");

        Ok(ProcessResult {
            height,
            trades_finalized,
            orders_updated: 0,
        })
    }

    /// Roll back data above `fork_height` on a chain reorg.
    pub async fn handle_reorg(&self, fork_height: u64) -> Result<(), String> {
        tracing::warn!(fork_height, "Chain reorg — rolling back indexed data");
        metrics::counter!("coinjecture_reorg_events_total").increment(1);

        let body = serde_json::json!({ "is_finalized": false });
        self
            .supabase
            .patch_rows(
                &format!("trades?block_height=gt.{fork_height}"),
                body,
            )
            .await
            .map_err(|e| format!("failed to unfinalize trades above fork: {e}"))?;

        self.supabase
            .delete_rows(&format!(
                "marketplace_block_events?block_height=gt.{fork_height}"
            ))
            .await
            .map_err(|e| format!("failed to delete reorged marketplace events: {e}"))?;
        self.supabase
            .delete_rows(&format!("solution_sets?block_height=gt.{fork_height}"))
            .await
            .map_err(|e| format!("failed to delete reorged solution sets: {e}"))?;
        self.supabase
            .delete_rows(&format!("block_transactions?block_height=gt.{fork_height}"))
            .await
            .map_err(|e| format!("failed to delete reorged transactions: {e}"))?;
        self.supabase
            .delete_rows(&format!("blocks?height=gt.{fork_height}"))
            .await
            .map_err(|e| format!("failed to delete reorged blocks: {e}"))?;

        Ok(())
    }

    pub async fn find_fork_height(&self, parent_hash: &str) -> Result<u64, String> {
        if parent_hash.is_empty() {
            return Ok(0);
        }

        let rows = self
            .supabase
            .postgrest_get_public(&format!(
                "blocks?hash=eq.{parent_hash}&select=height&limit=1"
            ))
            .await
            .map_err(|e| format!("failed to look up fork height: {e}"))?;

        Ok(rows
            .as_array()
            .and_then(|rows| rows.first())
            .and_then(|row| row["height"].as_u64())
            .unwrap_or(0))
    }

    pub async fn get_block_hash_at_height(&self, height: u64) -> Result<String, String> {
        if height == 0 {
            return Ok(String::new());
        }

        let rows = self
            .supabase
            .postgrest_get_public(&format!(
                "blocks?height=eq.{height}&select=hash&limit=1"
            ))
            .await
            .map_err(|e| format!("failed to look up block hash: {e}"))?;

        Ok(rows
            .as_array()
            .and_then(|rows| rows.first())
            .and_then(|row| row["hash"].as_str())
            .unwrap_or("")
            .to_string())
    }

    async fn finalize_matching_trades(
        &self,
        height: u64,
        tx_hashes: &[String],
    ) -> Result<usize, String> {
        let mut finalized = 0;

        for tx_hash in tx_hashes {
            let body = json!({
                "is_finalized": true,
                "block_height": height,
            });

            if self
                .supabase
                .patch_rows(&format!("trades?on_chain_tx_hash=eq.{tx_hash}"), body)
                .await
                .is_ok()
            {
                finalized += 1;
            }
        }

        Ok(finalized)
    }

    async fn refresh_dataset_products(
        &self,
        height: u64,
        tx_count: usize,
        event_rows: &[Value],
        block: &Value,
    ) -> Result<(), String> {
        let snapshot_specs = vec![
            (
                "marketplace-events-by-block",
                "marketplace_events",
                "Marketplace Events By Block",
                "Normalized marketplace events extracted from confirmed blocks.",
                event_rows.len() as u64,
            ),
            (
                "problem-submissions-and-solutions",
                "problem_activity",
                "Problem Submissions And Solutions",
                "Problem submission and solution activity from the PoUW marketplace.",
                count_events(event_rows, &["submit_problem", "submit_solution"]),
            ),
            (
                "bounty-payout-history",
                "bounty_payouts",
                "Bounty Payout History",
                "Claimed bounty events suitable for historical payout analysis.",
                count_events(event_rows, &["claim_bounty"]),
            ),
            (
                "trading-and-liquidity-activity",
                "trading_activity",
                "Trading And Liquidity Activity",
                "Chain-backed liquidity and trading-related marketplace events.",
                count_events(event_rows, &["trade", "liquidity", "pool_swap"]),
            ),
            (
                "verified-solution-sets",
                "solution_sets",
                "Verified Solution Sets",
                "Block-linked NP problem and solution bundles with sortable quality and work metrics.",
                if block.get("solution_reveal").is_some() { 1 } else { 0 },
            ),
        ];

        for (slug, dataset_type, title, description, row_count) in snapshot_specs {
            let snapshot = self
                .supabase
                .upsert_rows(
                    "dataset_snapshots",
                    "slug,version",
                    json!({
                        "slug": slug,
                        "version": format!("h{height}"),
                        "dataset_type": dataset_type,
                        "title": title,
                        "description": description,
                        "start_height": height,
                        "end_height": height,
                        "row_count": row_count,
                        "manifest": {
                            "generated_by": "indexer",
                            "height": height,
                            "tx_count": tx_count,
                        },
                        "checksum": format!("{slug}:h{height}:{row_count}"),
                        "storage_path": format!("supabase://dataset_snapshots/{slug}/h{height}.json"),
                        "status": "ready",
                    }),
                )
                .await
                .map_err(|e| format!("failed to upsert dataset snapshot for {slug}: {e}"))?;

            let snapshot_id = snapshot
                .as_array()
                .and_then(|rows| rows.first())
                .and_then(|row| row["id"].as_str())
                .ok_or_else(|| format!("dataset snapshot id missing for {slug}"))?;

            self.supabase
                .patch_rows(
                    &format!("dataset_catalog?slug=eq.{slug}"),
                    json!({ "latest_snapshot_id": snapshot_id }),
                )
                .await
                .map_err(|e| format!("failed to update dataset catalog for {slug}: {e}"))?;
        }

        Ok(())
    }
}

fn block_height(block: &Value) -> Result<u64, String> {
    block["height"]
        .as_u64()
        .or_else(|| block["header"]["height"].as_u64())
        .ok_or_else(|| "block height missing".to_string())
}

fn block_hash(block: &Value) -> String {
    string_field(block, &["hash", "block_hash"])
        .or_else(|| string_field(&block["header"], &["hash"]))
        .or_else(|| hash_bytes_to_hex(&block["header"]["hash"]))
        .or_else(|| header_hash_from_json(&block["header"]))
        .unwrap_or_default()
}

fn parent_hash(block: &Value) -> String {
    string_field(block, &["parent_hash", "prev_hash"])
        .or_else(|| string_field(&block["header"], &["parent_hash", "prev_hash"]))
        .or_else(|| hash_bytes_to_hex(&block["header"]["prev_hash"]))
        .unwrap_or_default()
}

fn miner(block: &Value) -> Option<String> {
    string_field(block, &["miner"])
        .or_else(|| string_field(&block["header"], &["miner"]))
        .or_else(|| hash_bytes_to_hex(&block["header"]["miner"]))
}

fn block_timestamp(block: &Value) -> Option<String> {
    block["timestamp"]
        .as_i64()
        .or_else(|| block["header"]["timestamp"].as_i64())
        .and_then(|ts| chrono::DateTime::<chrono::Utc>::from_timestamp(ts, 0))
        .map(|dt| dt.to_rfc3339())
}

fn work_score(block: &Value) -> Option<f64> {
    block["work_score"]
        .as_f64()
        .or_else(|| block["header"]["work_score"].as_f64())
}

fn raw_header(block: &Value) -> Value {
    block.get("header").cloned().unwrap_or_else(|| {
        json!({
            "height": block["height"],
            "hash": block["hash"],
            "parent_hash": block["parent_hash"],
            "timestamp": block["timestamp"],
            "miner": block["miner"],
            "work_score": block["work_score"],
        })
    })
}

fn transactions(block: &Value) -> Vec<&Value> {
    block["transactions"]
        .as_array()
        .map(|txs| txs.iter().collect())
        .unwrap_or_default()
}

fn tx_hash(tx: &Value, block_hash: &str, height: u64, tx_index: usize) -> String {
    string_field(tx, &["hash", "tx_hash"]).unwrap_or_else(|| {
        if !block_hash.is_empty() {
            format!("{block_hash}:{tx_index}")
        } else {
            format!("{height}:{tx_index}")
        }
    })
}

fn tx_type(tx: &Value) -> String {
    if let Some(kind) = string_field(tx, &["type", "tx_type"]) {
        return kind;
    }

    tx.as_object()
        .and_then(|obj| obj.keys().next().cloned())
        .unwrap_or_else(|| "unknown".to_string())
}

fn tx_signer(tx: &Value) -> Option<String> {
    string_field(tx, &["from", "signer", "wallet_address"])
        .or_else(|| variant_value(tx).and_then(|inner| string_field(inner, &["from", "signer"])))
}

fn extract_marketplace_events(
    block_height: u64,
    tx_index: usize,
    tx_hash: &str,
    signer: Option<&str>,
    tx: &Value,
) -> Vec<Value> {
    let Some(marketplace_tx) = marketplace_tx(tx) else {
        return Vec::new();
    };

    let Some(operation) = marketplace_operation(marketplace_tx) else {
        return vec![json!({
            "block_height": block_height,
            "tx_hash": tx_hash,
            "tx_index": tx_index,
            "event_index": 0,
            "event_type": "marketplace_operation",
            "actor_wallet": signer,
            "event_payload": marketplace_tx,
        })];
    };

    let (event_type, payload) = enum_variant(operation)
        .unwrap_or_else(|| ("marketplace_operation".to_string(), operation.clone()));

    vec![json!({
        "block_height": block_height,
        "tx_hash": tx_hash,
        "tx_index": tx_index,
        "event_index": 0,
        "event_type": to_snake_case(&event_type),
        "problem_id": string_field(&payload, &["problem_id"]),
        "actor_wallet": signer,
        "amount": numeric_field(&payload, &["bounty", "amount"]),
        "event_payload": payload,
    })]
}

fn marketplace_tx(tx: &Value) -> Option<&Value> {
    if tx.get("Marketplace").is_some() {
        return tx.get("Marketplace");
    }

    if tx["type"].as_str() == Some("Marketplace") {
        return Some(tx);
    }

    None
}

fn marketplace_operation(tx: &Value) -> Option<&Value> {
    tx.get("operation").or_else(|| tx.get("MarketplaceOperation"))
}

fn variant_value(value: &Value) -> Option<&Value> {
    value
        .as_object()
        .and_then(|obj| if obj.len() == 1 { obj.values().next() } else { None })
}

fn enum_variant(value: &Value) -> Option<(String, Value)> {
    let obj = value.as_object()?;
    if obj.len() != 1 {
        return None;
    }

    let (key, inner) = obj.iter().next()?;
    Some((key.clone(), inner.clone()))
}

fn string_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key))
        .and_then(value_to_string)
}

fn value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.to_string()),
        Value::Array(values) if values.iter().all(|item| item.as_u64().is_some()) => {
            let bytes = values
                .iter()
                .map(|item| item.as_u64().unwrap_or_default() as u8)
                .collect::<Vec<_>>();
            Some(hex::encode(bytes))
        }
        _ => None,
    }
}

fn hash_bytes_to_hex(value: &Value) -> Option<String> {
    value_to_string(value)
}

fn header_hash_from_json(header: &Value) -> Option<String> {
    if header.is_null() {
        return None;
    }

    let bytes = serde_json::to_vec(header).ok()?;
    Some(hex::encode(blake3::hash(&bytes).as_bytes()))
}

fn numeric_field(value: &Value, keys: &[&str]) -> Option<f64> {
    keys.iter().find_map(|key| match value.get(*key) {
        Some(Value::Number(number)) => number.as_f64(),
        Some(Value::String(text)) => text.parse::<f64>().ok(),
        _ => None,
    })
}

fn to_snake_case(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for (idx, ch) in input.chars().enumerate() {
        if ch.is_uppercase() {
            if idx > 0 {
                out.push('_');
            }
            out.extend(ch.to_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

fn count_events(event_rows: &[Value], event_types: &[&str]) -> u64 {
    event_rows
        .iter()
        .filter(|row| {
            row["event_type"]
                .as_str()
                .map(|event_type| event_types.contains(&event_type))
                .unwrap_or(false)
        })
        .count() as u64
}

fn extract_solution_set_row(block: &Value, height: u64, block_hash: &str) -> Option<Value> {
    let solution_reveal = block.get("solution_reveal")?;
    let problem = solution_reveal.get("problem").cloned().unwrap_or(Value::Null);
    let solution = solution_reveal.get("solution").cloned().unwrap_or(Value::Null);
    let commitment = solution_reveal.get("commitment").cloned().unwrap_or(Value::Null);

    let (problem_type, problem_payload) =
        enum_variant(&problem).unwrap_or_else(|| ("unknown".to_string(), problem.clone()));
    let (solution_type, solution_payload) =
        enum_variant(&solution).unwrap_or_else(|| ("unknown".to_string(), solution.clone()));

    let solution_quality = block["header"]["solution_quality"]
        .as_f64()
        .or_else(|| block["solution_quality"].as_f64());
    let work_score = work_score(block);

    Some(json!({
        "block_height": height,
        "block_hash": block_hash,
        "problem_id": string_field(&problem_payload, &["problem_id"])
            .or_else(|| string_field(&commitment, &["problem_hash"])),
        "problem_type": problem_type,
        "solution_type": solution_type,
        "miner": miner(block),
        "work_score": work_score,
        "solve_time_us": block["header"]["solve_time_us"].as_u64().or_else(|| block["solve_time_us"].as_u64()),
        "verify_time_us": block["header"]["verify_time_us"].as_u64().or_else(|| block["verify_time_us"].as_u64()),
        "time_asymmetry_ratio": block["header"]["time_asymmetry_ratio"].as_f64().or_else(|| block["time_asymmetry_ratio"].as_f64()),
        "solution_quality": solution_quality,
        "complexity_weight": block["header"]["complexity_weight"].as_f64().or_else(|| block["complexity_weight"].as_f64()),
        "energy_estimate_joules": block["header"]["energy_estimate_joules"].as_f64().or_else(|| block["energy_estimate_joules"].as_f64()),
        "quality_band": quality_band(solution_quality),
        "raw_problem": problem_payload,
        "raw_solution": solution_payload,
        "raw_solution_reveal": solution_reveal,
    }))
}

fn quality_band(solution_quality: Option<f64>) -> &'static str {
    match solution_quality {
        Some(q) if q >= 0.95 => "elite",
        Some(q) if q >= 0.80 => "high",
        Some(q) if q >= 0.60 => "medium",
        Some(_) => "emerging",
        None => "unknown",
    }
}
