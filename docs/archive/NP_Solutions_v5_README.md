# 🚀 COINjecture NP Solutions v5

> **The Empirical Evolution** - Zero hardcoded constants, all values derived from network state

[![Dataset](https://img.shields.io/badge/HuggingFace-Dataset-yellow)](https://huggingface.co/datasets/COINjecture/v5)
[![License](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

## What's New in v5

| Feature | v4 | v5 |
|---------|----|----|
| **Tokenomics** | Hardcoded constants | 100% empirical (network-derived) |
| **Light Clients** | Basic SPV | FlyClient + MMR proofs |
| **Node Types** | Single type | 6 specialized types |
| **Mobile SDK** | None | WASM + C-FFI ready |
| **Metrics** | Static | Live network oracle |

## 🧮 Empirical Tokenomics

v5 introduces **zero hardcoded constants**. Every economic parameter is derived from live network state:

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

### Key Formulas

| Parameter | v4 (Hardcoded) | v5 (Empirical) |
|-----------|----------------|----------------|
| `C_BASE` | `1000` | `median_fee()` |
| `H_factor` | `[1.0, 2.0, 4.0, ...]` | `solve_time[cat] / solve_time[baseline]` |
| `MIN_STAKE` | `1000000` | `stake_threshold_percentile(25)` |
| `baseline_hashrate` | `1.0` | `median_hash_rate()` |
| Fault severities | Static values | `avg_reorg_blocks_caused / total_reorg_blocks` |

## 📱 LightSync Protocol (FlyClient)

v5 enables **O(log n) chain verification** using Merkle Mountain Ranges:

```
Traditional SPV:     Download all headers → O(n) bandwidth
FlyClient (v5):      Sample log(n) headers with MMR proofs → O(log n) bandwidth

For 1M blocks:
  SPV:       80MB of headers
  FlyClient: ~500 headers + proofs ≈ 50KB
```

### Proof Types

| Proof | Use Case | Size |
|-------|----------|------|
| `FlyClientProof` | Verify entire chain state | O(log n) |
| `MMRProof` | Verify single block inclusion | O(log n) |
| `TxProof` | Verify transaction in block | O(log n) + O(log m) |

## 🔧 6 Node Types

Nodes are classified **empirically** based on behavior, not self-declaration:

| Type | Storage | Validation | Special Ability |
|------|---------|------------|-----------------|
| **Light** | Headers only | None | Mobile-friendly |
| **Full** | Recent blocks | Full | Serve light clients |
| **Archive** | Complete chain | Full | Historical queries |
| **Validator** | Full + mempool | Active | Block production |
| **Bounty** | Partial | None | Problem solving |
| **Oracle** | Variable | Price feeds | External data |

## 📊 Dataset Schema

```json
{
  "problem_id": "string",
  "problem_type": "SubsetSum | SAT3 | TSP | Custom",
  "problem_data": { ... },
  "solution_data": { ... },
  
  "block_height": 12345,
  "timestamp": 1699999999,
  "block_hash": "0x...",
  "prev_block_hash": "0x...",
  
  "work_score": 100.0,
  "solution_quality": 1.0,
  "problem_complexity": 3.5,
  "bounty": "1000000",
  
  "solve_time_us": 150000,
  "verify_time_us": 1200,
  "energy_ratio": 1920.5,
  
  "network_metrics": {
    "hash_rate": 1.5,
    "peer_count": 25,
    "consensus_agreement": 0.95
  }
}
```

## 🚀 Quick Start

### Load Dataset

```python
from datasets import load_dataset

dataset = load_dataset("COINjecture/NP_Solutions_v5")

for record in dataset["train"]:
    print(f"Block {record['block_height']}: {record['problem_type']}")
    print(f"  Energy ratio: {record.get('energy_ratio', 'N/A')}x")
```

### Compare with v4

```python
v4 = load_dataset("COINjecture/NP_Solutions_v4")
v5 = load_dataset("COINjecture/NP_Solutions_v5")

# v5 includes network metrics oracle data
v5_with_metrics = v5["train"].filter(lambda x: x.get("network_metrics"))
print(f"Records with metrics: {len(v5_with_metrics)}")
```

## 🌐 Node Deployment

### Local 3-Node Network

```powershell
# Windows
.\deploy-v5-network.ps1 -HfToken "hf_your_token_here"

# With ADZDB storage (recommended)
.\deploy-v5-network.ps1 -HfToken "hf_your_token_here" -UseAdzdb
```

### Single Node

```bash
./coinject \
  --mine \
  --hf-token "hf_your_token_here" \
  --hf-dataset-name "COINjecture/NP_Solutions_v5" \
  --verbose
```

## 📈 Migration from v4

v5 is **backward compatible** with v4 data. To migrate:

```python
from datasets import load_dataset, concatenate_datasets

v4 = load_dataset("COINjecture/NP_Solutions_v4")
v5 = load_dataset("COINjecture/NP_Solutions_v5")

# Combine datasets
combined = concatenate_datasets([v4["train"], v5["train"]])
```

## 📚 Related

| Resource | Link |
|----------|------|
| v4 Dataset | [COINjecture/NP_Solutions_v4](https://huggingface.co/datasets/COINjecture/NP_Solutions_v4) |
| v3 Dataset (Legacy) | [COINjecture/NP_Solutions_v3](https://huggingface.co/datasets/COINjecture/NP_Solutions_v3) |
| GitHub | [Quigles1337/GITHUB-SUCKS](https://github.com/Quigles1337/GITHUB-SUCKS) |

## 📜 Citation

```bibtex
@dataset{coinjecture_np_solutions_v5,
  title={COINjecture NP Solutions Dataset v5 (Empirical)},
  author={COINjecture Team},
  year={2024},
  publisher={Hugging Face},
  url={https://huggingface.co/datasets/COINjecture/NP_Solutions_v5},
}
```

---

**v5: Where the network decides its own rules** 🌐

