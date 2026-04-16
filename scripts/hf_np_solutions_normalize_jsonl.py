#!/usr/bin/env python3
"""
Normalize COINjecture/NP-Solutions JSONL rows for Hugging Face Data Studio / `datasets`.

Legacy rows omitted optional keys (`serde(skip_serializing_if)`), so different shards
inferred different Arrow columns → CastError. This script adds every key that exists on
`DatasetRecord` in `huggingface/src/client.rs`, using JSON null (or small defaults) when
a key was missing. Unknown keys are dropped so the schema stays stable.

Keep RECORD_KEYS in sync when `DatasetRecord` changes.

Usage (single file):
  python3 scripts/hf_np_solutions_normalize_jsonl.py --in data/data_123.jsonl --out data/data_123.norm.jsonl

Streaming:
  python3 scripts/hf_np_solutions_normalize_jsonl.py < in.jsonl > out.jsonl

Bulk local (then upload with `hf upload` or hub API):
  for f in data/data_*.jsonl; do
    python3 scripts/hf_np_solutions_normalize_jsonl.py --in "$f" --out "${f%.jsonl}.norm.jsonl"
  done
"""

from __future__ import annotations

import argparse
import json
import sys
from typing import Any, Dict, Mapping, TextIO

# Must match field names on `DatasetRecord` in `huggingface/src/client.rs` (serde).
RECORD_KEYS: tuple[str, ...] = (
    "problem_id",
    "problem_type",
    "problem_data",
    "solution_data",
    "block_height",
    "timestamp",
    "block_hash",
    "prev_block_hash",
    "submitter",
    "solver",
    "work_score",
    "solution_quality",
    "problem_complexity",
    "bounty",
    "solve_time_us",
    "verify_time_us",
    "block_time_seconds",
    "mining_attempts",
    "time_asymmetry",
    "space_asymmetry",
    "energy_asymmetry",
    "solve_memory_bytes",
    "verify_memory_bytes",
    "peak_memory_bytes",
    "solve_energy_joules",
    "verify_energy_joules",
    "total_energy_joules",
    "energy_per_operation",
    "energy_efficiency",
    "peer_count",
    "propagation_time_ms",
    "sync_lag_blocks",
    "difficulty_target",
    "nonce",
    "hash_rate_estimate",
    "chain_work",
    "transaction_count",
    "block_size_bytes",
    "block_reward",
    "total_fees",
    "pool_distributions",
    "cpu_model",
    "cpu_cores",
    "cpu_threads",
    "ram_total_bytes",
    "os_info",
    "status",
    "submission_mode",
    "energy_measurement_method",
    "metrics_source",
    "measurement_confidence",
    "data_version",
    "node_version",
    "node_id",
    "explorer_card",
)

_KEY_SET = frozenset(RECORD_KEYS)


def _coerce_json_object(name: str, value: Any) -> Any:
    """Match Rust `coerce_json_object_for_hub`: object in JSON, or stringified JSON → object."""
    if value is None:
        return None
    if isinstance(value, dict):
        return value
    if isinstance(value, str):
        s = value.strip()
        if not s:
            return {}
        try:
            parsed = json.loads(s)
        except json.JSONDecodeError:
            return {"_legacy_unparsed": value}
        if isinstance(parsed, dict):
            return parsed
        if isinstance(parsed, list):
            return {"items": parsed}
        return {"value": parsed}
    if isinstance(value, list):
        return {"items": value}
    return {"value": value}


def normalize_record(row: Mapping[str, Any]) -> Dict[str, Any]:
    out: Dict[str, Any] = {k: None for k in RECORD_KEYS}
    for k, v in row.items():
        if k not in _KEY_SET:
            continue
        out[k] = v

    out["problem_data"] = _coerce_json_object("problem_data", out.get("problem_data"))
    if out["problem_data"] is None:
        out["problem_data"] = {}

    sd = out.get("solution_data")
    if sd is not None:
        out["solution_data"] = _coerce_json_object("solution_data", sd)

    if out.get("explorer_card") is None:
        out["explorer_card"] = ""

    return out


def run(in_fp: TextIO, out_fp: TextIO) -> tuple[int, int]:
    n_in = 0
    n_out = 0
    for line in in_fp:
        n_in += 1
        line = line.strip()
        if not line:
            continue
        try:
            row = json.loads(line)
        except json.JSONDecodeError as e:
            print(f"skip invalid json line {n_in}: {e}", file=sys.stderr)
            continue
        if not isinstance(row, dict):
            print(f"skip non-object line {n_in}", file=sys.stderr)
            continue
        norm = normalize_record(row)
        out_fp.write(json.dumps(norm, separators=(",", ":"), ensure_ascii=False) + "\n")
        n_out += 1
    return n_in, n_out


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--in", dest="in_path", default="-", help="Input JSONL path (default: stdin)")
    p.add_argument("--out", dest="out_path", default="-", help="Output JSONL path (default: stdout)")
    args = p.parse_args()

    in_fp: TextIO
    out_fp: TextIO
    if args.in_path == "-":
        in_fp = sys.stdin
    else:
        in_fp = open(args.in_path, "r", encoding="utf-8")

    if args.out_path == "-":
        out_fp = sys.stdout
    else:
        out_fp = open(args.out_path, "w", encoding="utf-8")

    try:
        n_in, n_out = run(in_fp, out_fp)
    finally:
        if args.in_path != "-":
            in_fp.close()
        if args.out_path != "-":
            out_fp.close()

    print(f"normalized {n_out} records ({n_in} non-empty lines read)", file=sys.stderr)


if __name__ == "__main__":
    main()
