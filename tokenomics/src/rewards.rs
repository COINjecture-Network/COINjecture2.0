// Block reward (asymptotic decay, no emission floors)
//
// Canonical law: `block_reward = base_reward × (1/W)` with `W` = parent-chain cumulative work
// (Σ truncated header.work_score through the parent block). Over ℤ, this is implemented as
// `⌊base_reward / W⌋` = `base_reward.checked_div(W)` for `W > 0`; `W == 0` ⇒ 0 (safety only).

use coinject_core::Balance;

pub struct RewardCalculator {
    /// Scale constant `B` in `B × (1/W)` (same units as `Balance` / on-chain amounts).
    base_reward: u128,
}

impl Default for RewardCalculator {
    fn default() -> Self {
        Self::new()
    }
}

impl RewardCalculator {
    pub fn new() -> Self {
        RewardCalculator {
            base_reward: 10_000_000, // 10M — matches prior test scale order of magnitude
        }
    }

    /// `block_reward = base_reward × (1/W)` as `⌊base_reward / W⌋` for `W > 0`; `W == 0` ⇒ 0.
    pub fn calculate_block_reward(&self, parent_cumulative_work: u128) -> Balance {
        if parent_cumulative_work == 0 {
            return 0;
        }
        self.base_reward
            .checked_div(parent_cumulative_work)
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn w_zero_yields_zero() {
        let c = RewardCalculator::new();
        assert_eq!(c.calculate_block_reward(0), 0);
    }

    #[test]
    fn decay_matches_floor_div() {
        let c = RewardCalculator::new();
        assert_eq!(c.calculate_block_reward(1), 10_000_000);
        assert_eq!(c.calculate_block_reward(2), 5_000_000);
        assert_eq!(c.calculate_block_reward(10_000_000), 1);
        assert_eq!(c.calculate_block_reward(10_000_001), 0);
    }
}
