//! Precomputed human-readable block card for JSONL (`explorer_card` field).
//! Uses absolute UTC time (not "Ns ago") so archived rows stay meaningful.

use crate::client::DatasetRecord;
use serde_json::Value;

/// Render the explorer-style card from a fully populated [`DatasetRecord`].
pub fn render(record: &DatasetRecord) -> String {
    let time_line = chrono::DateTime::from_timestamp(record.timestamp, 0)
        .map(|dt| dt.format("Time (UTC): %Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| format!("Time: unix {}", record.timestamp));

    let solver_hex = record.solver.as_deref().or(record.submitter.as_deref());
    let solver_line = addr_short(solver_hex).unwrap_or_else(|| "—".to_string());

    let reward = format_u128_commas(record.bounty);

    let work = record
        .work_score
        .map(|w| format!("{w:.3}"))
        .unwrap_or_else(|| "—".to_string());

    let asym = record
        .time_asymmetry
        .map(|a| format!("{a:.2}×"))
        .unwrap_or_else(|| "—".to_string());

    let quality = record
        .solution_quality
        .map(|q| format!("{q:.3}"))
        .unwrap_or_else(|| "—".to_string());

    let su = record.solve_time_us.unwrap_or(0);
    let vu = record.verify_time_us.unwrap_or(0);
    let dt_line = format!(
        "Δt: {:.1} ms solve / {:.2} ms verify",
        su as f64 / 1000.0,
        vu as f64 / 1000.0
    );

    let type_line = record.problem_type.trim().to_string();
    let mut lines: Vec<String> = vec![
        format!("Block #{}", record.block_height),
        type_line.clone(),
        time_line,
        String::new(),
        "Problem:".to_string(),
        problem_line(&type_line, &record.problem_data),
        String::new(),
        "Solution:".to_string(),
        solution_line(
            &type_line,
            &record.problem_data,
            record.solution_data.as_ref(),
        ),
        String::new(),
        format!("Solver: {solver_line}"),
        String::new(),
        "Reward (BEANS)".to_string(),
        reward,
        String::new(),
        "Work (bits)".to_string(),
        work,
        String::new(),
        "Asymmetry".to_string(),
        asym,
        String::new(),
        "Quality".to_string(),
        quality,
        String::new(),
        dt_line,
    ];

    if let Some(ej) = record.total_energy_joules {
        lines.push(format!("Est. energy: {ej:.1} J"));
    }

    lines.join("\n")
}

fn addr_short(hex: Option<&str>) -> Option<String> {
    let h = hex?;
    if h.len() < 16 {
        return Some(h.to_string());
    }
    Some(format!("{}...{}", &h[..8], &h[h.len().saturating_sub(6)..]))
}

fn format_u128_commas(n: u128) -> String {
    let s = n.to_string();
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    let mut result = String::new();
    for (i, c) in chars.iter().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            result.push(',');
        }
        result.push(*c);
    }
    result
}

fn problem_line(problem_type: &str, pd: &Value) -> String {
    let pt = problem_type.trim();
    if pt == "SAT" || pt.starts_with("SAT") {
        let n_clauses = pd
            .get("clauses")
            .and_then(|c| c.as_array())
            .map(|a| a.len())
            .unwrap_or(0);
        let n_vars = pd.get("variables").and_then(json_u64).unwrap_or(0);
        return format!("Satisfy {n_clauses} clauses with {n_vars} variables");
    }
    if pt == "SubsetSum" || pt.starts_with("SubsetSum") {
        if let Some(nums) = pd.get("numbers").and_then(|v| v.as_array()) {
            let tgt = pd.get("target").map(value_to_string).unwrap_or_default();
            let nums_s: Vec<String> = nums.iter().map(value_to_string).collect();
            return format!(
                "Find subset summing to {tgt}\nValues: [{}]",
                nums_s.join(", ")
            );
        }
    }
    if pt == "TSP" || pt.starts_with("TSP") {
        let cities = pd
            .get("cities")
            .and_then(|c| c.as_array())
            .map(|a| a.len())
            .unwrap_or(0);
        return format!("TSP with {cities} cities (see problem_data)");
    }
    if pt == "Private" || pt.starts_with("Private") {
        return "Private problem (see problem_data if revealed)".to_string();
    }
    serde_json::to_string(pd).unwrap_or_else(|_| "{}".to_string())
}

fn solution_line(problem_type: &str, pd: &Value, sd: Option<&Value>) -> String {
    let Some(sd) = sd else {
        return "—".to_string();
    };
    let pt = problem_type.trim();
    if pt == "SAT" || pt.starts_with("SAT") {
        return "Satisfying assignment found".to_string();
    }
    if pt == "SubsetSum" || pt.starts_with("SubsetSum") {
        let idx: Vec<String> = sd
            .get("indices")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().map(value_to_string).collect())
            .unwrap_or_default();
        let tgt = pd
            .get("target")
            .map(value_to_string)
            .unwrap_or_else(|| "—".to_string());
        if idx.is_empty() {
            return "—".to_string();
        }
        return format!("Indices {} → Sum: {}", idx.join(", "), tgt);
    }
    if pt == "TSP" || pt.starts_with("TSP") {
        if let Some(tour) = sd.get("tour").and_then(|v| v.as_array()) {
            let n = tour.len();
            return format!("Tour length {n} (see solution_data)");
        }
    }
    serde_json::to_string(sd).unwrap_or_else(|_| "—".to_string())
}

fn json_u64(v: &Value) -> Option<u64> {
    v.as_u64().or_else(|| v.as_i64().map(|i| i as u64))
}

fn value_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
        _ => serde_json::to_string(v).unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sat_card_includes_clauses_line() {
        let clauses: Vec<Value> = (0..78).map(|_| json!({"literals": [1, -2]})).collect();
        let mut r = DatasetRecord {
            problem_id: "x".into(),
            problem_type: "SAT".into(),
            problem_data: json!({ "variables": 26, "clauses": clauses }),
            solution_data: Some(json!({"assignments": [1, 0]})),
            block_height: 123525,
            timestamp: 1_700_000_000,
            block_hash: None,
            prev_block_hash: None,
            submitter: None,
            solver: Some("74446bf9abcd00000000000000000000000000000000000000000000d77e15".into()),
            work_score: Some(12.432),
            solution_quality: Some(1.0),
            problem_complexity: 1.0,
            bounty: 124_324_271,
            solve_time_us: Some(28_600),
            verify_time_us: Some(10),
            block_time_seconds: None,
            mining_attempts: None,
            time_asymmetry: Some(5725.40),
            space_asymmetry: None,
            energy_asymmetry: None,
            solve_memory_bytes: None,
            verify_memory_bytes: None,
            peak_memory_bytes: None,
            solve_energy_joules: None,
            verify_energy_joules: None,
            total_energy_joules: Some(2.9),
            energy_per_operation: None,
            energy_efficiency: None,
            peer_count: None,
            propagation_time_ms: None,
            sync_lag_blocks: None,
            difficulty_target: None,
            nonce: None,
            hash_rate_estimate: None,
            chain_work: None,
            transaction_count: None,
            block_size_bytes: None,
            block_reward: None,
            total_fees: None,
            pool_distributions: None,
            cpu_model: None,
            cpu_cores: None,
            cpu_threads: None,
            ram_total_bytes: None,
            os_info: None,
            status: "Mined".into(),
            submission_mode: "mining".into(),
            energy_measurement_method: "Estimate".into(),
            metrics_source: "block_header_actual".into(),
            measurement_confidence: "high".into(),
            data_version: "v3.1".into(),
            node_version: None,
            node_id: None,
            explorer_card: String::new(),
        };
        r.explorer_card = render(&r);
        assert!(r.explorer_card.contains("Block #123525"));
        assert!(r
            .explorer_card
            .contains("Satisfy 78 clauses with 26 variables"));
        assert!(r.explorer_card.contains("74446bf9...d77e15"));
        assert!(r.explorer_card.contains("124,324,271"));
    }
}
