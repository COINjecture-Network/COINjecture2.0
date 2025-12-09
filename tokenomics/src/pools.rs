// =============================================================================
// Dimensional Token Pools
// Based on exponential scales D_n = e^(-τn/√2)
// =============================================================================
// Each pool serves a specific economic function with mathematically-derived allocations

use crate::dimensions::ETA;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The 8 dimensional token pools
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PoolType {
    /// D1: Genesis/Mining rewards - immediate liquidity
    Genesis,
    /// D2: Network coupling incentives - validator rewards
    NetworkCoupling,
    /// D3: First harmonic - staking rewards
    Staking,
    /// D4: Golden ratio - governance participation
    Governance,
    /// D5: Half-scale - bounty market
    Bounties,
    /// D6: Development fund
    Development,
    /// D7: Ecosystem grants
    Ecosystem,
    /// D8: Long-term reserve
    Reserve,
}

impl PoolType {
    /// Get the dimensional index (1-8)
    pub fn index(&self) -> u8 {
        match self {
            PoolType::Genesis => 1,
            PoolType::NetworkCoupling => 2,
            PoolType::Staking => 3,
            PoolType::Governance => 4,
            PoolType::Bounties => 5,
            PoolType::Development => 6,
            PoolType::Ecosystem => 7,
            PoolType::Reserve => 8,
        }
    }

    /// Get the time parameter τ_n for this pool
    pub fn tau(&self) -> f64 {
        match self {
            PoolType::Genesis => 0.00,
            PoolType::NetworkCoupling => 0.20,
            PoolType::Staking => 0.41,
            PoolType::Governance => 0.68,
            PoolType::Bounties => 0.98,
            PoolType::Development => 1.36,
            PoolType::Ecosystem => 1.96,
            PoolType::Reserve => 2.72,
        }
    }

    /// Calculate dimensional scale: D_n = e^(-τ_n/√2) = e^(-η·τ_n)
    pub fn scale(&self) -> f64 {
        (-ETA * self.tau()).exp()
    }

    /// Get all pool types in order
    pub fn all() -> Vec<PoolType> {
        vec![
            PoolType::Genesis,
            PoolType::NetworkCoupling,
            PoolType::Staking,
            PoolType::Governance,
            PoolType::Bounties,
            PoolType::Development,
            PoolType::Ecosystem,
            PoolType::Reserve,
        ]
    }
}

/// Dimensional pool with allocation and state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DimensionalPool {
    /// Pool type
    pub pool_type: PoolType,
    /// Dimensional scale D_n
    pub scale: f64,
    /// Normalized allocation percentage (p_n = D_n / Σ(D_k))
    pub allocation_pct: f64,
    /// Total tokens allocated to this pool
    pub total_allocation: u128,
    /// Tokens currently in pool (not yet distributed)
    pub balance: u128,
    /// Tokens distributed from this pool
    pub distributed: u128,
    /// Unlock start time (τ_n in blocks)
    pub unlock_start_block: u64,
    /// Current unlock percentage
    pub unlocked_pct: f64,
}

impl DimensionalPool {
    /// Create a new pool with calculated allocation
    pub fn new(pool_type: PoolType, total_supply: u128, sum_of_scales: f64) -> Self {
        let scale = pool_type.scale();
        let allocation_pct = (scale / sum_of_scales) * 100.0;
        let total_allocation = ((total_supply as f64) * (allocation_pct / 100.0)) as u128;
        
        DimensionalPool {
            pool_type,
            scale,
            allocation_pct,
            total_allocation,
            balance: total_allocation,
            distributed: 0,
            unlock_start_block: (pool_type.tau() * 10000.0) as u64, // Scale tau to blocks
            unlocked_pct: 0.0,
        }
    }

    /// Calculate unlock percentage at given block
    /// U_n(τ) = 1 - e^(-η(τ - τ_n)) for τ ≥ τ_n
    pub fn calculate_unlock(&self, current_block: u64) -> f64 {
        if current_block < self.unlock_start_block {
            return 0.0;
        }
        let delta = (current_block - self.unlock_start_block) as f64 / 10000.0;
        1.0 - (-ETA * delta).exp()
    }

    /// Get available (unlocked) balance
    pub fn available_balance(&self, current_block: u64) -> u128 {
        let unlock_pct = self.calculate_unlock(current_block);
        let unlockable = ((self.total_allocation as f64) * unlock_pct) as u128;
        unlockable.saturating_sub(self.distributed)
    }

    /// Distribute tokens from pool (returns actual amount distributed)
    pub fn distribute(&mut self, amount: u128, current_block: u64) -> u128 {
        let available = self.available_balance(current_block);
        let actual = amount.min(available);
        
        if actual > 0 {
            self.balance = self.balance.saturating_sub(actual);
            self.distributed += actual;
            self.unlocked_pct = self.calculate_unlock(current_block);
        }
        
        actual
    }
}

/// Manager for all dimensional pools
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolManager {
    /// Total token supply
    pub total_supply: u128,
    /// Sum of all dimensional scales (for normalization)
    pub sum_of_scales: f64,
    /// All pools
    pub pools: HashMap<PoolType, DimensionalPool>,
    /// Current block height
    pub current_block: u64,
}

impl PoolManager {
    /// Create pool manager with total supply
    pub fn new(total_supply: u128) -> Self {
        // Calculate sum of scales for normalization
        let sum_of_scales: f64 = PoolType::all().iter().map(|p| p.scale()).sum();
        
        let mut pools = HashMap::new();
        for pool_type in PoolType::all() {
            let pool = DimensionalPool::new(pool_type, total_supply, sum_of_scales);
            pools.insert(pool_type, pool);
        }
        
        PoolManager {
            total_supply,
            sum_of_scales,
            pools,
            current_block: 0,
        }
    }

    /// Update current block
    pub fn set_block(&mut self, block: u64) {
        self.current_block = block;
    }

    /// Get pool by type
    pub fn get_pool(&self, pool_type: PoolType) -> Option<&DimensionalPool> {
        self.pools.get(&pool_type)
    }

    /// Get mutable pool by type
    pub fn get_pool_mut(&mut self, pool_type: PoolType) -> Option<&mut DimensionalPool> {
        self.pools.get_mut(&pool_type)
    }

    /// Distribute from specific pool
    pub fn distribute_from(&mut self, pool_type: PoolType, amount: u128) -> u128 {
        if let Some(pool) = self.pools.get_mut(&pool_type) {
            pool.distribute(amount, self.current_block)
        } else {
            0
        }
    }

    /// Get total available across all pools
    pub fn total_available(&self) -> u128 {
        self.pools.values()
            .map(|p| p.available_balance(self.current_block))
            .sum()
    }

    /// Get pool status summary
    pub fn status(&self) -> Vec<PoolStatus> {
        self.pools.values()
            .map(|p| PoolStatus {
                pool_type: p.pool_type,
                scale: p.scale,
                allocation_pct: p.allocation_pct,
                total: p.total_allocation,
                balance: p.balance,
                distributed: p.distributed,
                available: p.available_balance(self.current_block),
                unlocked_pct: p.calculate_unlock(self.current_block) * 100.0,
            })
            .collect()
    }
}

/// Pool status for reporting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolStatus {
    pub pool_type: PoolType,
    pub scale: f64,
    pub allocation_pct: f64,
    pub total: u128,
    pub balance: u128,
    pub distributed: u128,
    pub available: u128,
    pub unlocked_pct: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_scales() {
        // D1 should be 1.0 (e^0)
        assert!((PoolType::Genesis.scale() - 1.0).abs() < 0.01);
        
        // D4 should be ~0.618 (golden ratio)
        let d4 = PoolType::Governance.scale();
        assert!(d4 > 0.6 && d4 < 0.65, "D4 scale: {}", d4);
        
        // D5 should be ~0.5
        let d5 = PoolType::Bounties.scale();
        assert!(d5 > 0.48 && d5 < 0.52, "D5 scale: {}", d5);
    }

    #[test]
    fn test_pool_allocations_sum_to_100() {
        let total_supply = 1_000_000_000_000u128; // 1 trillion
        let manager = PoolManager::new(total_supply);
        
        let total_allocated: u128 = manager.pools.values()
            .map(|p| p.total_allocation)
            .sum();
        
        // Should be very close to total supply (within rounding)
        assert!(total_allocated <= total_supply);
        assert!(total_allocated > total_supply - 1000);
    }

    #[test]
    fn test_unlock_curve() {
        let total_supply = 1_000_000_000_000u128;
        let mut manager = PoolManager::new(total_supply);
        
        // At block 0, D1 (Genesis) should have some available (τ=0)
        manager.set_block(0);
        let genesis_pool = manager.get_pool(PoolType::Genesis).unwrap();
        // At exactly τ_n, unlock is 0, but immediately after it starts
        
        // At block 10000, all pools should have some unlock
        manager.set_block(10000);
        for pool in manager.pools.values() {
            let unlock = pool.calculate_unlock(10000);
            if pool.unlock_start_block <= 10000 {
                assert!(unlock > 0.0, "Pool {:?} should have unlocked", pool.pool_type);
            }
        }
    }
}

