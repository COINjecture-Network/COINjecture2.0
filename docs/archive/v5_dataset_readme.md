---
license: mit
task_categories:
  - tabular-classification
  - tabular-regression
language:
  - en
tags:
  - blockchain
  - np-hard
  - computational-complexity
  - proof-of-useful-work
  - cryptography
  - optimization
  - sat
  - tsp
  - subset-sum
pretty_name: COINjecture NP Solutions v5
size_categories:
  - 10K<n<100K
---

<div align="center">

# 🧮 COINjecture NP Solutions v5

### The Empirical Evolution

**Live solutions to NP-hard computational problems from the COINjecture blockchain**

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Dataset](https://img.shields.io/badge/🤗-Dataset-blue)](https://huggingface.co/datasets/COINjecture/v5)
[![Network](https://img.shields.io/badge/Network-Live-brightgreen)](https://coinjecture.com)

[Documentation](https://coinjecture.com/docs) • [GitHub](https://github.com/Quigles1337/GITHUB-SUCKS) • [Website](https://coinjecture.com)

</div>

---

## 📋 Overview

This dataset contains **real-time solutions to NP-hard problems** mined from the COINjecture Network B blockchain. Unlike traditional Proof-of-Work that wastes computational energy, COINjecture implements **Proof-of-Useful-Work (PoUW)** — every hash contributes to solving computationally significant problems.

### What Makes v5 Special

| Feature | v4 (Previous) | v5 (Current) |
|---------|---------------|--------------|
| **Tokenomics** | Hardcoded constants | 100% empirical (network-derived) |
| **Light Clients** | Basic SPV | FlyClient + MMR proofs |
| **Node Classification** | Single type | 6 specialized types |
| **Mobile Support** | None | WASM + C-FFI SDK |
| **Metrics** | Static | Live network oracle |

---

## 🎯 Problem Types

### Subset Sum
Given a set of integers, find a subset that sums to a target value.
- **Complexity**: NP-complete
- **Applications**: Cryptography, resource allocation, financial modeling

### Boolean Satisfiability (SAT)
Determine if a Boolean formula can be satisfied.
- **Complexity**: NP-complete (Cook-Levin theorem)
- **Applications**: Hardware verification, AI planning, scheduling

### Traveling Salesman Problem (TSP)
Find the shortest route visiting all cities exactly once.
- **Complexity**: NP-hard
- **Applications**: Logistics, circuit design, DNA sequencing

### Custom Problems
User-submitted computational challenges with bounties.
- **Complexity**: Variable (verified NP-hard)
- **Applications**: Research, optimization, real-world problems

---

## 📊 Dataset Schema

```json
{
  "problem_id": "uuid-v4",
  "problem_type": "SubsetSum | SAT3 | TSP | Custom",
  "problem_data": {
    "elements": [1, 2, 3, ...],
    "target": 42
  },
  "solution_data": {
    "selected_indices": [0, 2, 5],
    "selected_elements": [1, 3, 10]
  },
  
  "block_height": 12345,
  "timestamp": 1733500000,
  "block_hash": "0x1a2b3c...",
  "prev_block_hash": "0x9f8e7d...",
  
  "work_score": 100.0,
  "solution_quality": 1.0,
  "problem_complexity": 3.5,
  "bounty": "1000000",
  
  "solve_time_us": 150000,
  "verify_time_us": 1200,
  "energy_ratio": 1920.5,
  
  "solver": "12D3KooW...",
  "submitter": "12D3KooW...",
  
  "network_metrics": {
    "hash_rate": 1.5,
    "peer_count": 25,
    "consensus_agreement": 0.95
  }
}
```

### Field Descriptions

| Field | Type | Description |
|-------|------|-------------|
| `problem_id` | string | Unique identifier for the problem |
| `problem_type` | enum | Category of NP-hard problem |
| `problem_data` | object | Problem-specific input data |
| `solution_data` | object | Verified solution |
| `block_height` | integer | Block number in the chain |
| `timestamp` | integer | Unix timestamp of block creation |
| `work_score` | float | Computational work performed |
| `solution_quality` | float | Optimality measure (1.0 = optimal) |
| `problem_complexity` | float | Estimated problem difficulty |
| `bounty` | string | Reward in microCOIN (u128 as string) |
| `solve_time_us` | integer | Solution time in microseconds |
| `verify_time_us` | integer | Verification time in microseconds |
| `energy_ratio` | float | Verification/solve energy ratio |

---

## 🚀 Quick Start

### Load with Hugging Face Datasets

```python
from datasets import load_dataset

# Load the full dataset
dataset = load_dataset("COINjecture/v5")

# Iterate through solutions
for record in dataset["train"]:
    print(f"Block {record['block_height']}: {record['problem_type']}")
    print(f"  Work Score: {record['work_score']}")
    print(f"  Energy Ratio: {record.get('energy_ratio', 'N/A')}x")
```

### Filter by Problem Type

```python
# Get only SAT problems
sat_problems = dataset["train"].filter(
    lambda x: x["problem_type"] == "SAT3"
)

# Get only high-complexity problems
hard_problems = dataset["train"].filter(
    lambda x: x["problem_complexity"] > 4.0
)
```

### Stream Large Datasets

```python
# Stream without downloading entire dataset
dataset = load_dataset("COINjecture/v5", streaming=True)

for record in dataset["train"]:
    process(record)
```

---

## 📈 Statistics

| Metric | Value |
|--------|-------|
| **Update Frequency** | Real-time (every block) |
| **Avg Block Time** | ~30 seconds |
| **Problem Types** | 4 |
| **Verification Rate** | 100% |
| **Energy Asymmetry** | >1000x (v5 with ADZDB) |

---

## 🔬 Research Applications

### Machine Learning
- Train models to predict problem difficulty
- Learn heuristics for NP-hard optimization
- Benchmark solver algorithms

### Cryptography
- Study hash function distributions
- Analyze computational hardness assumptions
- Research post-quantum implications

### Distributed Systems
- Study consensus mechanisms
- Analyze network behavior under load
- Research incentive-compatible protocols

---

## 🧮 Empirical Tokenomics (v5)

v5 introduces **zero hardcoded constants**. All economic parameters are derived from live network state:

```
┌─────────────────────────────────────────────────────────────────┐
│                    NETWORK METRICS ORACLE                       │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  Hash Rate    ──→  Emission Bounds                             │
│  Solve Times  ──→  Problem Hardness Factor                     │
│  Median Fees  ──→  Base Storage Cost                           │
│  Stake Dist   ──→  Staking Thresholds                          │
│  Fault Impact ──→  Reputation Severities                       │
│                                                                 │
│  Formula: value = f(network_state)                             │
│  Result:  Self-regulating, governance-free economics           │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## 📱 LightSync Protocol

v5 implements **FlyClient** with Merkle Mountain Ranges for O(log n) chain verification:

| Protocol | Bandwidth | Use Case |
|----------|-----------|----------|
| Full Sync | O(n) | Archive nodes |
| SPV | O(n) headers | Desktop wallets |
| **FlyClient** | **O(log n)** | **Mobile devices** |

For 1M blocks:
- SPV: ~80 MB of headers
- FlyClient: ~50 KB (proofs + sampled headers)

---

## 🔗 Related Resources

| Resource | Link |
|----------|------|
| **v4 Dataset** | [COINjecture/NP_Solutions_v4](https://huggingface.co/datasets/COINjecture/NP_Solutions_v4) |
| **v3 Dataset** | [COINjecture/NP_Solutions_v3](https://huggingface.co/datasets/COINjecture/NP_Solutions_v3) |
| **GitHub** | [Quigles1337/GITHUB-SUCKS](https://github.com/Quigles1337/GITHUB-SUCKS) |
| **Website** | [coinjecture.com](https://coinjecture.com) |

---

## 📜 Citation

```bibtex
@dataset{coinjecture_v5_2024,
  title     = {COINjecture NP Solutions Dataset v5},
  author    = {COINjecture Network Contributors},
  year      = {2024},
  publisher = {Hugging Face},
  url       = {https://huggingface.co/datasets/COINjecture/v5},
  note      = {Real-time NP-hard problem solutions from Proof-of-Useful-Work blockchain}
}
```

---

## ⚖️ License

This dataset is released under the **MIT License**. You are free to use, modify, and distribute the data for any purpose, including commercial applications.

---

<div align="center">

**Built with 🧠 by the COINjecture community**

*Where every hash solves something meaningful*

</div>

