// =============================================================================
// Bounty Market Pricing
// bounty_price = base_reward × (solve_time/verify_time) × √(solve_memory/verify_memory) × weight
// =============================================================================
// Dynamic pricing based on computational asymmetry

use coinject_core::{ETA, PHI}; // Import from core (single source of truth)
use serde::{Deserialize, Serialize};

/// Euler's number for STATISTICAL strategy premium
pub const E: f64 = 2.718281828459045;

/// Aggregation strategy for bounty solutions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AggregationStrategy {
    /// First valid solution wins (1.0x)
    Any,
    /// Best quality solution wins (φ = 1.618x)
    Best,
    /// Multiple diverse solutions accepted (2.0x)
    Multiple,
    /// Statistical sampling collection (e ≈ 2.718x)
    Statistical,
}

impl AggregationStrategy {
    /// Get price multiplier for this strategy
    pub fn multiplier(&self) -> f64 {
        match self {
            AggregationStrategy::Any => 1.0,
            AggregationStrategy::Best => PHI,        // Golden ratio
            AggregationStrategy::Multiple => 2.0,    // Power of two
            AggregationStrategy::Statistical => E,   // Euler's number
        }
    }

    /// Get description
    pub fn description(&self) -> &'static str {
        match self {
            AggregationStrategy::Any => "First valid solution wins",
            AggregationStrategy::Best => "Highest quality solution wins (competition)",
            AggregationStrategy::Multiple => "Multiple diverse solutions accepted",
            AggregationStrategy::Statistical => "Statistical sampling for ML/research",
        }
    }
}

/// Problem complexity metrics for pricing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProblemMetrics {
    /// Expected solve time (microseconds)
    pub solve_time_us: u64,
    /// Expected verify time (microseconds)
    pub verify_time_us: u64,
    /// Expected solve memory (bytes)
    pub solve_memory_bytes: u64,
    /// Expected verify memory (bytes)
    pub verify_memory_bytes: u64,
    /// Problem size/complexity weight (1.0 = average)
    pub complexity_weight: f64,
}

impl ProblemMetrics {
    /// Calculate time asymmetry ratio
    pub fn time_asymmetry(&self) -> f64 {
        if self.verify_time_us == 0 {
            return 1.0;
        }
        (self.solve_time_us as f64) / (self.verify_time_us as f64)
    }

    /// Calculate space asymmetry ratio (square root for geometric mean)
    pub fn space_asymmetry(&self) -> f64 {
        if self.verify_memory_bytes == 0 {
            return 1.0;
        }
        ((self.solve_memory_bytes as f64) / (self.verify_memory_bytes as f64)).sqrt()
    }

    /// Calculate combined asymmetry score
    pub fn asymmetry_score(&self) -> f64 {
        self.time_asymmetry() * self.space_asymmetry()
    }
}

/// Bounty pricing calculator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BountyPricer {
    /// Base reward for average complexity problem
    pub base_reward: u128,
    /// Minimum bounty floor
    pub min_bounty: u128,
    /// Network fee percentage (for marketplace operation)
    pub network_fee_pct: f64,
}

impl BountyPricer {
    /// Create new pricer
    pub fn new(base_reward: u128) -> Self {
        BountyPricer {
            base_reward,
            min_bounty: base_reward / 10,
            network_fee_pct: 0.02, // 2% fee
        }
    }

    /// Calculate suggested bounty price
    /// bounty = base × asymmetry × complexity × strategy_multiplier
    pub fn calculate_price(
        &self,
        metrics: &ProblemMetrics,
        strategy: AggregationStrategy,
    ) -> BountyPrice {
        let asymmetry = metrics.asymmetry_score();
        let strategy_mult = strategy.multiplier();
        
        // Raw price calculation
        let raw_price = (self.base_reward as f64) 
            * asymmetry 
            * metrics.complexity_weight 
            * strategy_mult;
        
        let suggested = (raw_price as u128).max(self.min_bounty);
        let network_fee = ((suggested as f64) * self.network_fee_pct) as u128;
        
        BountyPrice {
            suggested_bounty: suggested,
            network_fee,
            total_required: suggested + network_fee,
            time_asymmetry: metrics.time_asymmetry(),
            space_asymmetry: metrics.space_asymmetry(),
            strategy_multiplier: strategy_mult,
            breakdown: PriceBreakdown {
                base: self.base_reward,
                asymmetry_factor: asymmetry,
                complexity_factor: metrics.complexity_weight,
                strategy_factor: strategy_mult,
            },
        }
    }

    /// Calculate solver reward distribution for MULTIPLE strategy
    pub fn distribute_multiple_rewards(
        &self,
        total_bounty: u128,
        num_solutions: u32,
        quality_scores: &[f64],
    ) -> Vec<u128> {
        if quality_scores.is_empty() || num_solutions == 0 {
            return vec![];
        }

        let total_quality: f64 = quality_scores.iter().sum();
        if total_quality == 0.0 {
            // Equal distribution if no quality differentiation
            let share = total_bounty / (num_solutions as u128);
            return vec![share; num_solutions as usize];
        }

        // Quality-weighted distribution
        quality_scores.iter()
            .map(|q| ((total_bounty as f64) * (q / total_quality)) as u128)
            .collect()
    }

    /// Calculate solver reward with quality bonus for BEST strategy
    pub fn calculate_best_reward(
        &self,
        base_bounty: u128,
        quality_score: f64,
        max_quality: f64,
    ) -> u128 {
        if max_quality == 0.0 {
            return base_bounty;
        }
        
        // Winner gets base bounty plus quality bonus
        // Bonus scales with how much better the solution is
        let quality_ratio = quality_score / max_quality;
        let bonus = ((base_bounty as f64) * (quality_ratio - 1.0).max(0.0) * ETA) as u128;
        
        base_bounty + bonus
    }
}

/// Calculated bounty price
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BountyPrice {
    /// Suggested bounty amount
    pub suggested_bounty: u128,
    /// Network fee
    pub network_fee: u128,
    /// Total required (bounty + fee)
    pub total_required: u128,
    /// Time asymmetry factor used
    pub time_asymmetry: f64,
    /// Space asymmetry factor used
    pub space_asymmetry: f64,
    /// Strategy multiplier applied
    pub strategy_multiplier: f64,
    /// Detailed breakdown
    pub breakdown: PriceBreakdown,
}

/// Price calculation breakdown
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceBreakdown {
    pub base: u128,
    pub asymmetry_factor: f64,
    pub complexity_factor: f64,
    pub strategy_factor: f64,
}

/// Bounty market statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BountyMarketStats {
    pub total_bounties_posted: u64,
    pub total_bounties_solved: u64,
    pub total_value_escrowed: u128,
    pub total_value_paid: u128,
    pub average_solve_time_blocks: f64,
    pub average_asymmetry_score: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strategy_multipliers() {
        assert_eq!(AggregationStrategy::Any.multiplier(), 1.0);
        assert!((AggregationStrategy::Best.multiplier() - PHI).abs() < 0.001);
        assert_eq!(AggregationStrategy::Multiple.multiplier(), 2.0);
        assert!((AggregationStrategy::Statistical.multiplier() - E).abs() < 0.001);
    }

    #[test]
    fn test_asymmetry_calculation() {
        let metrics = ProblemMetrics {
            solve_time_us: 1_000_000,   // 1 second
            verify_time_us: 1_000,       // 1 millisecond
            solve_memory_bytes: 1_000_000_000, // 1 GB
            verify_memory_bytes: 1_000_000,     // 1 MB
            complexity_weight: 1.0,
        };
        
        let time_asym = metrics.time_asymmetry();
        assert_eq!(time_asym, 1000.0);
        
        let space_asym = metrics.space_asymmetry();
        assert!((space_asym - 31.62).abs() < 0.1); // √1000
        
        let total_asym = metrics.asymmetry_score();
        assert!(total_asym > 30000.0);
    }

    #[test]
    fn test_bounty_pricing() {
        let pricer = BountyPricer::new(1_000_000);
        
        let simple_metrics = ProblemMetrics {
            solve_time_us: 10_000,
            verify_time_us: 1_000,
            solve_memory_bytes: 10_000_000,
            verify_memory_bytes: 1_000_000,
            complexity_weight: 1.0,
        };
        
        let any_price = pricer.calculate_price(&simple_metrics, AggregationStrategy::Any);
        let best_price = pricer.calculate_price(&simple_metrics, AggregationStrategy::Best);
        
        // BEST should be ~1.618x more expensive
        let ratio = (best_price.suggested_bounty as f64) / (any_price.suggested_bounty as f64);
        assert!((ratio - PHI).abs() < 0.1);
    }

    #[test]
    fn test_multiple_distribution() {
        let pricer = BountyPricer::new(1_000_000);
        
        let quality_scores = vec![1.0, 0.8, 0.6];
        let rewards = pricer.distribute_multiple_rewards(1_000_000, 3, &quality_scores);
        
        assert_eq!(rewards.len(), 3);
        assert!(rewards[0] > rewards[1]);
        assert!(rewards[1] > rewards[2]);
        
        let total: u128 = rewards.iter().sum();
        assert!(total <= 1_000_000);
    }
}

