// =============================================================================
// Multi-Dimensional Staking with Viviani Oracle
// multiplier(η_user, λ_user) = 1 + Δ(η_user, λ_user)
// =============================================================================
// Users who stake across multiple dimensions receive bonuses based on
// how close their portfolio is to the critical equilibrium η = λ = 1/√2

use crate::dimensions::ETA;
use crate::pools::PoolType;
use coinject_core::Address;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The Viviani oracle constant Δ_critical ≈ 0.231
pub const DELTA_CRITICAL: f64 = 0.231;

/// Maximum Viviani bonus (23.1%)
pub const MAX_VIVIANI_BONUS: f64 = DELTA_CRITICAL;

/// Dimensional yield rates: yield_n(τ) = η · D_n
/// These are base APY rates before any bonuses
pub fn get_base_yield(pool_type: PoolType) -> f64 {
    ETA * pool_type.scale() // η · D_n
}

/// Staker's position in a dimensional pool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StakePosition {
    /// Pool type
    pub pool_type: PoolType,
    /// Staked amount
    pub amount: u128,
    /// Stake start block
    pub start_block: u64,
    /// Last reward claim block
    pub last_claim_block: u64,
    /// Accumulated unclaimed rewards
    pub pending_rewards: u128,
}

impl StakePosition {
    /// Calculate base rewards (before multiplier)
    pub fn calculate_base_rewards(&self, current_block: u64, blocks_per_year: u64) -> u128 {
        let blocks_elapsed = current_block.saturating_sub(self.last_claim_block);
        let base_yield = get_base_yield(self.pool_type);
        
        // Annual yield scaled to blocks elapsed
        let yield_fraction = (blocks_elapsed as f64) / (blocks_per_year as f64);
        ((self.amount as f64) * base_yield * yield_fraction) as u128
    }
}

/// Multi-dimensional staking portfolio
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StakingPortfolio {
    /// Owner address
    pub owner: Address,
    /// Positions by pool type
    pub positions: HashMap<PoolType, StakePosition>,
    /// Current Viviani metric
    pub viviani_delta: f64,
    /// Current multiplier (1.0 + bonus)
    pub multiplier: f64,
}

impl StakingPortfolio {
    /// Create new portfolio
    pub fn new(owner: Address) -> Self {
        StakingPortfolio {
            owner,
            positions: HashMap::new(),
            viviani_delta: 0.0,
            multiplier: 1.0,
        }
    }

    /// Calculate η and λ for the portfolio
    /// η_user = normalized weighted average of dimensional scales
    /// λ_user = diversification factor (inverse HHI)
    fn calculate_eta_lambda(&self) -> (f64, f64) {
        if self.positions.is_empty() {
            return (0.0, 0.0);
        }

        let total_staked: u128 = self.positions.values().map(|p| p.amount).sum();
        if total_staked == 0 {
            return (0.0, 0.0);
        }

        // Calculate weighted average of dimensional scales
        let weighted_scale: f64 = self.positions.iter()
            .map(|(pool_type, pos)| {
                let weight = (pos.amount as f64) / (total_staked as f64);
                pool_type.scale() * weight
            })
            .sum();
        
        // Normalize η to [0, 1] range - max scale is 1.0 (D1)
        // At optimal diversification across all pools, η should approach ETA
        let eta_user = weighted_scale;

        // Calculate diversification (λ_user) - based on Herfindahl-Hirschman index
        // HHI = Σ(share_i²) ranges from 1/n (equal) to 1 (concentrated)
        // With 8 pools equally weighted, HHI = 8 * (1/8)² = 0.125
        let hhi: f64 = self.positions.values()
            .map(|pos| {
                let share = (pos.amount as f64) / (total_staked as f64);
                share * share
            })
            .sum();
        
        // Normalize λ: when HHI is low (diversified), λ is high
        // At optimal: HHI = 0.125 (8 pools), λ should be near ETA
        let lambda_user = 1.0 - hhi;

        (eta_user, lambda_user)
    }

    /// Calculate Viviani delta measuring distance from optimal diversification
    /// Lower is better (more optimally diversified)
    pub fn calculate_viviani_delta(&self) -> f64 {
        let (eta, lambda) = self.calculate_eta_lambda();
        
        if eta == 0.0 || lambda == 0.0 {
            return 1.0; // Maximum distance if no stake
        }
        
        // For weighted scale η: optimal depends on stake distribution
        // With equal stakes across 8 pools: η = avg(D_1..D_8) ≈ 0.564
        // We want to reward getting close to the exponential distribution
        
        // For diversification λ: optimal is high (low HHI)
        // With 8 equal pools: λ = 1 - 0.125 = 0.875
        
        // Combined metric: reward both scale distribution and diversification
        // Δ = (1 - λ) + |η - target_eta| where target_eta ≈ 0.564
        let target_eta = 0.564; // Average of D1-D8 scales
        let scale_deviation = (eta - target_eta).abs();
        let concentration = 1.0 - lambda; // How concentrated (lower = better)
        
        scale_deviation + concentration
    }

    /// Calculate staking multiplier based on Viviani oracle
    /// Bonus increases with better diversification (lower delta)
    pub fn calculate_multiplier(&self) -> f64 {
        let (eta, lambda) = self.calculate_eta_lambda();
        
        // No positions = no bonus
        if self.positions.is_empty() {
            return 1.0;
        }
        
        // Multiplier based on diversification level
        // λ ranges from 0 (single pool) to ~0.875 (8 equal pools)
        // Bonus = λ * MAX_BONUS = up to 0.875 * 0.231 ≈ 20%
        
        // Also factor in number of pools (more pools = more bonus)
        let pool_factor = (self.positions.len() as f64).min(8.0) / 8.0;
        
        // Combined bonus: diversification + pool coverage
        let bonus = (lambda * pool_factor * MAX_VIVIANI_BONUS).min(MAX_VIVIANI_BONUS);
        
        1.0 + bonus
    }

    /// Update portfolio metrics
    pub fn update_metrics(&mut self) {
        self.viviani_delta = self.calculate_viviani_delta();
        self.multiplier = self.calculate_multiplier();
    }

    /// Stake tokens in a pool
    pub fn stake(&mut self, pool_type: PoolType, amount: u128, current_block: u64) {
        if let Some(position) = self.positions.get_mut(&pool_type) {
            position.amount += amount;
        } else {
            self.positions.insert(pool_type, StakePosition {
                pool_type,
                amount,
                start_block: current_block,
                last_claim_block: current_block,
                pending_rewards: 0,
            });
        }
        self.update_metrics();
    }

    /// Unstake tokens from a pool
    pub fn unstake(&mut self, pool_type: PoolType, amount: u128) -> Option<u128> {
        if let Some(position) = self.positions.get_mut(&pool_type) {
            let actual = amount.min(position.amount);
            position.amount -= actual;
            
            if position.amount == 0 {
                self.positions.remove(&pool_type);
            }
            
            self.update_metrics();
            Some(actual)
        } else {
            None
        }
    }

    /// Calculate total rewards with multiplier
    pub fn calculate_total_rewards(&self, current_block: u64, blocks_per_year: u64) -> u128 {
        let base_rewards: u128 = self.positions.values()
            .map(|pos| pos.calculate_base_rewards(current_block, blocks_per_year))
            .sum();
        
        ((base_rewards as f64) * self.multiplier) as u128
    }

    /// Claim all pending rewards
    pub fn claim_rewards(&mut self, current_block: u64, blocks_per_year: u64) -> u128 {
        let total = self.calculate_total_rewards(current_block, blocks_per_year);
        
        // Update all positions' last claim block
        for position in self.positions.values_mut() {
            position.last_claim_block = current_block;
            position.pending_rewards = 0;
        }
        
        total
    }

    /// Get portfolio summary
    pub fn summary(&self) -> PortfolioSummary {
        let total_staked: u128 = self.positions.values().map(|p| p.amount).sum();
        let (eta, lambda) = self.calculate_eta_lambda();
        
        PortfolioSummary {
            owner: self.owner,
            total_staked,
            num_pools: self.positions.len(),
            eta_user: eta,
            lambda_user: lambda,
            viviani_delta: self.viviani_delta,
            multiplier: self.multiplier,
            bonus_pct: (self.multiplier - 1.0) * 100.0,
        }
    }
}

/// Portfolio summary for reporting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioSummary {
    pub owner: Address,
    pub total_staked: u128,
    pub num_pools: usize,
    pub eta_user: f64,
    pub lambda_user: f64,
    pub viviani_delta: f64,
    pub multiplier: f64,
    pub bonus_pct: f64,
}

/// Staking manager for all portfolios
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StakingManager {
    pub portfolios: HashMap<Address, StakingPortfolio>,
    pub total_staked: u128,
    pub blocks_per_year: u64,
}

impl StakingManager {
    pub fn new(blocks_per_year: u64) -> Self {
        StakingManager {
            portfolios: HashMap::new(),
            total_staked: 0,
            blocks_per_year,
        }
    }

    pub fn get_or_create_portfolio(&mut self, owner: Address) -> &mut StakingPortfolio {
        self.portfolios.entry(owner)
            .or_insert_with(|| StakingPortfolio::new(owner))
    }

    pub fn stake(&mut self, owner: Address, pool_type: PoolType, amount: u128, current_block: u64) {
        let portfolio = self.get_or_create_portfolio(owner);
        portfolio.stake(pool_type, amount, current_block);
        self.total_staked += amount;
    }

    pub fn unstake(&mut self, owner: Address, pool_type: PoolType, amount: u128) -> Option<u128> {
        if let Some(portfolio) = self.portfolios.get_mut(&owner) {
            if let Some(actual) = portfolio.unstake(pool_type, amount) {
                self.total_staked = self.total_staked.saturating_sub(actual);
                return Some(actual);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_yields() {
        // D1 yield should be highest (η · 1.0 = 0.707)
        let d1_yield = get_base_yield(PoolType::Genesis);
        assert!((d1_yield - ETA).abs() < 0.01);
        
        // D4 yield should be ~43.7% (η · 0.618)
        let d4_yield = get_base_yield(PoolType::Governance);
        assert!(d4_yield > 0.4 && d4_yield < 0.5);
    }

    #[test]
    fn test_viviani_multiplier() {
        let owner = Address::from_bytes([0u8; 32]);
        let mut portfolio = StakingPortfolio::new(owner);
        
        // Single pool stake - poor diversification, minimal bonus
        portfolio.stake(PoolType::Genesis, 1000, 0);
        let single_multiplier = portfolio.multiplier;
        println!("Single pool multiplier: {:.4}", single_multiplier);
        
        // Diversified stake across multiple pools
        portfolio.stake(PoolType::NetworkCoupling, 1000, 0);
        portfolio.stake(PoolType::Staking, 1000, 0);
        portfolio.stake(PoolType::Governance, 1000, 0);
        portfolio.stake(PoolType::Bounties, 1000, 0);
        
        // Should have higher multiplier with diversification
        println!("5-pool multiplier: {:.4}", portfolio.multiplier);
        assert!(portfolio.multiplier > single_multiplier, 
            "Diversified should beat single: {} > {}", portfolio.multiplier, single_multiplier);
    }

    #[test]
    fn test_optimal_diversification() {
        let owner = Address::from_bytes([0u8; 32]);
        let mut portfolio = StakingPortfolio::new(owner);
        
        // Stake equally across all pools for maximum diversification
        for pool_type in PoolType::all() {
            portfolio.stake(pool_type, 1000, 0);
        }
        
        // With 8 equal pools, should get close to max bonus
        println!("8-pool equal stake multiplier: {:.4} (bonus: {:.2}%)", 
            portfolio.multiplier, (portfolio.multiplier - 1.0) * 100.0);
        
        // Should have notable bonus (at least 10%)
        assert!(portfolio.multiplier > 1.10, 
            "Expected > 1.10, got {}", portfolio.multiplier);
        // Should not exceed theoretical max (1 + 0.231 = 1.231)
        assert!(portfolio.multiplier <= 1.0 + MAX_VIVIANI_BONUS + 0.01);
    }
}

