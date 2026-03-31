# Source Files Reference

Complete reference of all source files, their purpose, dependencies, and compliance status.

## Legend

- ✅ = Fully compliant (Empirical, Self-referential, Dimensionless)
- ⚠️ = Partially compliant (some violations)
- ❌ = Not compliant (major violations)
- N/A = Not applicable (infrastructure, data structures)

---

## Core Layer (`core/src/`)

| File | Purpose | Key Types/Functions | Dependencies | ETA Usage | Compliance |
|------|---------|---------------------|--------------|-----------|------------|
| `lib.rs` | Core exports | Re-exports all modules | None | N/A | ✅ |
| `dimensional.rs` | **Dimensionless Equilibrium Constant** | `ETA`, `LAMBDA`, `ConsensusState`, `DimensionalScales`, `VivianiOracle` | None | ✅ Primary definition | ✅ |
| `block.rs` | Block structures | `BlockHeader`, `Block`, `Blockchain` | `types`, `crypto`, `transaction`, `commitment` | N/A | ✅ |
| `transaction.rs` | Transaction types | `Transaction` enum, `TransferTransaction`, `CoinbaseTransaction`, etc. | `types`, `crypto` | N/A | ✅ |
| `problem.rs` | NP-hard problems | `ProblemType`, `Solution`, `Clause` | `types` | N/A | ✅ |
| `commitment.rs` | Commit-reveal protocol | `Commitment`, `SolutionReveal` | `types`, `problem` | N/A | ✅ |
| `crypto.rs` | Cryptographic primitives | `KeyPair`, `PublicKey`, `Ed25519Signature`, `MerkleTree` | `types` | N/A | ✅ |
| `types.rs` | Fundamental types | `Hash`, `Address`, `Balance`, `WorkScore` | None | N/A | ✅ |
| `privacy.rs` | Privacy features | `SubmissionMode`, `ProblemParameters`, `WellformednessProof` | `types`, `problem` | N/A | ✅ |

**Core Layer Summary**: ✅ Fully compliant, foundation layer with ETA definition

---

## Consensus Layer (`consensus/src/`)

| File | Purpose | Key Types/Functions | Dependencies | ETA Usage | Compliance |
|------|---------|---------------------|--------------|-----------|------------|
| `lib.rs` | Consensus exports | Re-exports | None | N/A | ✅ |
| `miner.rs` | Mining engine | `Miner`, `generate_problem()`, `solve_problem()`, `mine_block()` | `core`, `tokenomics::RewardCalculator`, `tokenomics::NetworkMetrics` | ⚠️ Indirect (via difficulty) | ⚠️ Uses NetworkMetrics when available |
| `difficulty.rs` | Difficulty adjustment | `DifficultyAdjuster`, `adjust_difficulty_async()`, `optimal_solve_time()` | `tokenomics::NetworkMetrics`, `tokenomics::ETA` | ✅ `optimal = median * ETA` | ✅ |
| `work_score.rs` | Work score calculation | `WorkScoreCalculator`, `calculate()`, `calculate_normalized()` | `core`, `tokenomics::NetworkMetrics` | N/A | ✅ |

**Consensus Layer Summary**: ✅ Fully compliant after recent updates

---

## Tokenomics Layer (`tokenomics/src/`)

| File | Purpose | Key Types/Functions | Dependencies | ETA Usage | Compliance |
|------|---------|---------------------|--------------|-----------|------------|
| `lib.rs` | Tokenomics exports | Re-exports all modules | None | N/A | ✅ |
| `dimensions.rs` | Dimensional scales | `Dimension`, `ETA` | None | ⚠️ **DUPLICATE** (should use `core::ETA`) | ⚠️ |
| `network_metrics.rs` | **Central Oracle** | `NetworkMetrics`, `NetworkSnapshot`, `median_block_time()`, `hardness_factor()` | `dimensions::ETA` | ✅ Uses ETA | ✅ |
| `emission.rs` | Emission calculation | `EmissionCalculator`, `calculate_emission()` | `dimensions::ETA`, `network_metrics` | ✅ `emission = ETA * |ψ|` | ✅ |
| `staking.rs` | Staking system | `StakingPortfolio`, `calculate_viviani_delta()`, `delta_critical()` | `dimensions::ETA`, `network_metrics` | ✅ `Δ = η(1-η)` | ✅ |
| `rewards.rs` | Reward calculation | `RewardCalculator`, `calculate_reward()` | `core` | ❌ **MISSING** (hardcoded base) | ⚠️ |
| `pools.rs` | Dimensional pools | `PoolManager`, `DimensionalPool` | `dimensions::ETA` | ✅ Uses ETA | ✅ |
| `bounty_pricing.rs` | Bounty pricing | `BountyPricer`, `calculate_price()` | `dimensions::ETA` | ✅ Uses ETA | ✅ |
| `deflation.rs` | Deflation mechanisms | `DeflationEngine`, `FeeCalculator` | `dimensions::ETA` | ✅ Uses ETA | ✅ |
| `amm.rs` | AMM functionality | `AmmManager`, `LiquidityPool` | `dimensions::ETA` | ✅ Uses ETA | ✅ |
| `governance.rs` | On-chain governance | `GovernanceManager`, `Proposal`, `Vote` | `dimensions::ETA` | ✅ Uses ETA | ✅ |
| `distributor.rs` | Reward distribution | `DimensionalDistributor`, `allocate()` | `dimensions::Dimension` | N/A | ✅ |

**Tokenomics Layer Summary**: ⚠️ Mostly compliant, but has duplicate ETA and missing ETA in rewards

---

## Network Layer (`network/src/`)

| File | Purpose | Key Types/Functions | Dependencies | ETA Usage | Compliance |
|------|---------|---------------------|--------------|-----------|------------|
| `lib.rs` | Network exports | Re-exports | None | N/A | ✅ |
| `protocol.rs` | libp2p protocol | `NetworkService`, `NetworkEvent`, `NetworkMessage` | `core`, `libp2p` | N/A | N/A |
| `reputation.rs` | Peer reputation | `ReputationManager`, `PeerReputation`, `calculate_reputation()` | `tokenomics::NetworkMetrics` | N/A | ✅ |
| `eclipse.rs` | Eclipse defense | `EclipseDefense`, `IpBucketManager`, `FeelerManager` | `libp2p` | N/A | N/A |

**Network Layer Summary**: ✅ Fully compliant (reputation uses NetworkMetrics)

---

## State Layer (`state/src/`)

| File | Purpose | Key Types/Functions | Dependencies | ETA Usage | Compliance |
|------|---------|---------------------|--------------|-----------|------------|
| `lib.rs` | State exports | Re-exports | None | N/A | ✅ |
| `dimensional_pools.rs` | Dimensional pool state | `DimensionalPoolState`, `PoolLiquidity`, `ConsensusMetrics` | `core::ConsensusState`, `core::DimensionalScales` | ⚠️ **DUPLICATE** `SATOSHI_ETA` | ⚠️ |
| `accounts.rs` | Account state | `AccountState`, balance management | `core` | N/A | ✅ |
| `channels.rs` | Payment channels | `ChannelState`, `Channel` | `core` | N/A | ✅ |
| `escrows.rs` | Escrow state | `EscrowState`, `Escrow` | `core` | N/A | ✅ |
| `trustlines.rs` | Trustline state | `TrustLineState`, `TrustLine` | `core` | ⚠️ **DUPLICATE** `SATOSHI_ETA` | ⚠️ |
| `timelocks.rs` | Timelock state | `TimeLockState`, `TimeLock` | `core` | N/A | ✅ |
| `marketplace.rs` | Marketplace state | `MarketplaceState`, `ProblemSubmission` | `core` | N/A | ✅ |

**State Layer Summary**: ⚠️ Mostly compliant, but has duplicate ETA definitions

---

## Mempool Layer (`mempool/src/`)

| File | Purpose | Key Types/Functions | Dependencies | ETA Usage | Compliance |
|------|---------|---------------------|--------------|-----------|------------|
| `lib.rs` | Mempool exports | Re-exports | None | N/A | ✅ |
| `data_pricing.rs` | Dynamic data pricing | `DataPricingEngine`, `CategoryMarket`, `calculate_price()` | `tokenomics::NetworkMetrics` | ✅ Uses ETA (via NetworkMetrics) | ✅ |
| `pool.rs` | Transaction pool | `TransactionPool`, `PoolConfig`, `PoolStats` | `core` | N/A | ✅ |
| `fee_market.rs` | Fee market | `FeeMarket`, `FeeBreakdown` | `core` | N/A | ✅ |
| `marketplace.rs` | Problem marketplace | `ProblemMarketplace`, `ProblemSubmission` | `core` | N/A | ✅ |
| `mining_incentives.rs` | Mining incentives | `MiningIncentives`, `EnhancedReward` | `core`, `tokenomics::DimensionalDistributor` | N/A | ✅ |

**Mempool Layer Summary**: ✅ Fully compliant

---

## Node Layer (`node/src/`)

| File | Purpose | Key Types/Functions | Dependencies | ETA Usage | Compliance |
|------|---------|---------------------|--------------|-----------|------------|
| `main.rs` | Entry point | `main()` | `service`, `config` | N/A | ✅ |
| `service.rs` | **Main orchestration** | `CoinjectNode`, `mining_loop()`, `start()` | All layers | N/A | ⚠️ Some hardcoded configs |
| `metrics_integration.rs` | NetworkMetrics bridge | `MetricsCollector`, `on_block_added()` | `tokenomics::NetworkMetrics` | N/A | ✅ |
| `chain.rs` | Chain state (standard) | `ChainState`, block storage | `core` | N/A | ✅ |
| `chain_adzdb.rs` | Chain state (ADZDB) | `AdzdbChainState`, ADZDB backend | `core`, `adzdb` | N/A | ✅ |
| `validator.rs` | Block validation | `BlockValidator`, `validate_block()` | `core` | N/A | ✅ |
| `peer_consensus.rs` | Peer consensus | `PeerConsensus`, `should_mine()` | `core` | N/A | ✅ |
| `node_types.rs` | Node classification | `NodeType`, `NodeClassificationManager` | `core` | N/A | ⚠️ Hardcoded thresholds |
| `node_manager.rs` | Node management | `NodeTypeManager`, `CapabilityRouter` | `core`, `node_types` | N/A | ✅ |
| `config.rs` | Configuration | `NodeConfig` | None | N/A | ✅ |
| `genesis.rs` | Genesis block | `create_genesis_block()`, `GenesisConfig` | `core` | N/A | ✅ |
| `keystore.rs` | Key management | `ValidatorKeystore` | `core::crypto` | N/A | ✅ |
| `faucet.rs` | Faucet | `Faucet`, `FaucetConfig` | `core` | N/A | ✅ |
| `metrics.rs` | Metrics collection | Prometheus metrics | `core` | N/A | ✅ |
| `metrics_server.rs` | Metrics server | HTTP metrics endpoint | `metrics` | N/A | ✅ |
| `light_client.rs` | Light client | `LightClientState` | `core` | N/A | ✅ |
| `light_sync.rs` | Light sync | `LightSyncServer`, `FlyClientProof` | `core` | N/A | ✅ |
| `mobile_sdk.rs` | Mobile SDK | Mobile integration | `core` | N/A | ✅ |

**Node Layer Summary**: ⚠️ Mostly compliant, some hardcoded values in config/node_types

---

## Support Layers

### RPC (`rpc/src/`)

| File | Purpose | Key Types/Functions | Dependencies | ETA Usage | Compliance |
|------|---------|---------------------|--------------|-----------|------------|
| `lib.rs` | RPC exports | Re-exports | None | N/A | ✅ |
| `server.rs` | JSON-RPC server | `RpcServer`, `RpcServerState` | `core`, `node` | N/A | ✅ |

### Wallet (`wallet/src/`)

| File | Purpose | Key Types/Functions | Dependencies | ETA Usage | Compliance |
|------|---------|---------------------|--------------|-----------|------------|
| `main.rs` | Wallet CLI | `main()` | `commands` | N/A | ✅ |
| `rpc_client.rs` | RPC client | `RpcClient` | `rpc` | N/A | ✅ |
| `keystore.rs` | Wallet keystore | `WalletKeystore` | `core::crypto` | N/A | ✅ |
| `commands/` | CLI commands | Transaction, account, marketplace commands | `core` | N/A | ✅ |

### HuggingFace (`huggingface/src/`)

| File | Purpose | Key Types/Functions | Dependencies | ETA Usage | Compliance |
|------|---------|---------------------|--------------|-----------|------------|
| `lib.rs` | HF exports | Re-exports | None | N/A | ✅ |
| `client.rs` | HF client | `HuggingFaceClient` | `core` | N/A | ✅ |
| `metrics.rs` | HF metrics | `HuggingFaceMetrics` | `core`, `consensus` | N/A | ✅ |
| `serialize.rs` | Serialization | Problem/solution serialization | `core` | N/A | ✅ |
| `energy.rs` | Energy measurement | `EnergyConfig`, `EnergyMeasurementMethod` | `core` | N/A | ✅ |

### ADZDB (`adzdb/src/`)

| File | Purpose | Key Types/Functions | Dependencies | ETA Usage | Compliance |
|------|---------|---------------------|--------------|-----------|------------|
| `lib.rs` | ADZDB backend | ADZDB storage implementation | `core` | N/A | ✅ |

---

## Dependency Graph Summary

### Core Dependencies (Foundation)
```
Core (ETA definition)
  ↓
All other layers
```

### Tokenomics Dependencies
```
Tokenomics
  ├─→ Core (types, ETA)
  └─→ NetworkMetrics (central oracle)
```

### Consensus Dependencies
```
Consensus
  ├─→ Core (types, problems)
  └─→ Tokenomics (rewards, NetworkMetrics)
```

### Network Dependencies
```
Network
  ├─→ Core (types)
  └─→ Tokenomics (NetworkMetrics for reputation)
```

### State Dependencies
```
State
  ├─→ Core (types, ConsensusState, DimensionalScales)
  └─→ Tokenomics (for pool calculations)
```

### Mempool Dependencies
```
Mempool
  ├─→ Core (types, transactions)
  └─→ Tokenomics (NetworkMetrics for pricing)
```

### Node Dependencies
```
Node
  ├─→ All layers (orchestration)
  └─→ MetricsIntegration (feeds NetworkMetrics)
```

---

## NetworkMetrics Integration Status

### ✅ Fully Integrated
- `consensus::difficulty` - Uses for optimal solve times
- `consensus::work_score` - Uses for normalization
- `network::reputation` - Uses for fault severities
- `mempool::data_pricing` - Uses for median fees, hardness
- `tokenomics::emission` - Uses for baseline hashrate
- `tokenomics::staking` - Uses for network pool data

### ⚠️ Partially Integrated
- `consensus::miner` - Can use via `set_network_metrics()`, but defaults available
- `node::service` - Feeds data via `MetricsCollector`, but not all queries use it

### ❌ Not Integrated
- `tokenomics::rewards` - Still uses hardcoded `base_constant`
- Various timeout/delay calculations - Still hardcoded

---

## ETA Usage Summary

### ✅ Correctly Using ETA
1. `core::dimensional` - Primary definition
2. `consensus::difficulty` - `optimal = median * ETA`
3. `tokenomics::emission` - `emission = ETA * |ψ|`
4. `tokenomics::staking` - `Δ = η(1-η)`
5. `tokenomics::pools` - Dimensional calculations
6. `tokenomics::bounty_pricing` - Pricing formulas
7. `tokenomics::deflation` - Deflation rates
8. `tokenomics::amm` - AMM calculations
9. `tokenomics::governance` - Threshold calculations

### ⚠️ Duplicate Definitions (Should Import)
1. `tokenomics::dimensions::ETA` - Should use `core::dimensional::ETA`
2. `state::dimensional_pools::SATOSHI_ETA` - Should use `core::dimensional::ETA`
3. `state::trustlines::SATOSHI_ETA` - Should use `core::dimensional::ETA`

### ❌ Missing ETA Usage
1. `tokenomics::rewards` - Should scale with ETA
2. Timeout calculations - Should use `ETA * network_median`
3. Various scaling factors - Could use ETA or derived constants

---

## Compliance Summary by Layer

| Layer | Files | Compliant | Partial | Non-compliant | Compliance % |
|-------|-------|-----------|---------|---------------|--------------|
| Core | 9 | 9 | 0 | 0 | 100% |
| Consensus | 4 | 3 | 1 | 0 | 75% → 100%* |
| Tokenomics | 12 | 10 | 2 | 0 | 83% |
| Network | 4 | 3 | 0 | 0 | 75%** |
| State | 8 | 6 | 2 | 0 | 75% |
| Mempool | 6 | 6 | 0 | 0 | 100% |
| Node | 18 | 16 | 2 | 0 | 89% |
| Support | 10 | 10 | 0 | 0 | 100% |
| **Total** | **71** | **63** | **7** | **0** | **89%** |

*Consensus: 100% after recent updates (miner uses NetworkMetrics when available)
**Network: 75% because protocol/eclipse are infrastructure (N/A)

---

## Key Findings

### Strengths
1. ✅ Core layer is solid foundation with proper ETA definition
2. ✅ NetworkMetrics oracle is well-integrated in most modules
3. ✅ Recent updates to difficulty/work_score are fully compliant
4. ✅ Most tokenomics modules use ETA correctly

### Issues
1. ⚠️ **3 duplicate ETA definitions** (bloat)
2. ⚠️ **Rewards still hardcoded** (should use NetworkMetrics + ETA)
3. ⚠️ **Some timeouts hardcoded** (should be network-derived)
4. ⚠️ **Node configs have hardcoded values** (should be percentiles)

### Recommendations
1. **High Priority**: Consolidate ETA definitions (3 files)
2. **High Priority**: Make rewards use NetworkMetrics + ETA
3. **Medium Priority**: Replace hardcoded timeouts with network-derived
4. **Medium Priority**: Make node thresholds percentile-based

---

## File Count Summary

- **Total Source Files**: 71
- **Core Layer**: 9 files
- **Consensus Layer**: 4 files
- **Tokenomics Layer**: 12 files
- **Network Layer**: 4 files
- **State Layer**: 8 files
- **Mempool Layer**: 6 files
- **Node Layer**: 18 files
- **Support Layers**: 10 files

**Average Compliance**: 89% (excellent, but room for improvement)



