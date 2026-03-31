// =============================================================================
// Automated Market Maker with Exponential Decay
// x · y = k · e^(-ητ)
// =============================================================================
// Constant product AMM where k decays at rate η = 1/√2

use coinject_core::ETA; // Import from core (single source of truth)
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Liquidity pool between two dimensional tokens
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidityPool {
    /// Pool ID
    pub pool_id: [u8; 32],
    /// Token A identifier (dimensional pool type index)
    pub token_a: u8,
    /// Token B identifier
    pub token_b: u8,
    /// Reserve of token A
    pub reserve_a: u128,
    /// Reserve of token B
    pub reserve_b: u128,
    /// Initial k constant
    pub initial_k: f64,
    /// Creation block (τ = 0)
    pub created_at_block: u64,
    /// Total LP tokens issued
    pub total_lp_tokens: u128,
    /// LP token holders
    pub lp_balances: HashMap<[u8; 32], u128>,
    /// Cumulative fees collected
    pub cumulative_fees: u128,
}

impl LiquidityPool {
    /// Create new pool with initial liquidity
    pub fn new(
        pool_id: [u8; 32],
        token_a: u8,
        token_b: u8,
        amount_a: u128,
        amount_b: u128,
        provider: [u8; 32],
        current_block: u64,
    ) -> Self {
        let initial_k = (amount_a as f64) * (amount_b as f64);
        let lp_tokens = ((amount_a as f64) * (amount_b as f64)).sqrt() as u128;

        let mut lp_balances = HashMap::new();
        lp_balances.insert(provider, lp_tokens);

        LiquidityPool {
            pool_id,
            token_a,
            token_b,
            reserve_a: amount_a,
            reserve_b: amount_b,
            initial_k,
            created_at_block: current_block,
            total_lp_tokens: lp_tokens,
            lp_balances,
            cumulative_fees: 0,
        }
    }

    /// Calculate current k with decay: k(τ) = k₀ · e^(-ητ)
    pub fn current_k(&self, current_block: u64, blocks_per_year: u64) -> f64 {
        let tau = self.tau(current_block, blocks_per_year);
        self.initial_k * (-ETA * tau).exp()
    }

    /// Calculate τ (time in years since pool creation)
    fn tau(&self, current_block: u64, blocks_per_year: u64) -> f64 {
        let blocks_elapsed = current_block.saturating_sub(self.created_at_block);
        (blocks_elapsed as f64) / (blocks_per_year as f64)
    }

    /// Get current price of A in terms of B
    pub fn price_a_in_b(&self) -> f64 {
        if self.reserve_a == 0 {
            return 0.0;
        }
        (self.reserve_b as f64) / (self.reserve_a as f64)
    }

    /// Get current price of B in terms of A
    pub fn price_b_in_a(&self) -> f64 {
        if self.reserve_b == 0 {
            return 0.0;
        }
        (self.reserve_a as f64) / (self.reserve_b as f64)
    }

    /// Swap token A for token B
    /// Returns amount of B received
    pub fn swap_a_for_b(
        &mut self,
        amount_a_in: u128,
        current_block: u64,
        blocks_per_year: u64,
        fee_rate: f64,
    ) -> SwapResult {
        let k = self.current_k(current_block, blocks_per_year);

        // Apply fee
        let fee = ((amount_a_in as f64) * fee_rate) as u128;
        let amount_a_after_fee = amount_a_in - fee;
        self.cumulative_fees += fee;

        // Calculate output: (x + Δx)(y - Δy) = k
        // Δy = y - k/(x + Δx)
        let new_reserve_a = self.reserve_a + amount_a_after_fee;
        let new_reserve_b = (k / (new_reserve_a as f64)) as u128;
        let amount_b_out = self.reserve_b.saturating_sub(new_reserve_b);

        // Update reserves
        let old_price = self.price_a_in_b();
        self.reserve_a = new_reserve_a;
        self.reserve_b = new_reserve_b;
        let new_price = self.price_a_in_b();

        SwapResult {
            amount_in: amount_a_in,
            amount_out: amount_b_out,
            fee_paid: fee,
            price_before: old_price,
            price_after: new_price,
            price_impact: ((new_price - old_price) / old_price).abs(),
        }
    }

    /// Swap token B for token A
    pub fn swap_b_for_a(
        &mut self,
        amount_b_in: u128,
        current_block: u64,
        blocks_per_year: u64,
        fee_rate: f64,
    ) -> SwapResult {
        let k = self.current_k(current_block, blocks_per_year);

        let fee = ((amount_b_in as f64) * fee_rate) as u128;
        let amount_b_after_fee = amount_b_in - fee;
        self.cumulative_fees += fee;

        let new_reserve_b = self.reserve_b + amount_b_after_fee;
        let new_reserve_a = (k / (new_reserve_b as f64)) as u128;
        let amount_a_out = self.reserve_a.saturating_sub(new_reserve_a);

        let old_price = self.price_b_in_a();
        self.reserve_a = new_reserve_a;
        self.reserve_b = new_reserve_b;
        let new_price = self.price_b_in_a();

        SwapResult {
            amount_in: amount_b_in,
            amount_out: amount_a_out,
            fee_paid: fee,
            price_before: old_price,
            price_after: new_price,
            price_impact: ((new_price - old_price) / old_price).abs(),
        }
    }

    /// Add liquidity and receive LP tokens
    pub fn add_liquidity(
        &mut self,
        amount_a: u128,
        amount_b: u128,
        provider: [u8; 32],
    ) -> LiquidityResult {
        // Calculate LP tokens based on share of pool
        let lp_tokens = if self.total_lp_tokens == 0 {
            ((amount_a as f64) * (amount_b as f64)).sqrt() as u128
        } else {
            // Proportional to smaller contribution ratio
            let ratio_a = (amount_a as f64) / (self.reserve_a as f64);
            let ratio_b = (amount_b as f64) / (self.reserve_b as f64);
            let ratio = ratio_a.min(ratio_b);
            ((self.total_lp_tokens as f64) * ratio) as u128
        };

        self.reserve_a += amount_a;
        self.reserve_b += amount_b;
        self.total_lp_tokens += lp_tokens;

        *self.lp_balances.entry(provider).or_insert(0) += lp_tokens;

        LiquidityResult {
            lp_tokens_received: lp_tokens,
            amount_a_deposited: amount_a,
            amount_b_deposited: amount_b,
            share_of_pool: (lp_tokens as f64) / (self.total_lp_tokens as f64),
        }
    }

    /// Remove liquidity by burning LP tokens
    pub fn remove_liquidity(
        &mut self,
        lp_tokens: u128,
        provider: [u8; 32],
    ) -> Option<(u128, u128)> {
        let balance = self.lp_balances.get(&provider)?;
        if *balance < lp_tokens {
            return None;
        }

        let share = (lp_tokens as f64) / (self.total_lp_tokens as f64);
        let amount_a = ((self.reserve_a as f64) * share) as u128;
        let amount_b = ((self.reserve_b as f64) * share) as u128;

        self.reserve_a -= amount_a;
        self.reserve_b -= amount_b;
        self.total_lp_tokens -= lp_tokens;

        *self.lp_balances.get_mut(&provider).unwrap() -= lp_tokens;

        Some((amount_a, amount_b))
    }

    /// Calculate impermanent loss protection
    /// protection = 1 - e^(-η · time_staked)
    pub fn impermanent_loss_protection(
        &self,
        provider: [u8; 32],
        stake_start_block: u64,
        current_block: u64,
        blocks_per_year: u64,
    ) -> f64 {
        let _ = provider; // Could use for per-user tracking
        let time_staked =
            (current_block.saturating_sub(stake_start_block) as f64) / (blocks_per_year as f64);
        1.0 - (-ETA * time_staked).exp()
    }
}

/// Swap result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapResult {
    pub amount_in: u128,
    pub amount_out: u128,
    pub fee_paid: u128,
    pub price_before: f64,
    pub price_after: f64,
    pub price_impact: f64,
}

/// Liquidity provision result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidityResult {
    pub lp_tokens_received: u128,
    pub amount_a_deposited: u128,
    pub amount_b_deposited: u128,
    pub share_of_pool: f64,
}

/// AMM manager for all pools
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmmManager {
    pub pools: HashMap<[u8; 32], LiquidityPool>,
    pub fee_rate: f64,
    pub blocks_per_year: u64,
}

impl AmmManager {
    pub fn new(blocks_per_year: u64) -> Self {
        AmmManager {
            pools: HashMap::new(),
            fee_rate: 0.003, // 0.3% swap fee
            blocks_per_year,
        }
    }

    pub fn create_pool(
        &mut self,
        token_a: u8,
        token_b: u8,
        amount_a: u128,
        amount_b: u128,
        provider: [u8; 32],
        current_block: u64,
    ) -> [u8; 32] {
        // Generate pool ID from tokens
        let mut pool_id = [0u8; 32];
        pool_id[0] = token_a;
        pool_id[1] = token_b;
        pool_id[2..10].copy_from_slice(&current_block.to_le_bytes());

        let pool = LiquidityPool::new(
            pool_id,
            token_a,
            token_b,
            amount_a,
            amount_b,
            provider,
            current_block,
        );

        self.pools.insert(pool_id, pool);
        pool_id
    }

    pub fn get_pool(&self, pool_id: &[u8; 32]) -> Option<&LiquidityPool> {
        self.pools.get(pool_id)
    }

    pub fn get_pool_mut(&mut self, pool_id: &[u8; 32]) -> Option<&mut LiquidityPool> {
        self.pools.get_mut(pool_id)
    }

    pub fn swap(
        &mut self,
        pool_id: &[u8; 32],
        sell_a: bool,
        amount_in: u128,
        current_block: u64,
    ) -> Option<SwapResult> {
        let pool = self.pools.get_mut(pool_id)?;
        let result = if sell_a {
            pool.swap_a_for_b(
                amount_in,
                current_block,
                self.blocks_per_year,
                self.fee_rate,
            )
        } else {
            pool.swap_b_for_a(
                amount_in,
                current_block,
                self.blocks_per_year,
                self.fee_rate,
            )
        };
        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_k_decay() {
        let pool = LiquidityPool::new([0; 32], 1, 2, 1_000_000, 1_000_000, [1; 32], 0);

        let blocks_per_year = 100_000;

        // At creation, k should be initial_k
        let k0 = pool.current_k(0, blocks_per_year);
        assert!((k0 - pool.initial_k).abs() < 1.0);

        // After 1 year, k should decay to k₀ · e^(-η) ≈ k₀ × 0.4932
        let k1 = pool.current_k(blocks_per_year, blocks_per_year);
        let expected = pool.initial_k * (-ETA).exp();
        let ratio = k1 / expected;
        assert!(
            (ratio - 1.0).abs() < 0.01,
            "k1={}, expected={}",
            k1,
            expected
        );
    }

    #[test]
    fn test_swap_maintains_invariant() {
        let mut pool = LiquidityPool::new([0; 32], 1, 2, 1_000_000, 1_000_000, [1; 32], 0);

        let blocks_per_year = 100_000;
        let k_before = pool.current_k(0, blocks_per_year);

        // Swap some A for B
        pool.swap_a_for_b(10_000, 0, blocks_per_year, 0.003);

        // k should be maintained (approximately, due to fees)
        let actual_product = (pool.reserve_a as f64) * (pool.reserve_b as f64);
        // After fee extraction, product will be slightly higher than k
        assert!(actual_product >= k_before * 0.99);
    }

    #[test]
    fn test_impermanent_loss_protection() {
        let pool = LiquidityPool::new([0; 32], 1, 2, 1_000_000, 1_000_000, [1; 32], 0);

        let blocks_per_year = 100_000;

        // At start, no protection
        let prot0 = pool.impermanent_loss_protection([1; 32], 0, 0, blocks_per_year);
        assert!(prot0 < 0.01);

        // After 1 year, protection ≈ 50.68%
        let prot1 = pool.impermanent_loss_protection([1; 32], 0, blocks_per_year, blocks_per_year);
        assert!(
            prot1 > 0.4 && prot1 < 0.6,
            "Protection after 1 year: {}",
            prot1
        );

        // After 5 years, protection approaches 100%
        let prot5 =
            pool.impermanent_loss_protection([1; 32], 0, blocks_per_year * 5, blocks_per_year);
        assert!(prot5 > 0.95, "Protection after 5 years: {}", prot5);
    }

    #[test]
    fn test_add_remove_liquidity() {
        let mut pool = LiquidityPool::new([0; 32], 1, 2, 1_000_000, 1_000_000, [1; 32], 0);

        let initial_lp = pool.total_lp_tokens;

        // Add equal liquidity
        let result = pool.add_liquidity(500_000, 500_000, [2; 32]);

        assert!(result.lp_tokens_received > 0);
        assert!(result.share_of_pool > 0.0 && result.share_of_pool < 1.0);

        // Remove liquidity
        let (a, b) = pool
            .remove_liquidity(result.lp_tokens_received, [2; 32])
            .unwrap();

        assert!(a > 0 && b > 0);
        assert_eq!(pool.total_lp_tokens, initial_lp);
    }
}
