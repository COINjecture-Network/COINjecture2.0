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
  - legacy
pretty_name: COINjecture NP Solutions v3 (Legacy)
size_categories:
  - n<1K
configs:
  - config_name: default
    data_files:
      - split: train
        path: "data/*.jsonl"
---

<div align="center">

# 🔬 COINjecture NP Solutions v3 (Legacy)

### Redb-Based Blockchain Research Data

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Data Version](https://img.shields.io/badge/Data%20Version-v3.0-gray.svg)](#data-versioning)
[![Fork Status](https://img.shields.io/badge/Status-DEPRECATED-red.svg)](#migration-notice)

**Legacy fork using Redb storage - See v4 for the active Modern fork**

[Overview](#-overview) • [Migration Notice](#-migration-notice) • [Data Schema](#-data-schema) • [Usage](#-usage)

</div>

---

## ⚠️ Migration Notice

**This is the LEGACY v3 fork.** The COINjecture network has migrated to the **v4 Modern Fork** with ADZDB storage, which achieves true Proof-of-Useful-Work.

### Why Migrate to v4?

| Metric | v3 (This Dataset) | v4 (Recommended) |
|--------|-------------------|------------------|
| **Storage Engine** | Redb (B-Tree) | Adzdb (Dimensional) |
| **Energy Asymmetry** | ≈1.0 | **>1900.0** |
| **Work Score** | ~0.00007 | **~162.66** |
| **Status** | Deprecated | **Active** |

👉 **[Migrate to v4](https://huggingface.co/datasets/COINjecture/NP_Solutions_v4)**

---

## 📋 Overview

This dataset contains solutions to NP-hard problems from the **v3 Legacy Fork** of the COINjecture Network B blockchain. The v3 fork uses Redb (a general-purpose B-tree database) for storage.

### Key Characteristics

| Property | Value |
|----------|-------|
| **Network** | COINjecture Network B (v3 Legacy Fork) |
| **Storage Engine** | Redb (B-Tree) |
| **Energy Asymmetry** | ≈1.0 (Trivial Proofs) |
| **Problem Types** | TSP, 3SAT, Knapsack, Graph Coloring, Subset Sum |
| **Format** | JSON Lines (`.jsonl`) |
| **Status** | **DEPRECATED** |

### Limitation: No True PoUW

The v3 fork's energy asymmetry of ~1.0 means verification is approximately as expensive as solving. This makes it effectively **not** Proof-of-Useful-Work, as there's no computational asymmetry advantage.

---

## 📊 Data Schema

### Core Fields

| Field | Type | Description |
|-------|------|-------------|
| `block_height` | `uint64` | Block number |
| `block_hash` | `string` | Block identifier (hex) |
| `problem_type` | `string` | NP-hard problem type |
| `problem_instance` | `object` | Problem definition |
| `solution` | `object` | Verified solution |
| `work_score` | `float64` | Difficulty score (~0.00007) |
| `timestamp` | `uint64` | Unix timestamp |
| `miner_address` | `string` | Solver address |

### Performance Metrics

| Field | Type | v3 Typical Value |
|-------|------|------------------|
| `energy_asymmetry` | `float64` | ~1.0 |
| `space_asymmetry` | `float64` | ~1.0 |
| `work_score` | `float64` | ~0.00007 |

---

## 📖 Usage

### Loading the Dataset

```python
from datasets import load_dataset

# Load v3 legacy dataset
dataset = load_dataset("COINjecture/NP_Solutions_v3")

for record in dataset["train"]:
    print(f"Block {record['block_height']}: {record['problem_type']}")
    print(f"  Work Score: {record['work_score']}")
```

### Compare with v4

```python
# Load both versions
v3 = load_dataset("COINjecture/NP_Solutions_v3")
v4 = load_dataset("COINjecture/NP_Solutions_v4")

# v4 should have dramatically higher asymmetry
print("v3 uses Redb - no asymmetry")
print("v4 uses ADZDB - ~2000x energy asymmetry")
```

---

## 🔗 Related Resources

| Resource | Link |
|----------|------|
| **v4 Modern Dataset (RECOMMENDED)** | [COINjecture/NP_Solutions_v4](https://huggingface.co/datasets/COINjecture/NP_Solutions_v4) |
| **Source Code** | [GitHub](https://github.com/beanapologist/COINjecture-NetB-Updates) |

---

## 📜 Citation

```bibtex
@dataset{coinjecture_np_solutions_v3,
  title={COINjecture NP Solutions Dataset v3 (Legacy)},
  author={{COINjecture Network Contributors}},
  year={2024},
  publisher={Hugging Face},
  url={https://huggingface.co/datasets/COINjecture/NP_Solutions_v3},
  note={Legacy Redb-based fork - see v4 for active modern fork}
}
```

---

## 📄 License

MIT License

---

<div align="center">

⚠️ **This is a legacy dataset. Please use [v4](https://huggingface.co/datasets/COINjecture/NP_Solutions_v4) for current data.**

</div>
