//! Build a unified wallet activity feed from Supabase chain index tables.

use crate::node_rpc::NodeRpcClient;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

/// Percent-encode a query parameter value (ASCII-safe for JSON + `cs.` prefix).
pub fn encode_uri_query_value(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for &b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

/// JSONB `@>` filter fragment: `Transfer.to` equals this address as a 32-byte JSON array.
pub fn transfer_to_contains_filter(addr_hex: &str) -> Result<Value, String> {
    let bytes = hex_to_32(addr_hex)?;
    let arr: Vec<Value> = bytes.iter().map(|b| Value::from(u64::from(*b))).collect();
    Ok(json!({ "Transfer": { "to": arr } }))
}

fn hex_to_32(addr_hex: &str) -> Result<[u8; 32], String> {
    let t = addr_hex.trim().trim_start_matches("0x");
    if t.len() != 64 {
        return Err("address must be 64 hex chars (32-byte pubkey)".into());
    }
    let v = hex::decode(t).map_err(|e| e.to_string())?;
    if v.len() != 32 {
        return Err("invalid address length".into());
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&v);
    Ok(out)
}

pub fn normalize_hex_address(addr: &str) -> Result<String, String> {
    let t = addr.trim().trim_start_matches("0x").to_ascii_lowercase();
    hex_to_32(&t)?;
    Ok(t)
}

/// Normalize optional hex stored in the index (lowercase 64-char pubkey hex).
pub fn normalize_optional_hex(value: Option<String>) -> Option<String> {
    value.and_then(|s| normalize_hex_address(&s).ok())
}

pub fn block_height_from_json(block: &Value) -> Option<u64> {
    block["height"]
        .as_u64()
        .or_else(|| block["header"]["height"].as_u64())
}

pub fn block_hash_from_json(block: &Value) -> String {
    json_string_field(block, &["hash", "block_hash"])
        .or_else(|| json_string_field(&block["header"], &["hash"]))
        .or_else(|| json_value_to_hex(&block["header"]["hash"]))
        .unwrap_or_default()
}

pub fn parent_hash_from_json(block: &Value) -> String {
    json_string_field(block, &["parent_hash", "prev_hash"])
        .or_else(|| json_string_field(&block["header"], &["parent_hash", "prev_hash"]))
        .or_else(|| json_value_to_hex(&block["header"]["prev_hash"]))
        .unwrap_or_default()
}

pub fn miner_from_json(block: &Value) -> Option<String> {
    normalize_optional_hex(
        json_string_field(block, &["miner"])
            .or_else(|| json_string_field(&block["header"], &["miner"]))
            .or_else(|| json_value_to_hex(&block["header"]["miner"]))
            .or_else(|| {
                block
                    .get("coinbase")
                    .and_then(|cb| json_value_to_hex(&cb["to"]))
            }),
    )
}

/// Scan blocks via node RPC when Supabase index is empty or unavailable.
/// `max_blocks`: number of blocks to walk from tip; **0 = entire chain (genesis → tip)**.
pub async fn scan_wallet_activity_from_chain(
    node_rpc: &Arc<NodeRpcClient>,
    addr: &str,
    page_limit: usize,
    page_offset: usize,
    max_blocks: u64,
) -> Result<Value, String> {
    let chain_info = node_rpc
        .get_chain_info()
        .await
        .map_err(|e| format!("chain info: {e}"))?;
    let tip = chain_info["best_height"].as_u64().unwrap_or(0);
    if tip == 0 {
        return Ok(Value::Array(vec![]));
    }

    let scan = if max_blocks == 0 {
        tip.saturating_add(1)
    } else {
        max_blocks.max(1).min(tip)
    };
    let start = tip.saturating_sub(scan - 1);

    let mut signed_rows = Vec::new();
    let mut incoming_rows = Vec::new();
    let mut mined_rows = Vec::new();
    let mut marketplace_rows = Vec::new();

    let mut heights: Vec<u64> = (start..=tip).collect();
    heights.reverse();

    const CHUNK: usize = 32;
    for chunk in heights.chunks(CHUNK) {
        let mut handles = Vec::with_capacity(chunk.len());
        for &height in chunk {
            let rpc = Arc::clone(node_rpc);
            handles.push(tokio::spawn(async move {
                (height, rpc.get_block_by_height(height).await)
            }));
        }
        for handle in handles {
            let (height, block) = match handle.await {
                Ok((height, Ok(block))) => (height, block),
                _ => continue,
            };
            collect_wallet_rows_from_block(
                addr,
                height,
                &block,
                &mut signed_rows,
                &mut incoming_rows,
                &mut mined_rows,
                &mut marketplace_rows,
            );
        }
    }

    Ok(merge_wallet_activity(
        addr,
        page_limit,
        page_offset,
        Value::Array(signed_rows),
        Value::Array(incoming_rows),
        Value::Array(mined_rows),
        Value::Array(marketplace_rows),
    ))
}

fn collect_wallet_rows_from_block(
    addr: &str,
    height: u64,
    block: &Value,
    signed_rows: &mut Vec<Value>,
    incoming_rows: &mut Vec<Value>,
    mined_rows: &mut Vec<Value>,
    marketplace_rows: &mut Vec<Value>,
) {
    if let Some(miner) = miner_from_json(block) {
        if miner == addr {
            mined_rows.push(json!({
                "height": height,
                "hash": block_hash_from_json(block),
                "block_timestamp": block_timestamp_from_json(block),
                "miner": miner,
                "raw_block": block,
            }));
        }
    }

    let block_hash = block_hash_from_json(block);
    let txs = block["transactions"].as_array();
    let Some(txs) = txs else {
        return;
    };

    for (tx_index, tx) in txs.iter().enumerate() {
        let tx_type = tx_type_from_json(tx);
        let tx_hash = tx_hash_from_json(tx, &block_hash, height, tx_index);
        let signer = tx_signer_from_json(tx);

        if signer.as_deref() == Some(addr) {
            signed_rows.push(json!({
                "block_height": height,
                "tx_index": tx_index,
                "tx_hash": tx_hash,
                "tx_type": tx_type,
                "signer": signer,
                "payload": tx,
            }));
            marketplace_rows.extend(extract_marketplace_rows_for_wallet(
                height,
                tx_index,
                &tx_hash,
                signer.as_deref(),
                tx,
            ));
        }

        if tx_type == "Transfer" {
            let Some(inner) = transfer_inner(tx) else {
                continue;
            };
            let Some(from) = inner.get("from").and_then(json_addr_to_hex) else {
                continue;
            };
            let Some(to) = inner.get("to").and_then(json_addr_to_hex) else {
                continue;
            };
            if to == addr && from != addr {
                incoming_rows.push(json!({
                    "block_height": height,
                    "tx_index": tx_index,
                    "tx_hash": tx_hash,
                    "tx_type": tx_type,
                    "signer": signer,
                    "payload": tx,
                }));
            }
        }
    }
}

fn block_timestamp_from_json(block: &Value) -> Option<String> {
    block["timestamp"]
        .as_i64()
        .or_else(|| block["header"]["timestamp"].as_i64())
        .and_then(|ts| chrono::DateTime::<chrono::Utc>::from_timestamp(ts, 0))
        .map(|dt| dt.to_rfc3339())
}

fn tx_type_from_json(tx: &Value) -> String {
    if let Some(kind) = json_string_field(tx, &["type", "tx_type"]) {
        return kind;
    }
    tx.as_object()
        .and_then(|obj| obj.keys().next().cloned())
        .unwrap_or_else(|| "unknown".to_string())
}

fn tx_hash_from_json(tx: &Value, block_hash: &str, height: u64, tx_index: usize) -> String {
    json_string_field(tx, &["hash", "tx_hash"]).unwrap_or_else(|| {
        if !block_hash.is_empty() {
            format!("{block_hash}:{tx_index}")
        } else {
            format!("{height}:{tx_index}")
        }
    })
}

fn tx_signer_from_json(tx: &Value) -> Option<String> {
    normalize_optional_hex(
        json_string_field(tx, &["from", "signer", "wallet_address"]).or_else(|| {
            tx.as_object()
                .and_then(|obj| obj.values().next())
                .and_then(|inner| json_string_field(inner, &["from", "signer"]))
        }),
    )
}

fn json_string_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key))
        .and_then(json_value_to_hex)
}

fn json_value_to_hex(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => normalize_hex_address(text)
            .ok()
            .or_else(|| Some(text.to_ascii_lowercase())),
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

fn json_addr_to_hex(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => normalize_hex_address(s).ok(),
        Value::Array(arr) if arr.len() == 32 => {
            let mut b = [0u8; 32];
            for (i, x) in arr.iter().enumerate() {
                b[i] = x.as_u64()? as u8;
            }
            Some(hex::encode(b))
        }
        _ => None,
    }
}

fn transfer_inner(payload: &Value) -> Option<&Value> {
    payload.get("Transfer")
}

fn parse_u128_field(v: Option<&Value>) -> Option<String> {
    let v = v?;
    match v {
        Value::Number(n) => {
            if let Some(u) = n.as_u64() {
                return Some(u.to_string());
            }
            if let Some(i) = n.as_i64() {
                return Some(i.to_string());
            }
            n.to_string().parse::<u128>().ok().map(|x| x.to_string())
        }
        Value::String(s) => Some(s.clone()),
        _ => None,
    }
}

/// Merge signed txs, incoming transfers, mined blocks, and marketplace events.
pub fn merge_wallet_activity(
    addr: &str,
    page_limit: usize,
    page_offset: usize,
    signed: Value,
    incoming: Value,
    mined: Value,
    marketplace: Value,
) -> Value {
    let mut by_id: BTreeMap<String, Value> = BTreeMap::new();
    let mut used_tx: HashSet<String> = HashSet::new();

    // 1) Mining rewards
    if let Some(rows) = mined.as_array() {
        for row in rows {
            let Some(height) = row["height"].as_u64() else {
                continue;
            };
            let row_id = format!("mine:{height}");
            let reward = coinbase_reward_beans(&row["raw_block"]);
            let ts = row["block_timestamp"].as_str().unwrap_or("");
            let block_hash = row["hash"].as_str().unwrap_or("");
            by_id.insert(
                row_id.clone(),
                json!({
                    "id": row_id,
                    "kind": "mining_reward",
                    "label": "Block reward",
                    "block_height": height,
                    "block_timestamp": ts,
                    "tx_hash": null,
                    "tx_index": null,
                    "amount": reward,
                    "fee": null,
                    "counterparty": null,
                    "tx_type": "Coinbase",
                    "event_type": null,
                    "detail": { "block_hash": block_hash },
                }),
            );
        }
    }

    // 2) Marketplace / bounty events (richer than raw Marketplace tx row)
    if let Some(rows) = marketplace.as_array() {
        for row in rows {
            let height = row["block_height"].as_u64().unwrap_or(0);
            let txh = row["tx_hash"].as_str().unwrap_or("");
            let ev = row["event_index"].as_u64().unwrap_or(0);
            let row_id = format!("evt:{height}:{txh}:{ev}");
            let et = row["event_type"].as_str().unwrap_or("marketplace");
            let label = marketplace_event_label(et);
            by_id.insert(
                row_id.clone(),
                json!({
                    "id": row_id,
                    "kind": "marketplace",
                    "label": label,
                    "block_height": height,
                    "block_timestamp": null,
                    "tx_hash": txh,
                    "tx_index": row["tx_index"].as_u64(),
                    "amount": row["amount"],
                    "fee": null,
                    "counterparty": null,
                    "tx_type": "Marketplace",
                    "event_type": et,
                    "problem_id": row["problem_id"],
                    "detail": {
                        "problem_id": row["problem_id"],
                        "event_payload": row["event_payload"],
                    },
                }),
            );
        }
    }

    // 3) Signed transactions (skip raw Marketplace tx — covered by events)
    if let Some(rows) = signed.as_array() {
        for row in rows {
            let tx_type = row["tx_type"].as_str().unwrap_or("");
            if tx_type == "Marketplace" {
                continue;
            }
            let Some(tx_hash) = row["tx_hash"].as_str() else {
                continue;
            };
            used_tx.insert(tx_hash.to_string());

            let height = row["block_height"].as_u64().unwrap_or(0);
            let tx_index = row["tx_index"].as_u64().unwrap_or(0);
            let payload = &row["payload"];

            let item = if tx_type == "Transfer" {
                let inner = match transfer_inner(payload) {
                    Some(i) => i,
                    None => continue,
                };
                let Some(from) = inner.get("from").and_then(json_addr_to_hex) else {
                    continue;
                };
                let Some(to) = inner.get("to").and_then(json_addr_to_hex) else {
                    continue;
                };
                let amount = parse_u128_field(inner.get("amount"));
                let fee = parse_u128_field(inner.get("fee"));
                let (kind, label, counterparty) = if from == addr && to == addr {
                    (
                        "self_transfer",
                        "Self-transfer",
                        Value::String(addr.to_string()),
                    )
                } else if from == addr {
                    ("send", "Sent BEANS", truncate_hex(&to, 12))
                } else {
                    continue;
                };
                json!({
                    "id": format!("tx:{tx_hash}"),
                    "kind": kind,
                    "label": label,
                    "block_height": height,
                    "block_timestamp": null,
                    "tx_hash": tx_hash,
                    "tx_index": tx_index,
                    "amount": amount,
                    "fee": fee,
                    "counterparty": counterparty,
                    "tx_type": tx_type,
                    "event_type": null,
                    "detail": { "from": from, "to": to },
                })
            } else {
                json!({
                    "id": format!("tx:{tx_hash}"),
                    "kind": "chain_tx",
                    "label": humanize_tx_type(tx_type),
                    "block_height": height,
                    "block_timestamp": null,
                    "tx_hash": tx_hash,
                    "tx_index": tx_index,
                    "amount": null,
                    "fee": null,
                    "counterparty": null,
                    "tx_type": tx_type,
                    "event_type": null,
                    "detail": payload,
                })
            };

            by_id.insert(format!("tx:{tx_hash}"), item);
        }
    }

    // 4) Incoming transfers (receiver not necessarily signer)
    if let Some(rows) = incoming.as_array() {
        for row in rows {
            let Some(tx_hash) = row["tx_hash"].as_str() else {
                continue;
            };
            if used_tx.contains(tx_hash) {
                continue;
            }
            let inner = match transfer_inner(&row["payload"]) {
                Some(i) => i,
                None => continue,
            };
            let Some(from) = inner.get("from").and_then(json_addr_to_hex) else {
                continue;
            };
            let Some(to) = inner.get("to").and_then(json_addr_to_hex) else {
                continue;
            };
            if to != addr {
                continue;
            }
            let height = row["block_height"].as_u64().unwrap_or(0);
            let tx_index = row["tx_index"].as_u64().unwrap_or(0);
            let amount = parse_u128_field(inner.get("amount"));
            let fee = parse_u128_field(inner.get("fee"));
            by_id.insert(
                format!("tx:{tx_hash}"),
                json!({
                    "id": format!("tx:{tx_hash}"),
                    "kind": "receive",
                    "label": "Received BEANS",
                    "block_height": height,
                    "block_timestamp": null,
                    "tx_hash": tx_hash,
                    "tx_index": tx_index,
                    "amount": amount,
                    "fee": fee,
                    "counterparty": truncate_hex(&from, 12),
                    "tx_type": "Transfer",
                    "event_type": null,
                    "detail": { "from": from, "to": to },
                }),
            );
        }
    }

    // Sort: block height desc, then stable id for tie-break
    let mut entries: Vec<(u64, String, Value)> = by_id
        .into_iter()
        .map(|(k, v)| {
            let h = v["block_height"].as_u64().unwrap_or(0);
            (h, k, v)
        })
        .collect();
    entries.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));

    let take = if page_limit == 0 {
        0
    } else if page_limit == usize::MAX {
        usize::MAX
    } else {
        page_limit
    };
    let out: Vec<Value> = entries
        .into_iter()
        .map(|(_, _, v)| v)
        .skip(page_offset)
        .take(take)
        .collect();
    Value::Array(out)
}

fn extract_marketplace_rows_for_wallet(
    block_height: u64,
    tx_index: usize,
    tx_hash: &str,
    signer: Option<&str>,
    tx: &Value,
) -> Vec<Value> {
    let Some(marketplace_tx) = marketplace_tx_body(tx) else {
        return Vec::new();
    };

    let Some(operation) = marketplace_tx
        .get("operation")
        .or_else(|| marketplace_tx.get("MarketplaceOperation"))
    else {
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

    let (event_type, payload) = enum_variant_name(operation)
        .unwrap_or_else(|| ("marketplace_operation".to_string(), operation.clone()));

    vec![json!({
        "block_height": block_height,
        "tx_hash": tx_hash,
        "tx_index": tx_index,
        "event_index": 0,
        "event_type": snake_case_event_type(&event_type),
        "problem_id": json_string_field(&payload, &["problem_id"]),
        "actor_wallet": signer,
        "amount": marketplace_amount(&payload),
        "event_payload": payload,
    })]
}

fn marketplace_tx_body(tx: &Value) -> Option<&Value> {
    if tx.get("Marketplace").is_some() {
        return tx.get("Marketplace");
    }
    if tx["type"].as_str() == Some("Marketplace") {
        return Some(tx);
    }
    None
}

fn enum_variant_name(value: &Value) -> Option<(String, Value)> {
    let obj = value.as_object()?;
    if obj.len() != 1 {
        return None;
    }
    let (key, inner) = obj.iter().next()?;
    Some((key.clone(), inner.clone()))
}

fn snake_case_event_type(name: &str) -> String {
    name.chars()
        .fold(String::new(), |mut acc, c| {
            if c.is_uppercase() && !acc.is_empty() {
                acc.push('_');
            }
            acc.push(c.to_ascii_lowercase());
            acc
        })
}

fn marketplace_amount(payload: &Value) -> Option<Value> {
    payload
        .get("bounty")
        .or_else(|| payload.get("amount"))
        .cloned()
}

fn truncate_hex(h: &str, keep: usize) -> Value {
    if h.len() <= keep + 6 {
        return Value::String(h.to_string());
    }
    Value::String(format!("{}…{}", &h[..keep], &h[h.len() - 6..]))
}

fn humanize_tx_type(t: &str) -> String {
    let s = t.replace('_', " ");
    let mut c = s.chars();
    if let Some(f) = c.next() {
        return f.to_uppercase().collect::<String>() + c.as_str();
    }
    s
}

fn marketplace_event_label(event_type: &str) -> String {
    match event_type {
        "submit_problem" => "Bounty / problem".into(),
        "submit_solution" => "Solution submitted".into(),
        "place_order" => "Order placed".into(),
        "cancel_order" => "Order cancelled".into(),
        _ => humanize_tx_type(event_type),
    }
}

fn coinbase_reward_beans(raw_block: &Value) -> Option<String> {
    let reward = raw_block.get("coinbase")?.get("reward")?;
    parse_u128_field(Some(reward))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transfer_filter_roundtrip() {
        let addr = "01".repeat(32);
        let v = transfer_to_contains_filter(&addr).unwrap();
        assert!(v.get("Transfer").unwrap().get("to").unwrap().is_array());
    }
}
