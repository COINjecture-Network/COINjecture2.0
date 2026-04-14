//! Block reward — **COINjecture Network Tokenomics v1.0** (April 2026), on-chain form.
//!
//! ## Whitepaper law (dimensionless, no new parameters)
//!
//! **`block_reward = work_score / W`**, with **`work_score`** in bits (consensus
//! `log₂(solve_time / verify_time) × quality`) and **`W`** the cumulative sum of work scores since
//! genesis. The ratio introduces **no** separate issuance constant: it is **empirical,
//! self-referential, dimensionless**.
//!
//! **Genesis / first harvest.** When **`W = work_score`** (e.g. extending a chain whose parent
//! cumulative work already reflects genesis, and this header’s work matches that scale), the ratio is
//! **1** → **1 $BEANS** — a **mathematical consequence**, not an extra rule or “base reward” knob.
//!
//! **Supply.** Unbounded; asymptotically constrained by **`W`**. Natural deflation: reward → **0** as
//! **`W` → ∞** (smooth in ℝ; see below for ledger discretization).
//!
//! ## On-chain discretization (whole `Balance` only)
//!
//! **`Balance` is `u128`**. Cumulative **`W`** uses the same **truncation** as
//! `node/src/chain.rs`: each header contributes **`(work_score.max(0.0) as u64) as u128`**. Minted
//! emission is the integer floor of the whitepaper ratio, using **this** block’s truncated
//! **`w_trunc`** and **`W_parent`** (Σ `w_trunc` through the **parent**):
//!
//! ```text
//! block_reward = ⌊ w_trunc / W_parent ⌋
//! ```
//!
//! So the implementation is exactly **`work_score / W`** in ledger atoms, with **one** $BEANS atom
//! as the unit — not a scaled numerator like **`10⁷`**. When **`w_trunc < W_parent`**, **`⌊·⌋`** may
//! be **`0`** even though the real ratio is positive and arbitrarily small (germination → growth →
//! maturity in the whitepaper).
//!
//! **`W_parent == 0`** ⇒ **`0`** (pathological / safety only).

use coinject_core::Balance;

/// Truncated header work: same summand as `Chain::compute_cumulative_work_tip_db` / cumulative `W`.
#[inline]
pub fn header_work_score_trunc_u128(work_score: f64) -> u128 {
    if !work_score.is_finite() || work_score <= 0.0 {
        return 0;
    }
    (work_score.max(0.0) as u64) as u128
}

/// Block reward calculator: whitepaper **`work_score / W`**, integer floor for [`Balance`].
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
    fn first_harvest_integer_one_when_ratio_one() {
        let c = RewardCalculator::new();
        // Whitepaper: W = work_score ⇒ work_score/W = 1 → 1 $BEANS (trunc path: 1/1)
        assert_eq!(c.calculate_block_reward(1.0, 1), 1);
    }

    #[test]
    fn ratio_floor_matches_trunc_work() {
        let c = RewardCalculator::new();
        // w_trunc = 16, W = 2 → ⌊16/2⌋ = 8
        assert_eq!(c.calculate_block_reward(16.9, 2), 8);
        // w_trunc = 16, W = 521 → 0
        assert_eq!(c.calculate_block_reward(16.0, 521), 0);
    }
}
