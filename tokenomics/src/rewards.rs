//! Block reward — **COINjecture Network Tokenomics v1.0** (April 2026), on-chain form.
//!
//! ## Whitepaper law (dimensionless, no new parameters)
//!
//! **`work_score / W`**, with **`work_score`** in bits (consensus
//! `log₂(solve_time / verify_time) × quality`) and **`W`** the cumulative sum of work scores since
//! genesis. The ratio introduces **no** separate issuance constant: it is **empirical,
//! self-referential, dimensionless**.
//!
//! ## Fixed-point ledger atoms (`S = REWARD_FIXED_POINT_SCALE`)
//!
//! **`Balance` on-chain is stored in smallest units (“atoms”).** One **display BEANS** equals
//! **`REWARD_FIXED_POINT_SCALE`** atoms (same idea as wei per ETH). Minted emission per block:
//!
//! ```text
//! mint_atoms = ⌊ w_trunc · S / W_parent ⌋
//! ```
//!
//! with **`w_trunc`** the integer truncation of this header’s `work_score` (same as summed into
//! **`W`**) and **`W_parent`** the sum of **`w_trunc`** through the **parent** block.
//!
//! Display: **`BEANS = mint_atoms / S`** (real ratio in ℝ is approximated with sub-BEANS resolution).
//!
//! **Genesis / first harvest.** When **`W_parent = w_trunc`**, **`mint_atoms = ⌊S·w_trunc/w_trunc⌋ = S`**
//! → **1 display BEANS** worth of atoms (for **`w_trunc ≥ 1`**).
//!
//! **`W_parent == 0`** ⇒ **`0`** (pathological / safety only).

use coinject_core::Balance;

/// Atoms per one **display** BEANS (`10^12`). On-chain [`Balance`] is in **atoms**.
pub const REWARD_FIXED_POINT_SCALE: u128 = 1_000_000_000_000;

/// Truncated header work: same summand as `Chain::compute_cumulative_work_tip_db` / cumulative `W`.
#[inline]
pub fn header_work_score_trunc_u128(work_score: f64) -> u128 {
    if !work_score.is_finite() || work_score <= 0.0 {
        return 0;
    }
    (work_score.max(0.0) as u64) as u128
}

/// Block reward calculator: **`⌊ w_trunc · S / W_parent ⌋`** atoms.
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

    /// `⌊ w_trunc · S / W_parent ⌋` atoms for `W_parent > 0`; else **`0`**.
    pub fn calculate_block_reward(&self, work_score: f64, parent_cumulative_work: u128) -> Balance {
        if parent_cumulative_work == 0 {
            return 0;
        }
        let w = header_work_score_trunc_u128(work_score);
        let Some(num) = w.checked_mul(REWARD_FIXED_POINT_SCALE) else {
            return u128::MAX;
        };
        num / parent_cumulative_work
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn w_zero_or_parent_zero_yields_zero() {
        let c = RewardCalculator::new();
        assert_eq!(c.calculate_block_reward(16.0, 0), 0);
        assert_eq!(c.calculate_block_reward(0.0, 100), 0);
    }

    #[test]
    fn first_harvest_one_display_beans_of_atoms() {
        let c = RewardCalculator::new();
        let m = c.calculate_block_reward(1.0, 1);
        assert_eq!(m, REWARD_FIXED_POINT_SCALE);
    }

    #[test]
    fn scaled_ratio_matches_intuition() {
        let c = RewardCalculator::new();
        // w=16, W=2 → (16*S)/2 = 8*S atoms (= 8 display BEANS)
        let eight_beans_atoms = 8 * REWARD_FIXED_POINT_SCALE;
        assert_eq!(c.calculate_block_reward(16.9, 2), eight_beans_atoms);
        // w=16, W=521 → positive atoms (old ⌊16/521⌋ was 0)
        assert!(c.calculate_block_reward(16.0, 521) > 0);
        assert_eq!(
            c.calculate_block_reward(16.0, 521),
            (16 * REWARD_FIXED_POINT_SCALE) / 521
        );
    }

    #[test]
    fn small_w_large_w_parent_nonzero_atoms() {
        let c = RewardCalculator::new();
        // w=3, W=26 → old formula 0; new: floor(3*S/26)
        assert_eq!(
            c.calculate_block_reward(3.0, 26),
            (3 * REWARD_FIXED_POINT_SCALE) / 26
        );
    }
}
