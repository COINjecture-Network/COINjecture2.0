//! Block reward: asymptotic decay, **no emission floor** (large cumulative `W` ⇒ reward may be `0`).
//!
//! ## Whitepaper (COINjecture tokenomics, April 2026)
//!
//! Dimensionless issuance is **`block_reward = work_score / W`**, with **`W`** the cumulative sum of
//! work scores since genesis (through the **parent** of the block for the denominator used here).
//!
//! ## On-chain discretization (whole `Balance` only)
//!
//! **`Balance` is `u128`**. The implemented law matches the same **truncation** as cumulative **`W`**
//! in `node/src/chain.rs`: each header contributes **`(work_score.max(0.0) as u64) as u128`**. Emission is
//!
//! ```text
//! block_reward = ⌊ w_trunc / W_parent ⌋
//! ```
//!
//! with **`w_trunc`** from **this** block’s header **`work_score`**, and **`W_parent`** = Σ **`w_trunc`**
//! on the parent chain. No separate legacy scale **`B`**: when **`w_trunc = W_parent = 1`**, the first
//! mined block after genesis with minimal work mints **1** ledger unit (the integer analogue of “first
//! harvest = 1 $BEANS”). When **`w_trunc < W_parent`**, the floor is **`0`**.
//!
//! `W_parent == 0` ⇒ **`0`** (pathological / safety only).

use coinject_core::Balance;

/// Truncated header work: same summand as `Chain::compute_cumulative_work_tip_db` / cumulative `W`.
#[inline]
pub fn header_work_score_trunc_u128(work_score: f64) -> u128 {
    if !work_score.is_finite() || work_score <= 0.0 {
        return 0;
    }
    (work_score.max(0.0) as u64) as u128
}

/// Block reward calculator (whitepaper-aligned integer **`work_score / W`**).
pub struct RewardCalculator;

impl Default for RewardCalculator {
    fn default() -> Self {
        Self::new()
    }
}

impl RewardCalculator {
    pub fn new() -> Self {
        RewardCalculator
    }

    /// `⌊ w_trunc / W_parent ⌋` for `W_parent > 0`; else **`0`**.
    pub fn calculate_block_reward(&self, work_score: f64, parent_cumulative_work: u128) -> Balance {
        if parent_cumulative_work == 0 {
            return 0;
        }
        let w = header_work_score_trunc_u128(work_score);
        w.checked_div(parent_cumulative_work).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn w_zero_yields_zero() {
        let c = RewardCalculator::new();
        assert_eq!(c.calculate_block_reward(16.0, 0), 0);
    }

    #[test]
    fn first_harvest_integer_one() {
        let c = RewardCalculator::new();
        assert_eq!(c.calculate_block_reward(1.0, 1), 1);
    }

    #[test]
    fn ratio_floor_matches_trunc_work() {
        let c = RewardCalculator::new();
        // w_trunc = 16, W = 2 → 8
        assert_eq!(c.calculate_block_reward(16.9, 2), 8);
        // w_trunc = 16, W = 521 → 0
        assert_eq!(c.calculate_block_reward(16.0, 521), 0);
    }
}
