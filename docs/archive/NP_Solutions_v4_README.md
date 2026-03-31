---
license: mit
task_categories:
  - other
tags:
  - blockchain
  - np-hard
  - optimization
  - cryptocurrency
  - proof-of-work
  - proof-of-useful-work
  - adzdb
  - vector-storage
pretty_name: COINjecture NP Solutions v4
size_categories:
  - n<1K
configs:
  - config_name: default
    data_files:
      - split: train
        path: "data/*.jsonl"
---

<div align="center">

# 🔬 COINjecture NP Solutions v4

### ADZDB-Powered Blockchain Research Data

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Data Version](https://img.shields.io/badge/Data%20Version-v4.0-orange.svg)](#data-versioning)
[![Storage Engine](https://img.shields.io/badge/Storage-ADZDB-blue.svg)](#adzdb-integration)
[![Update Frequency](https://img.shields.io/badge/Updates-Real--time-green.svg)](#update-frequency)

**High-dimensional vector storage with true Proof-of-Useful-Work consensus**

[Overview](#-overview) • [ADZDB Integration](#-adzdb-integration) • [Fork Comparison](#-fork-comparison) • [Data Schema](#-data-schema) • [Usage](#-usage)

</div>

---

## 📋 Overview

This is the **v4 (Modern Fork)** of the COINjecture NP Solutions dataset, featuring the revolutionary **ADZDB** (Append-Delete-Zero Database) storage engine. This fork achieves true Proof-of-Useful-Work with energy asymmetry >1900x, compared to the legacy v3 fork's ~1x.

### Key Characteristics

| Property | Value |
|----------|-------|
| **Network** | COINjecture Network B (v4 Modern Fork) |
| **Storage Engine** | ADZDB (Dimensional Vector Store) |
| **Energy Asymmetry** | >1900x (True PoUW) |
| **Space Asymmetry** | ~44x |
| **Work Score** | ~162+ (vs v3's ~0.00007) |
| **Problem Types** | TSP, 3SAT, Knapsack, Graph Coloring, Subset Sum |
| **Update Frequency** | Real-time streaming |
| **Format** | JSON Lines (`.jsonl`) |

---

## 🗄️ ADZDB Integration

### What is ADZDB?

**ADZDB** (Append-Delete-Zero Database) is a specialized storage engine designed specifically for blockchain data, inspired by:

- **NuDB** (XRPL): Append-only data file, linear hashing, O(1) reads
- **TigerBeetle**: Deterministic operations, zero-copy structs, protocol-aware recovery

### Design Principles

| Principle | Description |
|-----------|-------------|
| **Append-only** | Data is never overwritten, only appended |
| **Deterministic** | All operations produce identical results |
| **Zero-copy** | Fixed-size headers for direct memory mapping |
| **Content-addressable** | O(1) lookups by block hash |
| **Height-indexed** | O(1) lookups by block height |

### File Structure

```
adzdb/
├── adzdb.idx     # Hash index (hash → offset)
├── adzdb.dat     # Data file (append-only block storage)
├── adzdb.hgt     # Height index (height → hash)
└── adzdb.meta    # Metadata (chain state)
```

### Why ADZDB Enables True PoUW

The v4 fork's high asymmetry metrics come from ADZDB's ability to efficiently store and retrieve high-dimensional vector problems. Unlike generic B-tree databases (Redb in v3), ADZDB is optimized for:

1. **Fast verification**: O(1) solution lookups for NP verification
2. **Dimensional search**: Efficient nearest-neighbor queries for TSP
3. **Memory efficiency**: Zero-copy design minimizes verify-time memory

---

## ⚖️ Fork Comparison

### v3 Legacy vs v4 Modern

| Metric | v3 (Legacy) | v4 (Modern) | Improvement |
|--------|-------------|-------------|-------------|
| **Storage Engine** | Redb (B-Tree) | Adzdb (Dimensional) | Purpose-built |
| **Energy Asymmetry** | ≈1.0 | **>1900.0** | ~2000x |
| **Space Asymmetry** | ≈1.0 | **~44.46** | ~44x |
| **Work Score** | ~0.00007 | **~162.66** | Orders of magnitude |
| **Consensus** | Trivial Proofs | **True PoUW** | Genuine NP-hardness |

### What This Means

- **v3 (Legacy)**: Uses generic Redb storage, verification is as expensive as solving → no asymmetry → not true PoUW
- **v4 (Modern)**: Uses specialized ADZDB storage, verification is ~2000x cheaper than solving → high asymmetry → **TRUE** Proof-of-Useful-Work

---

## 📊 Data Schema

Each entry contains comprehensive metrics from the v4 fork:

### Core Fields

| Field | Type | Description |
|-------|------|-------------|
| `block_height` | `uint64` | Block number where solution was found |
| `block_hash` | `string` | Unique block identifier (hex) |
| `problem_type` | `string` | Type of NP-hard problem |
| `problem_instance` | `object` | The problem definition |
| `solution` | `object` | Verified solution |
| `timestamp` | `uint64` | Unix timestamp |
| `miner_address` | `string` | Address of the solving node |

### Performance Metrics

| Field | Type | Description |
|-------|------|-------------|
| `work_score` | `float64` | Computational difficulty score |
| `solve_time_us` | `uint64` | Time to solve (microseconds) |
| `verify_time_us` | `uint64` | Time to verify (microseconds) |
| `time_asymmetry` | `float64` | solve_time / verify_time |
| `energy_asymmetry` | `float64` | solve_energy / verify_energy |
| `space_asymmetry` | `float64` | solve_memory / verify_memory |

### Energy Metrics

| Field | Type | Description |
|-------|------|-------------|
| `solve_energy_joules` | `float64` | Energy used solving |
| `verify_energy_joules` | `float64` | Energy used verifying |
| `total_energy_joules` | `float64` | Total energy consumed |
| `energy_efficiency` | `float64` | Work per joule |

### Hardware Context

| Field | Type | Description |
|-------|------|-------------|
| `cpu_model` | `string` | Processor model |
| `cpu_cores` | `uint32` | Physical CPU cores |
| `ram_total_bytes` | `uint64` | System memory |

---

## 🔬 Problem Types

### TSP (Traveling Salesman Problem)
Find the shortest Hamiltonian cycle through all vertices.

### 3SAT (Boolean Satisfiability)
Find a satisfying assignment for a 3-CNF Boolean formula.

### Knapsack (0/1 Knapsack)
Select items to maximize value within weight constraint.

### Graph Coloring
Color vertices so no adjacent vertices share a color.

### Subset Sum
Find a subset of numbers that sum to a target value.

---

## 📖 Usage

### Loading with Hugging Face Datasets

```python
from datasets import load_dataset

# Load the v4 dataset
dataset = load_dataset("COINjecture/NP_Solutions_v4")

# Explore the data
for record in dataset["train"]:
    print(f"Block {record['block_height']}: {record['problem_type']}")
    print(f"  Work Score: {record['work_score']}")
    print(f"  Energy Asymmetry: {record['energy_asymmetry']}")
    print(f"  Time Asymmetry: {record['time_asymmetry']}")
```

### Comparing v3 vs v4

```python
from datasets import load_dataset

v3 = load_dataset("COINjecture/NP_Solutions_v3")
v4 = load_dataset("COINjecture/NP_Solutions_v4")

# Compare average energy asymmetry
v3_asymmetry = sum(r.get('energy_asymmetry', 1) for r in v3['train']) / len(v3['train'])
v4_asymmetry = sum(r.get('energy_asymmetry', 1) for r in v4['train']) / len(v4['train'])

print(f"v3 avg energy asymmetry: {v3_asymmetry:.2f}")
print(f"v4 avg energy asymmetry: {v4_asymmetry:.2f}")
print(f"Improvement: {v4_asymmetry / v3_asymmetry:.0f}x")
```

### Filtering by Problem Type

```python
# Get all TSP solutions
tsp_solutions = dataset["train"].filter(
    lambda x: x["problem_type"] == "TSP"
)

# Analyze work scores
import statistics
work_scores = [r["work_score"] for r in tsp_solutions]
print(f"Mean TSP work score: {statistics.mean(work_scores):.2f}")
```

---

## 🔗 Related Resources

| Resource | Link |
|----------|------|
| **v3 Legacy Dataset** | [COINjecture/NP_Solutions_v3](https://huggingface.co/datasets/COINjecture/NP_Solutions_v3) |
| **ADZDB Repository** | [Quigles1337/ADZDB](https://github.com/Quigles1337/ADZDB) |
| **Source Code** | [GitHub](https://github.com/beanapologist/COINjecture-NetB-Updates) |

---

## 📜 Citation

```bibtex
@dataset{coinjecture_np_solutions_v4,
  title={COINjecture NP Solutions Dataset v4},
  author={{COINjecture Network Contributors}},
  year={2024},
  publisher={Hugging Face},
  url={https://huggingface.co/datasets/COINjecture/NP_Solutions_v4},
  note={ADZDB-powered blockchain with true Proof-of-Useful-Work (>1900x energy asymmetry)}
}
```

---

## 📄 License

MIT License - Free to use for any purpose.

---

<div align="center">

**Built with 💎 by the COINjecture Network**

*True Proof-of-Useful-Work through dimensional vector storage*

</div>


