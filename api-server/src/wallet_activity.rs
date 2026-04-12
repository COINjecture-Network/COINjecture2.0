//! Build a unified wallet activity feed from Supabase chain index tables.

use serde_json::{json, Value};
use std::collections::{BTreeMap, HashSet};

/// Percent-encode a query parameter value (ASCII-safe for JSON + `cs.` prefix).
pub fn encode_uri_query_value(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for &b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

/// JSONB `@>` filter fragment: `Transfer.to` equals this address as a 32-byte JSON array.
pub fn transfer_to_contains_filter(addr_hex: &str) -> Result<Value, String> {
    let bytes = hex_to_32(addr_hex)?;
    let arr: Vec<Value> = bytes
        .iter()
        .map(|b| Value::from(u64::from(*b)))
        .collect();
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
    limit: usize,
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
                    (
                        "send",
                        "Sent BEANS",
                        truncate_hex(&to, 12),
                    )
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

    let out: Vec<Value> = entries
        .into_iter()
        .map(|(_, _, v)| v)
        .take(limit)
        .collect();
    Value::Array(out)
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
