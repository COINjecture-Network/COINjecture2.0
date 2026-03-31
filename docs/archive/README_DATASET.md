---
license: mit
task_categories:
  - other
tags:
  - blockchain
  - proof-of-useful-work
  - np-complete
  - computational-complexity
  - consensus
  - cryptography
  - distributed-systems
  - research
pretty_name: COINjecture NP Solutions v2
size_categories:
  - 1K<n<10K
configs:
  - config_name: default
    data_files:
      - split: train
        path: "data/*.jsonl"
---

<div align="center">

# 🔬 COINjecture NP Solutions Dataset v2

### Institutional-Grade Blockchain Research Data

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Data Version](https://img.shields.io/badge/Data%20Version-v3.0-blue.svg)](#data-versioning)
[![Update Frequency](https://img.shields.io/badge/Updates-Real--time-green.svg)](#update-frequency)

**A comprehensive, real-time dataset of NP-complete problem solutions generated through Proof-of-Useful-Work (PoUW) blockchain consensus**

[Overview](#-overview) • [Data Schema](#-data-schema) • [Metrics Categories](#-metrics-categories) • [Usage](#-usage) • [Citation](#-citation)

</div>

---

## 📋 Overview

This dataset contains **institutional-grade metrics** from the COINjecture Network B blockchain, which implements a novel **Proof-of-Useful-Work (PoUW)** consensus mechanism. Unlike traditional Proof-of-Work systems that compute arbitrary hashes, COINjecture miners solve genuine NP-complete computational problems, producing verifiable solutions with real-world applicability.

### Key Characteristics

| Property | Value |
|----------|-------|
| **Network** | COINjecture Network B (Fresh Genesis) |
| **Genesis Hash** | `4a80254b4a48e867b57399469b0a1fbaba8848e8ac738587b55ebf6e6b8c4b23` |
| **Data Version** | v3.0 (Institutional Grade) |
| **Problem Types** | SAT, SubsetSum, TSP |
| **Update Frequency** | Every ~10 blocks (~10 seconds) |
| **Metrics Per Record** | 54+ fields |
| **Format** | JSON Lines (`.jsonl`) |

### Research Applications

- **Computational Complexity**: Empirical analysis of NP-complete problem hardness
- **Algorithm Performance**: Solve/verify time distributions across problem types
- **Distributed Systems**: Consensus metrics and network propagation analysis
- **Energy Research**: Computational efficiency and resource utilization studies
- **Cryptographic Analysis**: Hash function behavior and difficulty adjustment

---

## 📊 Data Schema

Each record represents a block in the COINjecture blockchain containing a solved NP-complete problem instance.

### Core Fields

| Field | Type | Description |
|-------|------|-------------|
| `block_height` | `uint64` | Sequential block number in the canonical chain |
| `block_hash` | `string` | SHA-256 hash of the block header (hex-encoded) |
| `prev_block_hash` | `string` | Hash of the parent block (enables chain traversal) |
| `timestamp` | `string` | ISO 8601 timestamp of block creation |
| `problem_type` | `string` | NP-complete problem class: `SAT`, `SubsetSum`, or `TSP` |

### Problem Instance Fields

| Field | Type | Description |
|-------|------|-------------|
| `problem_instance` | `object` | Serialized problem definition (varies by type) |
| `solution` | `object` | Verified solution to the problem instance |
| `problem_size` | `uint32` | Instance complexity metric (variables, nodes, etc.) |
| `is_satisfiable` | `boolean` | For SAT: whether a satisfying assignment exists |

---

## 📈 Metrics Categories

### ⏱️ Timing Metrics (Microsecond Precision)

High-resolution timing data for performance analysis:

| Field | Type | Unit | Description |
|-------|------|------|-------------|
| `solve_time_us` | `uint64` | μs | Time to find the solution |
| `verify_time_us` | `uint64` | μs | Time to verify solution correctness |
| `block_time_seconds` | `float64` | s | Total block production time |
| `mining_attempts` | `uint64` | count | Hash attempts before valid block found |

### 💾 Memory Metrics

Resource utilization during computation:

| Field | Type | Unit | Description |
|-------|------|------|-------------|
| `solve_memory_bytes` | `uint64` | bytes | Peak memory during solve phase |
| `verify_memory_bytes` | `uint64` | bytes | Peak memory during verification |
| `peak_memory_bytes` | `uint64` | bytes | Maximum memory allocation |

### 🌐 Network Metrics

Distributed system behavior:

| Field | Type | Unit | Description |
|-------|------|------|-------------|
| `peer_count` | `uint32` | count | Connected peers at block time |
| `propagation_time_ms` | `uint64` | ms | Block propagation latency |
| `sync_lag_blocks` | `int64` | blocks | Distance from network tip |

### ⛏️ Mining Metrics

Consensus and difficulty data:

| Field | Type | Description |
|-------|------|-------------|
| `difficulty_target` | `string` | Current difficulty target (hex) |
| `nonce` | `uint64` | Winning nonce value |
| `hash_rate_estimate` | `float64` | Estimated network hash rate (H/s) |
| `mined_locally` | `boolean` | Whether this node mined the block |

### 🔗 Chain Metrics

Blockchain state information:

| Field | Type | Description |
|-------|------|-------------|
| `chain_work` | `string` | Cumulative proof-of-work score |
| `transaction_count` | `uint32` | Transactions in block |
| `block_size_bytes` | `uint64` | Serialized block size |

### 💰 Economic Metrics

Token economics data:

| Field | Type | Unit | Description |
|-------|------|------|-------------|
| `block_reward` | `uint64` | tokens | Mining reward for this block |
| `total_fees` | `uint64` | tokens | Transaction fees collected |

### 🖥️ Hardware Context

Node environment information for reproducibility:

| Field | Type | Description |
|-------|------|-------------|
| `cpu_model` | `string` | Processor model identifier |
| `cpu_cores` | `uint32` | Physical CPU cores |
| `cpu_threads` | `uint32` | Logical CPU threads |
| `ram_total_bytes` | `uint64` | Total system RAM |
| `os_info` | `string` | Operating system details |

### 🏷️ Provenance Metadata

Data lineage and quality indicators:

| Field | Type | Description |
|-------|------|-------------|
| `node_version` | `string` | Software version that produced this record |
| `node_id` | `string` | Unique node identifier (anonymized) |
| `data_version` | `string` | Schema version (currently `v3.0`) |
| `measurement_confidence` | `float64` | Data quality score (0.0-1.0) |

---

## 🔬 Problem Types

### SAT (Boolean Satisfiability)

The canonical NP-complete problem. Given a Boolean formula in CNF, find a satisfying assignment or prove none exists.

```json
{
  "problem_type": "SAT",
  "problem_instance": {
    "num_variables": 50,
    "num_clauses": 215,
    "clauses": [[1, -3, 5], [-2, 4], ...]
  },
  "solution": {
    "satisfiable": true,
    "assignment": [true, false, true, ...]
  }
}
```

### SubsetSum

Given a set of integers and a target sum, find a subset that sums to the target.

```json
{
  "problem_type": "SubsetSum",
  "problem_instance": {
    "set": [3, 7, 1, 8, -2, 4],
    "target": 12
  },
  "solution": {
    "subset_indices": [1, 3, 5]
  }
}
```

### TSP (Traveling Salesman Problem)

Find the shortest Hamiltonian cycle through all vertices in a weighted graph.

```json
{
  "problem_type": "TSP",
  "problem_instance": {
    "num_cities": 20,
    "distances": [[0, 10, 15], [10, 0, 20], ...]
  },
  "solution": {
    "tour": [0, 3, 1, 4, 2, 0],
    "total_distance": 97
  }
}
```

---

## 📖 Usage

### Loading with Hugging Face Datasets

```python
from datasets import load_dataset

# Load the complete dataset
dataset = load_dataset("COINjecture/NP_Solutions_v2")

# Access records
for record in dataset["train"]:
    print(f"Block {record['block_height']}: {record['problem_type']}")
    print(f"  Solve time: {record['solve_time_us']}μs")
    print(f"  CPU: {record['cpu_model']}")
```

### Loading Raw JSONL

```python
import json
from pathlib import Path

records = []
for jsonl_file in Path("data").glob("*.jsonl"):
    with open(jsonl_file) as f:
        for line in f:
            records.append(json.loads(line))

print(f"Loaded {len(records)} records")
```

### Filtering by Problem Type

```python
sat_problems = dataset["train"].filter(
    lambda x: x["problem_type"] == "SAT"
)
print(f"SAT problems: {len(sat_problems)}")
```

### Performance Analysis Example

```python
import pandas as pd

# Convert to DataFrame for analysis
df = pd.DataFrame(dataset["train"])

# Analyze solve times by problem type
stats = df.groupby("problem_type")["solve_time_us"].agg(["mean", "std", "min", "max"])
print(stats)

# Hardware comparison
hardware_stats = df.groupby("cpu_model")["solve_time_us"].mean()
print(hardware_stats)
```

---

## 📊 Data Quality

### Verification Standards

All data in this dataset meets the following quality criteria:

| Standard | Description |
|----------|-------------|
| **Cryptographic Integrity** | Every block hash is verified against the chain |
| **Solution Validity** | All NP-complete solutions are independently verified |
| **Timing Accuracy** | Microsecond-precision timestamps from monotonic clocks |
| **Hardware Attribution** | Full system context for reproducibility |
| **Chain Continuity** | `prev_block_hash` enables complete chain reconstruction |

### Data Versioning

| Version | Release | Changes |
|---------|---------|---------|
| **v3.0** | Nov 2024 | Institutional-grade: 54+ fields, hardware context, chain linkage |
| v2.0 | Oct 2024 | Added timing metrics, energy estimates |
| v1.0 | Sep 2024 | Initial release: basic problem/solution data |

---

## 🔄 Update Frequency

This dataset receives **real-time updates** approximately every 10 blocks (~10 seconds of blockchain time). New JSONL files are appended as blocks are mined on the COINjecture Network B.

### Data Pipeline Architecture

<table>
<tr>
<td>

**⛓️ CONSENSUS LAYER**
```
     🌱 Genesis (Block 0)
            │
    ┌───────┼───────┐
    ▼       ▼       ▼
  🧮SAT   📊Sum   🗺️TSP
    │       │       │
    └───────┴───────┘
            │
            ▼
```

</td>
<td>

**🌐 P2P NETWORK**
```
  📡 Node 1 ◄────► 📡 Node 2
     │                 │
     └────────┬────────┘
              ▼
       💬 Gossipsub
              │
              ▼
```

</td>
</tr>
<tr>
<td>

**📈 METRICS ENGINE**
```
  ⏱️Timing  💾Memory  🖥️Hardware  🌐Network
      │         │          │          │
      └─────────┴──────────┴──────────┘
                     │
            54+ metrics/block
                     │
                     ▼
```

</td>
<td>

**🎯 DATA OUTPUT**
```
         📦 Buffer (10 blocks)
                │
         every ~10 seconds
                │
                ▼
         🤗 HuggingFace v2
                │
                ▼
         🔌 Datasets API
```

</td>
</tr>
</table>

<div align="center">

**🔬 RESEARCH APPLICATIONS**

| 🤖 Machine Learning | 📊 Performance Analysis | 🔐 Cryptography Research |
|:-------------------:|:-----------------------:|:------------------------:|
| Training data | Solve time analysis | Hash function studies |
| Benchmarking | Hardware comparisons | Difficulty research |

---

*Data flows from NP-complete problem solving → metrics collection → real-time research availability*

</div>

---

## 📜 Citation

If you use this dataset in your research, please cite:

```bibtex
@dataset{coinjecture_np_solutions_v2,
  title={COINjecture NP Solutions Dataset v2},
  author={{COINjecture Network Contributors}},
  year={2024},
  publisher={Hugging Face},
  url={https://huggingface.co/datasets/COINjecture/NP_Solutions_v2},
  note={Institutional-grade blockchain research data from Proof-of-Useful-Work consensus}
}
```

---

## 📄 License

This dataset is released under the [MIT License](https://opensource.org/licenses/MIT). You are free to use, modify, and distribute this data for any purpose, including commercial applications.

---

## 🔗 Related Resources

| Resource | Link |
|----------|------|
| **Legacy Dataset** | [COINjecture/NP_Solutions](https://huggingface.co/datasets/COINjecture/NP_Solutions) |
| **Source Code** | [GitHub](https://github.com/COINjecture) |
| **Network Explorer** | Coming Soon |
| **Technical Whitepaper** | Coming Soon |

---

## 🤝 Contributing

We welcome contributions to improve data quality and documentation. Please open an issue or pull request on our GitHub repository.

---

<div align="center">

**Built with 💎 by the COINjecture Network**

*Transforming computational waste into useful work*

</div>

