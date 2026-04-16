#!/usr/bin/env python3
"""
Emit `huggingface/dataset_infos.json` for Hub upload (dataset repo **root**).

The HF Data Studio worker can infer a schema from early shards that omits columns
present in later JSONL, which triggers `CastError: ... column names don't match`.
A root-level `dataset_infos.json` (legacy but still read by `datasets.load_dataset`)
pins the full column set and types (`Json` for `problem_data` / `solution_data` /
`pool_distributions`) so the target schema matches `DatasetRecord`.

Keep `FEATURE_DTYPES` aligned with `huggingface/src/client.rs` and
`scripts/hf_np_solutions_jsonl_common.py` (`RECORD_KEYS`).

Usage:
  python3 scripts/hf_np_solutions_emit_dataset_infos.py
  python3 scripts/hf_np_solutions_emit_dataset_infos.py --output /tmp/dataset_infos.json
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

import hf_np_solutions_jsonl_common as jl


def _value(dtype: str) -> dict:
    return {"dtype": dtype, "_type": "Value"}


def _json() -> dict:
    return {"_type": "Json"}


# dtypes for Hub `Features.from_dict` (datasets >= 4.6 on the Hub side).
FEATURE_DTYPES: dict[str, dict] = {
    "problem_id": _value("string"),
    "problem_type": _value("string"),
    "problem_data": _json(),
    "solution_data": _json(),
    "block_height": _value("int64"),
    "timestamp": _value("int64"),
    "block_hash": _value("string"),
    "prev_block_hash": _value("string"),
    "submitter": _value("string"),
    "solver": _value("string"),
    "work_score": _value("float64"),
    "solution_quality": _value("float64"),
    "problem_complexity": _value("float64"),
    "bounty": _value("string"),
    "solve_time_us": _value("int64"),
    "verify_time_us": _value("int64"),
    "block_time_seconds": _value("float64"),
    "mining_attempts": _value("int64"),
    "time_asymmetry": _value("float64"),
    "space_asymmetry": _value("float64"),
    "energy_asymmetry": _value("float64"),
    "solve_memory_bytes": _value("int64"),
    "verify_memory_bytes": _value("int64"),
    "peak_memory_bytes": _value("int64"),
    "solve_energy_joules": _value("float64"),
    "verify_energy_joules": _value("float64"),
    "total_energy_joules": _value("float64"),
    "energy_per_operation": _value("float64"),
    "energy_efficiency": _value("float64"),
    "peer_count": _value("int64"),
    "propagation_time_ms": _value("int64"),
    "sync_lag_blocks": _value("int64"),
    "difficulty_target": _value("int64"),
    "nonce": _value("int64"),
    "hash_rate_estimate": _value("float64"),
    "chain_work": _value("float64"),
    "transaction_count": _value("int64"),
    "block_size_bytes": _value("int64"),
    "block_reward": _value("string"),
    "total_fees": _value("string"),
    "pool_distributions": _json(),
    "cpu_model": _value("string"),
    "cpu_cores": _value("int64"),
    "cpu_threads": _value("int64"),
    "ram_total_bytes": _value("int64"),
    "os_info": _value("string"),
    "status": _value("string"),
    "submission_mode": _value("string"),
    "energy_measurement_method": _value("string"),
    "metrics_source": _value("string"),
    "measurement_confidence": _value("string"),
    "data_version": _value("string"),
    "node_version": _value("string"),
    "node_id": _value("string"),
    "explorer_card": _value("string"),
}


def build_dataset_infos() -> dict:
    keys = tuple(jl.RECORD_KEYS)
    missing = [k for k in keys if k not in FEATURE_DTYPES]
    extra = [k for k in FEATURE_DTYPES if k not in set(keys)]
    if missing or extra:
        raise SystemExit(
            f"RECORD_KEYS / FEATURE_DTYPES mismatch: missing={missing!r} extra={extra!r}"
        )
    features = {k: FEATURE_DTYPES[k] for k in keys}
    inner: dict = {
        "description": "COINjecture NP-Solutions: JSONL rows aligned with `DatasetRecord` (see repo huggingface/).",
        "citation": "",
        "homepage": "",
        "license": "",
        "features": features,
        "builder_name": "json",
        "dataset_name": "np_solutions",
        "config_name": "default",
        "version": {"version_str": "0.0.0", "major": 0, "minor": 0, "patch": 0},
        "splits": {
            "train": {
                "name": "train",
                "num_bytes": 0,
                "num_examples": 0,
                "dataset_name": "np_solutions",
            }
        },
    }
    return {"default": inner}


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument(
        "--output",
        type=Path,
        default=Path(__file__).resolve().parents[1] / "huggingface" / "dataset_infos.json",
        help="Write path (default: repo huggingface/dataset_infos.json)",
    )
    args = ap.parse_args()
    payload = build_dataset_infos()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
    print(f"Wrote {args.output}")


if __name__ == "__main__":
    main()
