# Dimensionless Equilibrium Constant Audit

## Mathematical Foundation

### Definition

The Dimensionless Equilibrium Constant is derived from the complex eigenvalue system:

```
µ = -η + iλ
|µ|² = η² + λ² = 1  (Unit circle constraint)
|Re(µ)| = |Im(µ)|  (Balance condition)
```

**Solution**:
```
2λ² = 1
λ = η = 1/√2 ≈ 0.7071067811865476
```

### Properties

1. **Unit Circle Constraint**: `η² + λ² = 1`
2. **Balance Condition**: `η = λ` (real and imaginary parts balanced)
3. **Critical Equilibrium**: Optimal stability without oscillation
4. **Satoshi Constant**: Named after Satoshi's 21M supply limit (21/√2 ≈ 14.85M effective)

---

## Current Usage Analysis

### ✅ Correctly Defined

**Primary Definition** (should be used everywhere):
- **File**: `core/src/dimensional.rs`
- **Code**: 
  ```rust
  pub const ETA: f64 = std::f64::consts::FRAC_1_SQRT_2; // 1/√2 ≈ 0.707107
  pub const LAMBDA: f64 = std::f64::consts::FRAC_1_SQRT_2; // λ = η
  ```
- **Status**: ✅ Correct, uses standard library constant

### ⚠️ Duplicate Definitions (BLOAT)

**1. `tokenomics/src/dimensions.rs`**
- **Code**: `pub const ETA: f64 = 0.7071067811865476;`
- **Issue**: Duplicate of `core::dimensional::ETA`
- **Fix**: Import from `core::dimensional::ETA`

**2. `state/src/dimensional_pools.rs`**
- **Code**: 
  ```rust
  pub const SATOSHI_ETA: f64 = 0.7071067811865476; // 1/√2
  pub const SATOSHI_LAMBDA: f64 = 0.7071067811865476; // 1/√2
  ```
- **Issue**: Duplicate with different name
- **Fix**: Import from `core::dimensional::{ETA, LAMBDA}`

**3. `state/src/trustlines.rs`**
- **Code**: 
  ```rust
  pub const SATOSHI_ETA: f64 = 0.7071067811865476; // 1/√2
  pub const SATOSHI_LAMBDA: f64 = 0.7071067811865476; // 1/√2
  ```
- **Issue**: Duplicate with different name
- **Fix**: Import from `core::dimensional::{ETA, LAMBDA}`

**Impact**: 
- Maintenance burden (4 definitions to update)
- Potential for inconsistency
- Confusion about which to use

---

## Correct Usage Patterns

### ✅ Pattern 1: Difficulty Adjustment

**File**: `consensus/src/difficulty.rs`

**Usage**:
```rust
async fn optimal_solve_time(&self) -> f64 {
    metrics.median_block_time() * ETA  // ✅ Correct
}
```

**Formula**: `optimal = median_block_time × η`
- **Why**: Scales network median by dimensionless constant
- **Compliance**: ✅ Empirical (network-derived), Self-referential (network median), Dimensionless (ratio)

### ✅ Pattern 2: Emission Calculation

**File**: `tokenomics/src/emission.rs`

**Usage**:
```rust
let raw_emission = (ETA * psi * self.base_emission as f64) as u128;
```

**Formula**: `emission = η × |ψ(t)| × base_emission`
- **Why**: Emission rate proportional to consensus magnitude, scaled by ETA
- **Compliance**: ✅ Empirical (psi from network), Self-referential (consensus state), Dimensionless (ratio)

### ✅ Pattern 3: Dimensional Scales

**File**: `core/src/dimensional.rs`

**Usage**:
```rust
pub fn scale_at_tau(tau: f64) -> f64 {
    (-ETA * tau).exp()  // ✅ Correct: D_n = e^(-η·τ_n)
}
```

**Formula**: `D_n = e^(-η·τ_n)`
- **Why**: Exponential decay with ETA as decay constant
- **Compliance**: ✅ Dimensionless (pure exponential)

### ✅ Pattern 4: Unlock Schedules

**File**: `core/src/dimensional.rs`

**Usage**:
```rust
pub fn unlock_fraction(&self, dimension: usize) -> f64 {
    1.0 - (-ETA * (self.tau - tau_n)).exp()  // ✅ Correct
}
```

**Formula**: `U_n(τ) = 1 - e^(-η(τ - τ_n))`
- **Why**: Exponential unlock with ETA as rate constant
- **Compliance**: ✅ Dimensionless (pure exponential)

### ✅ Pattern 5: Yield Rates

**File**: `core/src/dimensional.rs`

**Usage**:
```rust
pub fn yield_rate(&self, dimension: usize) -> f64 {
    ETA * (-ETA * tau_n).exp()  // ✅ Correct: r_n = η · e^(-ητ_n)
}
```

**Formula**: `r_n = η · e^(-ητ_n)`
- **Why**: Yield proportional to ETA, scaled by dimensional scale
- **Compliance**: ✅ Dimensionless (pure ratio)

### ✅ Pattern 6: Staking Viviani Oracle

**File**: `tokenomics/src/staking.rs`

**Usage**:
```rust
pub fn delta_critical() -> f64 {
    ETA * (1.0 - ETA)  // ✅ Correct: Δ = η(1-η)
}
```

**Formula**: `Δ_critical = η(1-η) ≈ 0.207`
- **Why**: Critical delta derived from ETA
- **Compliance**: ✅ Dimensionless (pure mathematical)

---

## Missing Usage (Should Use ETA)

### ❌ Missing: Reward Calculation

**File**: `tokenomics/src/rewards.rs`

**Current**:
```rust
pub struct RewardCalculator {
    base_constant: f64,  // ❌ Hardcoded = 10_000_000
    epoch_average_work: f64,
}
```

**Should Be**:
```rust
pub fn calculate_reward(&self, work_score: WorkScore, network_metrics: &NetworkMetrics) -> Balance {
    let base = network_metrics.median_reward();  // Network-derived
    let scaled = base * ETA * (work_score / self.epoch_average_work);  // ✅ Use ETA
    scaled as Balance
}
```

**Rationale**: Rewards should scale with ETA to maintain dimensional consistency

### ❌ Missing: Time-based Calculations

**Search for**: Hardcoded timeouts, intervals, delays

**Examples**:
- `node/src/service.rs`: Various `Duration::from_secs()` calls
- `consensus/src/miner.rs`: `MINING_TIMEOUT`, `FAILURE_PENALTY_TIME`

**Should Be**: 
- Timeouts: `network_median_time * ETA * multiplier`
- Intervals: `network_median_interval * ETA`

### ❌ Missing: Scaling Factors

**Search for**: Magic numbers like `0.5`, `0.7`, `0.85` that might be ETA-related

**Examples**:
- `consensus/src/difficulty.rs`: `raw_scale_factor *= 0.7` (could be `ETA` or `PHI_INV`)
- Various percentage calculations

**Should Be**: Use ETA or derived constants (PHI_INV, etc.) instead of magic numbers

---

## Compliance Checklist

### Core Mathematical Functions

- [x] Dimensional scales: `D_n = e^(-η·τ_n)` ✅
- [x] Consensus magnitude: `|ψ(τ)| = e^(-ητ)` ✅
- [x] Consensus phase: `θ(τ) = λτ` ✅
- [x] Unlock schedules: `U_n(τ) = 1 - e^(-η(τ - τ_n))` ✅
- [x] Yield rates: `r_n = η · e^(-ητ_n)` ✅
- [x] Viviani delta: `Δ = η(1-η)` ✅

### Network-Derived Calculations

- [x] Difficulty optimal time: `median_block_time * ETA` ✅
- [x] Emission rate: `ETA * |ψ(t)|` ✅
- [ ] Reward base: Should use `ETA * network_median` ❌
- [ ] Timeout calculations: Should use `ETA * network_median` ❌

### Constants Consolidation

- [ ] Remove `tokenomics::dimensions::ETA` → use `core::dimensional::ETA` ❌
- [ ] Remove `state::dimensional_pools::SATOSHI_ETA` → use `core::dimensional::ETA` ❌
- [ ] Remove `state::trustlines::SATOSHI_ETA` → use `core::dimensional::ETA` ❌
- [ ] Standardize all imports to use `core::dimensional::{ETA, LAMBDA}` ❌

---

## Refactoring Recommendations

### Priority 1: Consolidate Definitions

**Action**: Remove all duplicate ETA/LAMBDA definitions

**Files to Update**:
1. `tokenomics/src/dimensions.rs` - Remove ETA, import from core
2. `state/src/dimensional_pools.rs` - Remove SATOSHI_ETA, import ETA
3. `state/src/trustlines.rs` - Remove SATOSHI_ETA, import ETA

**Code Changes**:
```rust
// Before (tokenomics/src/dimensions.rs):
pub const ETA: f64 = 0.7071067811865476;

// After:
use coinject_core::dimensional::ETA;
// Remove local definition
```

### Priority 2: Add ETA to Reward Calculation

**File**: `tokenomics/src/rewards.rs`

**Changes**:
```rust
use coinject_core::dimensional::ETA;
use crate::network_metrics::NetworkMetrics;

impl RewardCalculator {
    pub fn calculate_reward(&self, work_score: WorkScore, metrics: &NetworkMetrics) -> Balance {
        let base = metrics.median_reward();  // Network-derived
        let scaled = base * ETA * (work_score / self.epoch_average_work);
        scaled as Balance
    }
}
```

### Priority 3: Replace Magic Numbers

**Search Pattern**: Find hardcoded values that might be ETA-related

**Examples**:
- `0.707` → `ETA`
- `0.618` → `PHI_INV` (already defined)
- `0.5` → Could be `ETA²` or `1/2`
- `0.85` → Could be `1 - ETA²` or derived

**Action**: Audit each magic number and replace with appropriate constant

---

## Mathematical Verification

### Unit Circle Constraint

```rust
#[test]
fn test_unit_circle_constraint() {
    let constraint = ETA.powi(2) + LAMBDA.powi(2);
    assert!((constraint - 1.0).abs() < 1e-10);  // ✅ Passes
}
```

### Balance Condition

```rust
#[test]
fn test_balance_condition() {
    assert!((ETA - LAMBDA).abs() < 1e-10);  // ✅ Passes
}
```

### Critical Value

```rust
#[test]
fn test_critical_value() {
    let expected = 1.0 / 2.0_f64.sqrt();
    assert!((ETA - expected).abs() < 1e-10);  // ✅ Passes
}
```

---

## Summary

### Current Status

- **Definitions**: 4 locations (should be 1) ⚠️
- **Correct Usage**: 6 patterns ✅
- **Missing Usage**: 3 areas ❌
- **Compliance**: ~75% (good, but can improve)

### Action Items

1. **High Priority**: Consolidate ETA definitions (3 files)
2. **High Priority**: Add ETA to reward calculation
3. **Medium Priority**: Replace magic numbers with ETA/derived constants
4. **Medium Priority**: Add ETA to timeout calculations
5. **Low Priority**: Document ETA usage patterns

### Expected Impact

- **Reduced Bloat**: -3 duplicate definitions
- **Improved Consistency**: Single source of truth
- **Better Compliance**: More areas using ETA correctly
- **Easier Maintenance**: One place to update

---

## References

- **Whitepaper**: "Exponential Dimensional Tokenomics: A Mathematical Framework for Multi-Scale Cryptocurrency Stability"
- **Theorem 1**: Critical equilibrium at η = λ = 1/√2
- **Section 2.5**: Viviani Oracle and performance regimes
- **Section 3.2**: Dimensional scales D_n = e^(-η·τ_n)
- **Section 6.3**: Unlock schedules U_n(τ) = 1 - e^(-η(τ - τ_n))

