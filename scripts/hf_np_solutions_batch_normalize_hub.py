#!/usr/bin/env python3
"""
Download each `data/*.jsonl` from a Hub dataset, normalize rows (full `DatasetRecord` keys),
and upload back to the **same paths** (one commit per file by default).

Requires: `pip install huggingface_hub`
Auth: `HF_TOKEN` or `HUGGINGFACE_TOKEN` env, or `--token`.

Examples:
  export HF_TOKEN=hf_...
  python3 scripts/hf_np_solutions_batch_normalize_hub.py --dry-run
  python3 scripts/hf_np_solutions_batch_normalize_hub.py --limit 3
  python3 scripts/hf_np_solutions_batch_normalize_hub.py --sleep 1.0

Resume after an error:
  python3 scripts/hf_np_solutions_batch_normalize_hub.py --start-after data/data_1775805000.jsonl
"""

from __future__ import annotations

import argparse
import os
import sys
import tempfile
import time
from pathlib import Path

_SCRIPTS = Path(__file__).resolve().parent
if str(_SCRIPTS) not in sys.path:
    sys.path.insert(0, str(_SCRIPTS))

import hf_np_solutions_jsonl_common as jl  # noqa: E402


def _token(cli_token: str | None, *, required: bool) -> str | None:
    if cli_token:
        return cli_token
    for k in ("HF_TOKEN", "HUGGINGFACE_TOKEN"):
        v = os.environ.get(k)
        if v:
            return v.strip()
    if required:
        sys.exit("Set HF_TOKEN (or HUGGINGFACE_TOKEN) or pass --token")
    return None


def _normalize_file(in_path: Path, out_path: Path) -> tuple[int, int]:
    with in_path.open("r", encoding="utf-8") as inf, out_path.open("w", encoding="utf-8") as outf:
        return jl.run_stream(inf, outf)


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--repo", default="COINjecture/NP-Solutions", help="Hub dataset repo id")
    p.add_argument("--token", default=None, help="Hub token (else HF_TOKEN / HUGGINGFACE_TOKEN)")
    p.add_argument("--dry-run", action="store_true", help="List files and row counts only; no upload")
    p.add_argument("--limit", type=int, default=0, help="Process at most N files (0 = all)")
    p.add_argument("--sleep", type=float, default=0.25, help="Seconds between uploads (rate limit)")
    p.add_argument(
        "--start-after",
        default="",
        help="Skip files until this path (exclusive), e.g. data/data_1775805000.jsonl",
    )
    args = p.parse_args()
    token = _token(args.token, required=not args.dry_run)

    from huggingface_hub import HfApi, hf_hub_download

    api = HfApi(token=token) if token else HfApi()
    all_files = sorted(
        f
        for f in api.list_repo_files(args.repo, repo_type="dataset")
        if f.startswith("data/data_") and f.endswith(".jsonl")
    )
    if args.start_after:
        if args.start_after in all_files:
            idx = all_files.index(args.start_after)
            all_files = all_files[idx + 1 :]
        else:
            all_files = [f for f in all_files if f > args.start_after]

    if args.limit and args.limit > 0:
        all_files = all_files[: args.limit]

    print(f"repo={args.repo} files={len(all_files)} dry_run={args.dry_run}", file=sys.stderr)
    if not all_files:
        print("No files to process.", file=sys.stderr)
        return

    if args.dry_run:
        for i, rel in enumerate(all_files):
            print(f"[{i+1}/{len(all_files)}] {rel}", file=sys.stderr)
        print("Dry run only; no downloads/uploads.", file=sys.stderr)
        return

    token = _token(args.token, required=True)
    api = HfApi(token=token)

    for i, rel in enumerate(all_files):
        local = hf_hub_download(args.repo, rel, repo_type="dataset", token=token)
        in_path = Path(local)
        n_lines = sum(1 for _ in in_path.open("r", encoding="utf-8"))
        print(f"[{i+1}/{len(all_files)}] {rel} lines={n_lines}", file=sys.stderr)

        with tempfile.NamedTemporaryFile(
            mode="w",
            suffix=".jsonl",
            encoding="utf-8",
            delete=False,
        ) as tmp:
            tmp_path = Path(tmp.name)

        try:
            _normalize_file(in_path, tmp_path)
        except Exception:
            tmp_path.unlink(missing_ok=True)
            raise

        try:
            url = api.upload_file(
                path_or_fileobj=str(tmp_path),
                path_in_repo=rel,
                repo_id=args.repo,
                repo_type="dataset",
                token=token,
                commit_message=f"Normalize JSONL keys (full DatasetRecord columns): {rel}",
            )
            print(f"  uploaded {url}", file=sys.stderr)
        finally:
            tmp_path.unlink(missing_ok=True)

        if args.sleep > 0 and i + 1 < len(all_files):
            time.sleep(args.sleep)


if __name__ == "__main__":
    main()
