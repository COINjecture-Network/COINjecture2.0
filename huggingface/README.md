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

# COINjecture NP Solutions Dataset

## Dataset Description

This dataset contains real-time blockchain data from the COINjecture Network, a proof-of-useful-work (PoUW) blockchain that uses NP-complete problems for consensus. This is a **unified, continuous dataset** that includes all problem types (SubsetSum, SAT, TSP, Custom) and consensus blocks in a single repository for comprehensive analysis.

### Dataset Summary

The COINjecture Network is a blockchain that replaces traditional proof-of-work mining with solving useful computational problems. This unified dataset captures:

- **Problem Submissions**: NP-complete problems (SubsetSum, SAT, TSP, Custom) submitted to the network
- **Solution Submissions**: Solutions to problems with verification metrics
- **Consensus Blocks**: Complete block data including transactions, PoUW metrics, and energy measurements

**All problem types are stored in a single continuous dataset** (`COINjecture/NP_Solutions`) to enable cross-problem-type analysis and unified research workflows.

### Supported Tasks

- **Research**: Study of NP-complete problem solving performance
- **Energy Analysis**: Energy consumption patterns in computational problem solving
- **Blockchain Analytics**: Consensus mechanism performance and transparency metrics
- **Machine Learning**: Training models on problem-solution pairs

### Languages

English (problem descriptions and metadata)

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
| `problem_data` | object | Complete problem data (JSON object) |
| `solution_data` | object (optional) | Solution data with normalized structure |
| **IDENTIFIERS** |||
| `block_height` | int64 | Block height when the record was created |
| `timestamp` | int64 | Unix timestamp |
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
| **METADATA** |||
| `status` | string | Status: "Pending", "Solved", "Mined", "Validated", etc. |
| `submission_mode` | string | Submission mode: "public", "private", or "mining" |
| `energy_measurement_method` | string | Method used: "rapl", "powermetrics", or "estimate" |
| **DATA PROVENANCE** |||
| `metrics_source` | string | Source of metrics: "block_header_actual", "measured_marketplace", "estimated", or "not_applicable" |
| `measurement_confidence` | string | Confidence level: "high" (from header), "medium" (proxy/measured), "low" (estimate), or "not_applicable" |
| `data_version` | string | Dataset schema version: "v2.0" (institutional-grade with actual metrics) |

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

## Dataset Statistics

- **Total Records**: Growing in real-time (unified dataset with all problem types)
- **Update Frequency**: Real-time (buffered, flushed when 10 total records accumulated across all problem types)
- **Data Format**: JSONL (newline-delimited JSON)
- **Storage Location**: `/data/` directory in the repository
- **Problem Types**: SubsetSum, SAT, TSP, Custom, Private (all in one dataset)
- **Data Quality**: v2.0 institutional-grade with actual block header metrics when available

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
  title={COINjecture NP Solutions Dataset},
  author={COINjecture Network},
  year={2024},
  url={https://huggingface.co/datasets/COINjecture/NP_Solutions}
}
```

## Dataset Access

### Using Hugging Face Datasets

```python
from datasets import load_dataset

# Load the dataset
dataset = load_dataset("COINjecture/NP_Solutions", split="train")

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
- Dataset viewer: https://huggingface.co/datasets/COINjecture/NP_Solutions
- API endpoint: `https://huggingface.co/api/datasets/COINjecture/NP_Solutions`

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
- Dataset repository: https://huggingface.co/datasets/COINjecture/NP_Solutions
- Open a discussion on the dataset page

## Changelog

### 2025-11-23
- **Unified Dataset**: Consolidated all problem types (SubsetSum, SAT, TSP, Custom) into a single continuous dataset
- **Schema Fix**: Fixed u128 bounty serialization (now serialized as string to avoid JSON precision loss)
- **Data Provenance**: Added institutional-grade data provenance fields (metrics_source, measurement_confidence, data_version)
- **Unified Buffer**: Changed from per-problem-type buffers to unified buffer that flushes all types together
- **Enhanced Metrics**: All consensus blocks now include actual block header metrics (high confidence)



