---
license: mit
task_categories:
- other
language:
- en
tags:
- blockchain
- proof-of-work
- np-complete
- optimization
- energy-measurement
- consensus
size_categories:
- 1K<n<10K
---

# COINjecture NP-Solutions Dataset

## Dataset Description

This dataset contains real-time blockchain data from the COINjecture Network, a proof-of-useful-work (PoUW) blockchain that uses NP-complete problems for consensus. This is a **unified, continuous dataset** that includes all problem types (SubsetSum, SAT, TSP, Custom) and consensus blocks in a single repository for comprehensive analysis.

### Dataset Summary

The COINjecture Network is a blockchain that replaces traditional proof-of-work mining with solving useful computational problems. This unified dataset captures:

- **Problem Submissions**: NP-complete problems (SubsetSum, SAT, TSP, Custom) submitted to the network
- **Solution Submissions**: Solutions to problems with verification metrics
- **Consensus Blocks**: Complete block data including transactions, PoUW metrics, and energy measurements

Records are produced by **network nodes** (see `coinject_huggingface::DatasetRecord` in this repo) and uploaded as JSONL to the Hub. They are **not** the same artifact as other exports (e.g. API index tables); use this dataset for raw node-emitted training and research corpora.

**All problem types are stored in a single continuous dataset** (`COINjecture/NP-Solutions`) to enable cross-problem-type analysis and unified research workflows.

### Supported Tasks

- **Research**: Study of NP-complete problem solving performance
- **Energy Analysis**: Energy consumption patterns in computational problem solving
- **Blockchain Analytics**: Consensus mechanism performance and transparency metrics
- **Machine Learning**: Training models on problem-solution pairs

### Languages

English (problem descriptions and metadata)

## Explorer-style layout (reference)

The JSONL rows are the source of truth; the layout below is the **recommended human-readable presentation** for explorers, dashboards, and docs. Times like `15s ago` are computed from `timestamp` (Unix seconds) relative to “now” when rendering.

### Example — SAT (consensus / mined block)

```text
Block #123525
SAT
15s ago

Problem:
Satisfy 78 clauses with 26 variables

Solution:
Satisfying assignment found

Solver: 74446bf9...d77e15

Reward (BEANS)
124,324,271

Work (bits)
12.432

Asymmetry
5725.40×

Quality
1.000

Δt: 28.6 ms solve / 0.01 ms verify
Est. energy: 2.9 J
```

### Example — SubsetSum (consensus / mined block)

```text
Block #123524
SubsetSum
20s ago

Problem:
Find subset summing to 3938
Values: [55, 683, 630, 222, 651, 376, 332, 38, 827, 191, 292, 485, 453, 744, 403, 283, 717, 823, 350, 55, 928, 967, 995, 384, 354, 979, 733, 488, 882, 708, 67, 309, 751, 831]

Solution:
Indices 9, 10, 18, 19, 23, 29, 30, 31, 32, 33 → Sum: 3938

Solver: 74446bf9...d77e15

Reward (BEANS)
110,826,140

Work (bits)
11.083

Asymmetry
2845.00×

Quality
1.000

Δt: 2.8 ms solve / 0.00 ms verify
Est. energy: (from total_energy_joules when present)
```

### Precomputed `explorer_card`

Current nodes set **`explorer_card`** on each emitted row to the same layout as below (UTC time line). For custom viewers you can print `record["explorer_card"]` directly, or rebuild from fields using the Python helper in this README.

### Line-by-line mapping (JSONL → display)

| Display | JSON fields / rule |
|--------|----------------------|
| **Block #…** | `block_height` |
| **Type line** (SAT, SubsetSum, …) | `problem_type` |
| **Relative time** | `timestamp` vs viewer clock (e.g. `format_relative(timestamp)`) |
| **Problem:** | Derived from `problem_data` by type (see below) |
| **Solution:** | Derived from `solution_data` + `problem_data` (see below) |
| **Solver:** | `solver` or `submitter` (hex); show as `first8...last6` for privacy |
| **Reward (BEANS)** | `bounty` (string u128) or formatted integer — native reward units on the network |
| **Work (bits)** | `work_score` when set; format with fixed decimals (e.g. 3) |
| **Asymmetry** | `time_asymmetry` (solve/verify time ratio); suffix `×` |
| **Quality** | `solution_quality` when set (0–1 scale) |
| **Δt:** | `solve_time_us`, `verify_time_us` → ms: `solve_time_us / 1000`, `verify_time_us / 1000` |
| **Est. energy** | `total_energy_joules` (or sum of solve/verify energy fields) with one decimal and ` J` |

**SAT — Problem line:** From `problem_data.clauses` length and `problem_data.variables` (or equivalent):  
`Satisfy {n_clauses} clauses with {n_vars} variables`.

**SAT — Solution line:** If `solution_data.assignments` exists: “Satisfying assignment found” (or list assignment preview for research dumps).

**SubsetSum — Problem line:** `Find subset summing to {problem_data.target}` plus `Values: {problem_data.numbers}` (truncate with “…” if extremely long).

**SubsetSum — Solution line:** `Indices {comma-separated} → Sum: {target}` where indices are `solution_data.indices` and target is `problem_data.target` (recompute sum for verification in tooling).

**TSP / Custom:** Use the same block header; problem/solution lines should summarize `problem_data` / `solution_data` (tour length, custom label) — extend the same pattern.

### Optional: Python sketch

```python
from __future__ import annotations

import time
from typing import Any, Mapping


def _rel_ago(ts: int) -> str:
    s = max(0, int(time.time()) - int(ts))
    if s < 60:
        return f"{s}s ago"
    if s < 3600:
        return f"{s // 60}m ago"
    return f"{s // 3600}h ago"


def _addr_short(hex64: str | None) -> str | None:
    if not hex64 or len(hex64) < 16:
        return hex64
    return f"{hex64[:8]}...{hex64[-6:]}"


def _fmt_int_string(s: str | None) -> str:
    if not s:
        return "—"
    try:
        return f"{int(s):,}"
    except ValueError:
        return s


def problem_line(pt: str, pd: Mapping[str, Any]) -> str:
    if pt == "SAT":
        n_c = len(pd.get("clauses") or [])
        n_v = int(pd.get("variables") or 0)
        return f"Satisfy {n_c} clauses with {n_v} variables"
    if pt == "SubsetSum":
        nums = pd.get("numbers") or []
        tgt = pd.get("target")
        return f"Find subset summing to {tgt}\nValues: {nums}"
    return str(pd)


def solution_line(pt: str, pd: Mapping[str, Any], sd: Mapping[str, Any] | None) -> str:
    if sd is None:
        return "—"
    if pt == "SAT":
        return "Satisfying assignment found"
    if pt == "SubsetSum":
        idx = sd.get("indices") or []
        tgt = pd.get("target")
        return f"Indices {', '.join(str(i) for i in idx)} → Sum: {tgt}"
    return str(sd)


def format_block_card(r: Mapping[str, Any]) -> str:
    pt = r.get("problem_type") or "?"
    pd = r.get("problem_data") or {}
    sd = r.get("solution_data")
    ws = r.get("work_score")
    ta = r.get("time_asymmetry")
    q = r.get("solution_quality")
    su = r.get("solve_time_us") or 0
    vu = r.get("verify_time_us") or 0
    ej = r.get("total_energy_joules")

    lines = [
        f"Block #{r.get('block_height', '?')}",
        str(pt),
        _rel_ago(int(r.get("timestamp") or 0)),
        "",
        "Problem:",
        problem_line(pt, pd),
        "",
        "Solution:",
        solution_line(pt, pd, sd),
        "",
        f"Solver: {_addr_short(r.get('solver') or r.get('submitter'))}",
        "",
        "Reward (BEANS)",
        _fmt_int_string(r.get("bounty")),
        "",
        "Work (bits)",
        f"{ws:.3f}" if isinstance(ws, (int, float)) else "—",
        "",
        "Asymmetry",
        f"{ta:.2f}×" if isinstance(ta, (int, float)) else "—",
        "",
        "Quality",
        f"{q:.3f}" if isinstance(q, (int, float)) else "—",
        "",
        f"Δt: {su / 1000:.1f} ms solve / {vu / 1000:.2f} ms verify",
    ]
    if isinstance(ej, (int, float)):
        lines.append(f"Est. energy: {ej:.1f} J")
    return "\n".join(lines)
```

## Dataset Structure

### Data Instances

Each record in the dataset represents either:
1. A problem submission (when a problem is submitted to the network)
2. A solution submission (when a solution is verified)
3. A consensus block (complete block data with all transactions)

### Data Fields

| Field | Type | Description |
|-------|------|-------------|
| **PRIMARY CONTENT** |||
| `problem_id` | string | Unique identifier for the problem |
| `problem_type` | string | Type of problem: "SubsetSum", "SAT", "TSP", "Custom", or "Private" |
| `problem_data` | object | Complete problem data. **Always a JSON object** in rows emitted by current nodes; very old JSONL may have used a string (double-encoded JSON), which breaks automatic schema inference across files. |
| `solution_data` | object (optional) | Solution data with normalized structure. Same object-vs-string caveat as `problem_data` for legacy shards. |
| `explorer_card` | string | Preformatted explorer-style card (multi-line text). Uses **absolute UTC** from `timestamp` in the card (not “Ns ago”). Omitted or empty on legacy JSONL without this field. |
| **IDENTIFIERS** |||
| `block_height` | int64 | Block height when the record was created |
| `timestamp` | int64 | Unix timestamp (consensus rows: block header time; marketplace rows may use ingest time — see `metrics_source`) |
| `submitter` | string (optional) | Address of the problem submitter (hex encoded) |
| `solver` | string (optional) | Address of the solution solver (hex encoded) |
| **PERFORMANCE METRICS** |||
| `problem_complexity` | float64 | Complexity score of the problem |
| `bounty` | string | Bounty amount in native tokens (serialized as string to avoid JSON precision loss) |
| `work_score` | float64 (optional) | Work score calculated for the solution |
| `solution_quality` | float64 (optional) | Quality score of the solution |
| **ASYMMETRY METRICS** |||
| `time_asymmetry` | float64 (optional) | Ratio of solve_time / verify_time |
| `space_asymmetry` | float64 (optional) | Memory asymmetry metric |
| `energy_asymmetry` | float64 (optional) | Energy asymmetry ratio |
| **ENERGY MEASUREMENTS** |||
| `solve_energy_joules` | float64 (optional) | Energy consumed during solving (joules) |
| `verify_energy_joules` | float64 (optional) | Energy consumed during verification (joules) |
| `total_energy_joules` | float64 (optional) | Total energy consumption (joules) |
| `energy_per_operation` | float64 (optional) | Energy per operation estimate |
| `energy_efficiency` | float64 (optional) | Energy efficiency metric |
| **TIMING (consensus / detailed rows)** |||
| `solve_time_us` | uint64 (optional) | Solve duration in microseconds (→ ms in explorer) |
| `verify_time_us` | uint64 (optional) | Verify duration in microseconds |
| `mining_attempts` | uint64 or null | **Present on every JSONL row** (`null` when not applicable). For consensus blocks the node sets this to the header `nonce` (on-chain proxy for search effort). Omitting the key across some shards caused Hugging Face Data Studio column mismatches. |
| **MINING / CONSENSUS** |||
| `difficulty_target` | uint32 (optional) | Minimum leading zero bits in block hash (node PoW setting) |
| `nonce` | uint64 (optional) | Winning header nonce |
| **METADATA** |||
| `status` | string | Status: "Pending", "Solved", "Mined", "Validated", etc. |
| `submission_mode` | string | Submission mode: "public", "private", or "mining" |
| `energy_measurement_method` | string | Method used: "rapl", "powermetrics", or "estimate" |
| **DATA PROVENANCE** |||
| `metrics_source` | string | Source of metrics: "block_header_actual", "measured_marketplace", "estimated", or "not_applicable" |
| `measurement_confidence` | string | Confidence level: "high" (from header), "medium" (proxy/measured), "low" (estimate), or "not_applicable" |
| `data_version` | string | Dataset schema version (e.g. `v3.1` — see `huggingface/src/metrics.rs`) |

Consensus and marketplace paths may populate **additional optional fields** (timing, memory, energy, network, mining, hardware, economics). The full schema is `DatasetRecord` in `huggingface/src/client.rs`.

### Solution Data Structure

Solutions are normalized to a consistent structure to avoid schema conflicts:

```json
{
  "type": "SubsetSum" | "SAT" | "TSP" | "Custom",
  "data": <normalized data>
}
```

- **SubsetSum**: `data` is an array of indices (numbers)
- **SAT**: `data` is an array of 0/1 values (normalized from booleans)
- **TSP**: `data` is an array representing the tour (numbers)
- **Custom**: `data` is a base64-encoded string

### Problem Data Structure

For consensus blocks, `problem_data` contains comprehensive block information:

```json
{
  "height": <block_height>,
  "miner": <miner_address>,
  "transactions": [...],
  "solution_reveal": {
    "problem": {...},
    "solution": {
      "type": "...",
      "data": [...]
    },
    "commitment_hash": "...",
    "problem_hash": "..."
  },
  "solve_time_us": <time_in_microseconds>,
  "verify_time_us": <time_in_microseconds>,
  "energy_estimate_joules": <energy>,
  ...
}
```

## Dataset Creation

### Source Data

Data is collected in real-time from running COINjecture Network nodes. Each node pushes records to this dataset when:
- A problem is submitted via transaction
- A solution is submitted and verified
- A consensus block is mined or validated

### Data Collection Process

1. **Problem Submission**: When a problem transaction is processed, a record is created with problem data
2. **Solution Submission**: When a solution is verified, metrics are calculated and a record is created
3. **Consensus Blocks**: Complete block data is recorded for transparency and analysis

### Data Preprocessing

- Solutions are normalized to consistent schema (see Solution Data Structure)
- Energy measurements use multiple methods (RAPL, powermetrics, or estimation)
- Addresses are hex-encoded for consistency
- Timestamps are Unix epoch seconds
- Large integers (u128) are serialized as strings to avoid JSON precision loss
- All problem types are unified in a single continuous dataset for cross-problem analysis

### Normalizing legacy JSONL (Hub viewer / `CastError`)

Older JSONL omitted optional keys on some rows. That yields different Arrow **column sets** across shards and breaks Data Studio with `CastError` ("column names don't match"). Current nodes emit the full key set on every line (`null` when unknown).

To **batch-fix** existing `data/*.jsonl` files locally (same key names as `DatasetRecord` in `huggingface/src/client.rs`; unknown top-level keys are dropped):

```bash
python3 scripts/hf_np_solutions_normalize_jsonl.py --in data/data_1775801281.jsonl --out data/data_1775801281.norm.jsonl
```

Then replace the originals on the Hub (e.g. `hf upload` or Hub API commits). For thousands of files, run in a loop or job runner and commit in batches. If you change `DatasetRecord`, update `RECORD_KEYS` in `scripts/hf_np_solutions_jsonl_common.py` to match.

**Automated Hub pass** (download → normalize → upload one commit per file; needs `pip install huggingface_hub` and a write token):

```bash
export HF_TOKEN=hf_...
python3 scripts/hf_np_solutions_batch_normalize_hub.py --dry-run
python3 scripts/hf_np_solutions_batch_normalize_hub.py --limit 5
python3 scripts/hf_np_solutions_batch_normalize_hub.py --sleep 1.0
```

Use `--start-after data/data_<timestamp>.jsonl` to resume. Full runs create thousands of commits; prefer a VM, tune `--sleep`, or fork the script to batch multiple files per `create_commit` if you hit rate limits.

**Pin the Hub schema (`dataset_infos.json`)**: Data Studio can infer a **partial** feature set from the first shards (missing e.g. `explorer_card`, `peer_count`, `sync_lag_blocks`), then fail when a later shard adds those columns (`CastError: column names don't match`). Upload a root-level `dataset_infos.json` so `datasets` uses the full column layout and `Json` types for `problem_data`, `solution_data`, and `pool_distributions`.

- Generate (or refresh after changing `DatasetRecord` / `RECORD_KEYS`):

```bash
python3 scripts/hf_np_solutions_emit_dataset_infos.py
```

- Commit the generated file in this repo at `huggingface/dataset_infos.json`, then upload it to the **dataset repository root** (next to `README.md`), not under `huggingface/` on the Hub:

```bash
hf upload COINjecture/NP-Solutions huggingface/dataset_infos.json dataset_infos.json --repo-type=dataset
```

You still need **normalized JSONL** everywhere so `problem_data` / `solution_data` are JSON objects (not stringified blobs); `dataset_infos.json` fixes the **target** schema, not bad row encodings.

## Dataset Statistics

- **Total Records**: Growing in real-time (unified dataset with all problem types)
- **Update Frequency**: Real-time (buffered, flushed when 10 total records accumulated across all problem types)
- **Data Format**: JSONL (newline-delimited JSON)
- **Storage Location**: `/data/` directory in the repository
- **Problem Types**: SubsetSum, SAT, TSP, Custom, Private (all in one dataset)
- **Data Quality**: v3.1 institutional-grade records when emitted by current nodes (block header and extended metrics where available)

## Considerations for Using the Data

### Ethical Considerations

- All data is from public blockchain transactions
- Addresses are included only if explicitly enabled (privacy option)
- No personally identifiable information is collected

### Licensing

This dataset is released under the MIT License.

### Citation Information

If you use this dataset in your research, please cite:

```bibtex
@dataset{coinjecture_np_solutions,
  title={COINjecture NP-Solutions Dataset},
  author={COINjecture Network},
  year={2024},
  url={https://huggingface.co/datasets/COINjecture/NP-Solutions}
}
```

## Dataset Access

### Using Hugging Face Datasets

```python
from datasets import load_dataset

# Load the dataset
dataset = load_dataset("COINjecture/NP-Solutions", split="train")

# Access records
for record in dataset:
    print(record["problem_id"])
    print(record["problem_data"])
```

### Direct File Access

The raw JSONL files are available in the `/data/` directory:
- Files are named `data_<timestamp>.jsonl`
- Each line is a complete JSON record
- Files can be processed with standard JSONL tools

### API Access

The dataset is accessible via the Hugging Face API:
- Dataset viewer: https://huggingface.co/datasets/COINjecture/NP-Solutions
- API endpoint: `https://huggingface.co/api/datasets/COINjecture/NP-Solutions`

### Uploading with the Hugging Face CLI

For manual pushes (exports, Parquet/JSONL, README updates), use the [`hf` CLI](https://huggingface.co/docs/huggingface_hub/guides/cli). Install **one** of these (pick what works on your machine):

```bash
# Option A — standalone installer (macOS / Linux; adds hf to your PATH)
curl -LsSf https://hf.co/cli/install.sh | bash

# Option B — Homebrew (if the formula is available on your Mac)
brew install hf

# Option C — Python (hf ships with huggingface_hub; ensure the install’s bin is on PATH)
python3 -m pip install -U "huggingface_hub"
```

Then authenticate and upload (example: dataset card only from this repo):

```bash
hf auth login   # or: export HF_TOKEN=...  (never commit tokens)

cd /path/to/COINjecture2.0-main
hf upload COINjecture/NP-Solutions huggingface/README.md --repo-type=dataset \
  --commit-message "Dataset card: viewer YAML + schema notes"

# Or upload everything in the current directory tree:
hf upload COINjecture/NP-Solutions . --repo-type=dataset
```

Set `HF_DATASET_NAME` / `--hf-dataset-name` to this repo’s Hub id (`COINjecture/NP-Solutions`). Hyphen vs underscore are different Hub repositories if both exist.

## Additional Information

### Energy Measurement Methods

- **RAPL** (Linux): Intel/AMD Running Average Power Limit counters
- **powermetrics** (macOS): macOS powermetrics tool
- **estimate**: CPU TDP-based estimation (fallback, works everywhere)

### Problem Types

1. **SubsetSum**: Find a subset of numbers that sum to a target
2. **SAT**: Boolean satisfiability problem
3. **TSP**: Traveling Salesman Problem
4. **Custom**: Arbitrary problem data (base64 encoded)

### Performance Metrics

- **Time Asymmetry**: Measures how much harder solving is than verifying
- **Space Asymmetry**: Memory usage differences
- **Energy Asymmetry**: Energy consumption differences
- **Energy Efficiency**: Work performed per unit of energy

## Contact

For questions or issues:
- Dataset repository: https://huggingface.co/datasets/COINjecture/NP-Solutions
- Open a discussion on the dataset page

## Changelog

### 2026-04-16
- **Hub `CastError` / “column names don’t match”**: Older JSONL omitted optional fields entirely (`serde` `skip_serializing_if`). Different shards then inferred different Arrow column sets. Nodes now serialize **every** `DatasetRecord` field on every line (`null` when `None`), so new shards align with the full schema. Existing Hub files stay sparse until replaced or batch-normalized.
- **`dataset_infos.json`**: Added `scripts/hf_np_solutions_emit_dataset_infos.py` and a generated `huggingface/dataset_infos.json` so the Hub can pin the full feature list (including `Json` columns). Upload that file to the **dataset repo root** on the Hub alongside `README.md`, then wait for the viewer job to refresh.

### 2026-04-15
- **JSONL shape (nodes)**: `mining_attempts` is always serialized (use `null` when unknown); consensus rows set it from header `nonce`. `problem_data` / `solution_data` are coerced to JSON objects before upload so stringified JSON blobs are not emitted.
- **Dataset card YAML**: Do **not** add a `configs` / `on_mixed_types` block for this repo: it made the Hub builder infer `problem_data`/`solution_data` as strings while legacy shards still use nested JSON objects, which triggers `DatasetGenerationError` / `CastError` when generating the preview table.

### 2026-04-10
- **`explorer_card` field**: Each JSONL row includes a precomputed multi-line card (UTC time); implemented in `huggingface/src/explorer_card.rs`.
- **Explorer layout**: Documented the block-card presentation (block #, type, time, problem/solution prose, solver, BEANS reward, work, asymmetry, quality, Δt, energy) with line-by-line JSON mapping and a Python `format_block_card` helper.

### 2025-11-23
- **Unified Dataset**: Consolidated all problem types (SubsetSum, SAT, TSP, Custom) into a single continuous dataset
- **Schema Fix**: Fixed u128 bounty serialization (now serialized as string to avoid JSON precision loss)
- **Data Provenance**: Added institutional-grade data provenance fields (metrics_source, measurement_confidence, data_version)
- **Unified Buffer**: Changed from per-problem-type buffers to unified buffer that flushes all types together
- **Enhanced Metrics**: All consensus blocks now include actual block header metrics (high confidence)



