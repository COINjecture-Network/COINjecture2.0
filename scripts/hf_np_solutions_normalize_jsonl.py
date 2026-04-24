#!/usr/bin/env python3
"""
Normalize COINjecture/NP-Solutions JSONL rows for Hugging Face Data Studio / `datasets`.

Legacy rows omitted optional keys (`serde(skip_serializing_if)`), so different shards
inferred different Arrow columns → CastError. This script adds every key that exists on
`DatasetRecord` in `huggingface/src/client.rs`, using JSON null (or small defaults) when
a key was missing. Unknown keys are dropped so the schema stays stable.

Shared logic: `hf_np_solutions_jsonl_common.py`.

Usage (single file):
  python3 scripts/hf_np_solutions_normalize_jsonl.py --in data/data_123.jsonl --out data/data_123.norm.jsonl

Streaming:
  python3 scripts/hf_np_solutions_normalize_jsonl.py < in.jsonl > out.jsonl

Hub batch (download → normalize → upload):
  export HF_TOKEN=...
  python3 scripts/hf_np_solutions_batch_normalize_hub.py --dry-run
  python3 scripts/hf_np_solutions_batch_normalize_hub.py --limit 5
  python3 scripts/hf_np_solutions_batch_normalize_hub.py
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

# Allow `python3 scripts/foo.py` from repo root
_SCRIPTS = Path(__file__).resolve().parent
if str(_SCRIPTS) not in sys.path:
    sys.path.insert(0, str(_SCRIPTS))

import hf_np_solutions_jsonl_common as jl  # noqa: E402


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--in", dest="in_path", default="-", help="Input JSONL path (default: stdin)")
    p.add_argument("--out", dest="out_path", default="-", help="Output JSONL path (default: stdout)")
    args = p.parse_args()

    if args.in_path == "-":
        in_fp = sys.stdin
    else:
        in_fp = open(args.in_path, "r", encoding="utf-8")

    if args.out_path == "-":
        out_fp = sys.stdout
    else:
        out_fp = open(args.out_path, "w", encoding="utf-8")

    try:
        n_in, n_out = jl.run_stream(in_fp, out_fp)
    finally:
        if args.in_path != "-":
            in_fp.close()
        if args.out_path != "-":
            out_fp.close()

    print(f"normalized {n_out} records ({n_in} non-empty lines read)", file=sys.stderr)


if __name__ == "__main__":
    main()
