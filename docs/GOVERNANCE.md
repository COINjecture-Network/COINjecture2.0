# COINjecture Governance

## Overview

COINjecture uses **multi-dimensional stake-weighted governance** where voting power is proportional to both the *amount* and *duration* of stake across all dimensional pools.  This prevents whale capture by rewarding long-term participants over short-term speculators.

```
voting_power = Σ (balance_n × D_n × unlock_n(τ))
```

Where:
- `balance_n` = token balance in pool n
- `D_n = e^(−τ_n / √2)` = dimensional scale for pool n
- `unlock_n(τ) = 1 − e^(−η × τ)` = time-weighted unlock factor (η = 1/√2)

---

## Proposal Types

| Type | Approval | Participation | Timelock | Description |
|------|----------|---------------|----------|-------------|
| `Parameter` | 50% | 10% | 10 000 blocks (~1 day) | Network parameter changes |
| `Treasury` | 50% | 15% | 20 000 blocks (~2 days) | Treasury fund allocation |
| `Upgrade` | 80% | 20% | 50 000 blocks (~5 days) | Protocol version upgrades |
| `Constitutional` | 80% | 30% | 100 000 blocks (~10 days) | Governance rule changes |
| `Emergency` | 90% | 5% | 0 (immediate) | Emergency halt/resume |

---

## Proposal Lifecycle

```
  create_proposal()
        │
        ▼
   [ Pending ]  ──── current_block >= voting_starts ────▶  [ Active ]
        │                                                        │
        │                                           current_block > voting_ends
        │                                                        │
        │                                          ┌─────────────┴────────────┐
        │                                      pass?                      fail?
        │                                          │                          │
        │                                     [ Passed ]               [ Failed ]
        │                                          │
        │                             current_block >= execution_at
        │                                          │
        │                                   execute_proposal()
        │                                          │
        │                                    [ Executed ]
        │
  cancel() ──────────────────────────────────────────────────▶ [ Cancelled ]
```

### State Transitions

| From | Event | To |
|------|-------|----|
| `Pending` | `current_block >= voting_starts` | `Active` |
| `Active` | Voting period ends, quorum + approval met | `Passed` |
| `Active` | Voting period ends, quorum or approval not met | `Failed` |
| `Passed` | `execute_proposal()` called after timelock | `Executed` |
| `Pending` or `Active` | Proposer cancels | `Cancelled` |

---

## On-Chain Actions (ProposalAction)

When a proposal passes, its `action` field is executed:

### `ChangeParameter`

Change a named network parameter.  Parameters use dot-path keys:

| Key | Type | Description |
|-----|------|-------------|
| `mempool.max_tx_per_block` | u64 | Maximum transactions per block |
| `consensus.block_time_target_ms` | u64 | Target block time in milliseconds |
| `consensus.difficulty_window` | u64 | Difficulty adjustment window (blocks) |
| `tokenomics.emission_rate_bps` | u64 | Emission rate in basis points |
| `tokenomics.treasury_fee_bps` | u64 | Treasury allocation from block reward |
| `network.max_peers` | u64 | Maximum peer connections |
| `governance.voting_period_blocks` | u64 | Voting period length |

Example:
```json
{
  "action": {
    "ChangeParameter": {
      "key": "mempool.max_tx_per_block",
      "old_value": "1000",
      "new_value": "2000"
    }
  }
}
```

### `ProtocolUpgrade`

Activate a new CPP protocol version at a specific block height:

```json
{
  "action": {
    "ProtocolUpgrade": {
      "target_version": 3,
      "activation_height": 500000,
      "description": "Enable Noise XX encrypted transport"
    }
  }
}
```

After execution, nodes enforce the new `MIN_SUPPORTED_VERSION` at `activation_height`.

### `TreasuryTransfer`

Transfer funds from the treasury pool:

```json
{
  "action": {
    "TreasuryTransfer": {
      "recipient": "0x...",
      "amount": 1000000000000,
      "purpose": "Bug bounty for critical security fix"
    }
  }
}
```

### `ConstitutionalAmendment`

Amend governance parameters themselves:

```json
{
  "action": {
    "ConstitutionalAmendment": {
      "amendment_text": "Increase voting period for non-emergency proposals",
      "new_voting_period_blocks": 150000
    }
  }
}
```

### `EmergencyAction`

Immediate action in response to a security incident:

```json
{
  "action": {
    "EmergencyAction": {
      "action_type": "PauseNetwork",
      "reason": "Critical vulnerability in block validation"
    }
  }
}
```

---

## Voting Power

Voting power is calculated at the time of the vote:

```rust
voting_power = Σ (balance_n × D_n × unlock_n(τ))

// Where unlock factor grows with staking duration:
unlock_n(τ) = 1 - e^(-η × τ)
// τ = blocks_staked / 100_000  (~1 year = 100k blocks)
// η = 1/√2 ≈ 0.7071
```

**Pool dimensional scales (D_n):**

| Pool | Scale | Description |
|------|-------|-------------|
| Genesis | 1.000 | τ=0, genesis stakers |
| Liquid | 0.750 | τ=0.41, liquid staking |
| Validator | 0.618 | τ=0.68, validator stake |
| Bounty | 0.500 | τ=0.98, bounty pool |
| Treasury | 0.382 | τ=1.36, treasury reserve |

---

## Proposal Creation Requirements

- The proposer must hold voting power ≥ `total_voting_power × Δ_critical` (≈23.1%)
- `Δ_critical` is derived from the staking module's `delta_critical()` function

---

## Governance Security Properties

1. **Whale resistance**: Power scales with staking duration, not just balance.
2. **Quorum requirements**: Proposals require minimum participation to prevent low-turnout capture.
3. **Supermajority for critical changes**: Protocol upgrades and constitutional changes require 80%.
4. **Timelocks**: Mandatory delay between passage and execution allows community response.
5. **Cancellation**: Proposers can cancel their proposals before execution.
6. **Execution receipts**: All executed proposals emit an `ExecutionReceipt` for auditability.

---

## Code Location

| Module | File |
|--------|------|
| Governance types & logic | `tokenomics/src/governance.rs` |
| Pool type definitions | `tokenomics/src/pools.rs` |
| Staking (delta_critical) | `tokenomics/src/staking.rs` |
| Dimensional scales | `tokenomics/src/dimensions.rs` |
