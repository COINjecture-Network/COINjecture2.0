//! Block reward — **COINjecture Network Tokenomics v1.0** (April 2026), on-chain form.
//!
//! ## Whitepaper law (dimensionless `w/√W` shape)
//!
//! **`work_score / √W`**, with **`work_score`** in bits (consensus
//! `log₂(solve_time / verify_time) × quality`) and **`W`** the cumulative sum of work scores since
//! genesis. The ratio is **empirical and self-referential**; absolute issuance scale uses fixed-point
//! **`S`** and consensus **`K`** ([`REWARD_EMISSION_MULTIPLIER`]) without changing how **`w`** or **`W`**
//! are derived from headers.
//!
//! ## Fixed-point ledger atoms (`S = REWARD_FIXED_POINT_SCALE`)
//!
//! **`Balance` on-chain is stored in smallest units (“atoms”).** One **display BEANS** equals
//! **`REWARD_FIXED_POINT_SCALE`** atoms (same idea as wei per ETH). Minted emission per block:
//!
//! ```text
//! mint_atoms = ⌊ w_trunc · S · K / isqrt(W_parent) ⌋
//! ```
//!
//! with **`w_trunc`** the integer truncation of this header’s `work_score` (same as summed into
//! **`W`**) and **`W_parent`** the sum of **`w_trunc`** through the **parent** block, and
//! **`K = REWARD_EMISSION_MULTIPLIER`** (consensus issuance scalar; same **`w`/`W`** inputs).
//!
//! Display: **`BEANS = mint_atoms / S`** (real ratio in ℝ is approximated with sub-BEANS resolution).
//!
//! **Genesis / first harvest.** When **`W_parent = 1`** and **`w_trunc ≥ 1`**, **`isqrt(W_parent) = 1`**
//! so **`mint_atoms = S·K`** → **`K` display BEANS** worth of atoms.
//!
//! **`W_parent == 0`** ⇒ **`0`** (pathological / safety only).

use coinject_core::fixed_point::isqrt;
use coinject_core::Balance;

/// Atoms per one **display** BEANS (`10^12`). On-chain [`Balance`] is in **atoms**.
pub const REWARD_FIXED_POINT_SCALE: u128 = 1_000_000_000_000;

/// Consensus multiplier **`K`** in **`mint = ⌊w·S·K / isqrt(W)⌋`**. Does **not** change how **`w`** or **`W`**
/// are computed — same header truncation and parent cumulative work; scales **absolute** minted atoms.
///
/// **Consensus-breaking** if changed on a live network: all nodes must agree on **`K`**.
/// Tune toward a desired typical **display BEANS** band (e.g. ~0–100 on average) without altering
/// the **`w/√W`** allocation shape (floors aside).
pub const REWARD_EMISSION_MULTIPLIER: u128 = 50;

/// Truncated header work: same summand as `Chain::compute_cumulative_work_tip_db` / cumulative `W`.
#[inline]
pub fn header_work_score_trunc_u128(work_score: f64) -> u128 {
    if !work_score.is_finite() || work_score <= 0.0 {
        return 0;
    }
    (work_score.max(0.0) as u64) as u128
}

/// Block reward calculator: **`⌊ w_trunc · S · K / isqrt(W_parent) ⌋`** atoms (`K` = [`REWARD_EMISSION_MULTIPLIER`]).
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

    /// `⌊ w_trunc · S · K / isqrt(W_parent) ⌋` atoms for `W_parent > 0`; else **`0`**.
    pub fn calculate_block_reward(&self, work_score: f64, parent_cumulative_work: u128) -> Balance {
        if parent_cumulative_work == 0 {
            return 0;
        }
        let w = header_work_score_trunc_u128(work_score);
        let denom = isqrt(parent_cumulative_work).max(1);
        let Some(ws) = w.checked_mul(REWARD_FIXED_POINT_SCALE) else {
            return u128::MAX;
        };
        let Some(num) = ws.checked_mul(REWARD_EMISSION_MULTIPLIER) else {
            return u128::MAX;
        };
        num / denom
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
        assert_eq!(m, REWARD_FIXED_POINT_SCALE * REWARD_EMISSION_MULTIPLIER);
    }

    #[test]
    fn scaled_ratio_matches_intuition() {
        let c = RewardCalculator::new();
        // w=16, W=2 → isqrt(2)=1 → 16*K*S atoms
        let sixteen_beans_atoms = 16 * REWARD_FIXED_POINT_SCALE * REWARD_EMISSION_MULTIPLIER;
        assert_eq!(c.calculate_block_reward(16.9, 2), sixteen_beans_atoms);
        // w=16, W=521 → isqrt(521)=22
        assert!(c.calculate_block_reward(16.0, 521) > 0);
        assert_eq!(
            c.calculate_block_reward(16.0, 521),
            (16 * REWARD_FIXED_POINT_SCALE * REWARD_EMISSION_MULTIPLIER) / 22
        );
    }

    #[test]
    fn small_w_large_w_parent_nonzero_atoms() {
        let c = RewardCalculator::new();
        // w=3, W=26 → isqrt(26)=5
        assert_eq!(
            c.calculate_block_reward(3.0, 26),
            (3 * REWARD_FIXED_POINT_SCALE * REWARD_EMISSION_MULTIPLIER) / 5
        );
    }
}
