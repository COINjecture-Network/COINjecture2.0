// =============================================================================
// Deflationary Mechanism
// burn_rate(t) = λ · cumulative_work(t) / total_supply(t)
// =============================================================================
// Creates natural deflation tied to network utility

use coinject_core::ETA; // Import from core (single source of truth)
use serde::{Deserialize, Serialize};

/// Lambda constant (λ = 1/√2 = η)
pub const LAMBDA: f64 = ETA;

/// Burn mechanism tied to cumulative work
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeflationEngine {
    /// Cumulative work done by network
    pub cumulative_work: f64,
    /// Total tokens burned
    pub total_burned: u128,
    /// Current circulating supply
    pub circulating_supply: u128,
    /// Initial total supply (for reference)
    pub initial_supply: u128,
    /// Burn rate history (for smoothing)
    burn_rate_history: Vec<f64>,
}

impl DeflationEngine {
    /// Create new deflation engine
    pub fn new(initial_supply: u128) -> Self {
        DeflationEngine {
            cumulative_work: 0.0,
            total_burned: 0,
            circulating_supply: initial_supply,
            initial_supply,
            burn_rate_history: Vec::new(),
        }
    }

    /// Calculate current burn rate
    /// burn_rate = λ · cumulative_work / circulating_supply
    pub fn current_burn_rate(&self) -> f64 {
        if self.circulating_supply == 0 {
            return 0.0;
        }
        LAMBDA * self.cumulative_work / (self.circulating_supply as f64)
    }

    /// Calculate smoothed burn rate (average of recent rates)
    pub fn smoothed_burn_rate(&self) -> f64 {
        if self.burn_rate_history.is_empty() {
            return self.current_burn_rate();
        }
        
        // Use last 100 rates for smoothing
        let recent: Vec<_> = self.burn_rate_history.iter()
            .rev()
            .take(100)
            .collect();
        
        recent.iter().copied().sum::<f64>() / (recent.len() as f64)
    }

    /// Record work and calculate burn amount
    pub fn record_work(&mut self, work_score: f64) -> BurnResult {
        self.cumulative_work += work_score;
        
        let burn_rate = self.current_burn_rate();
        self.burn_rate_history.push(burn_rate);
        
        // Keep history bounded
        if self.burn_rate_history.len() > 1000 {
            self.burn_rate_history.remove(0);
        }
        
        // Calculate burn amount for this work
        // burn = work_score × burn_rate × scaling_factor
        let scaling = 1e-6; // Prevent excessive burning
        let burn_amount = ((work_score * burn_rate * scaling) as u128)
            .min(self.circulating_supply / 1000); // Never burn more than 0.1% at once
        
        BurnResult {
            burn_rate,
            burn_amount,
            work_recorded: work_score,
        }
    }

    /// Execute a burn
    pub fn execute_burn(&mut self, amount: u128) -> u128 {
        let actual_burn = amount.min(self.circulating_supply);
        self.total_burned += actual_burn;
        self.circulating_supply = self.circulating_supply.saturating_sub(actual_burn);
        actual_burn
    }

    /// Get deflation statistics
    pub fn stats(&self) -> DeflationStats {
        let deflation_pct = if self.initial_supply > 0 {
            ((self.total_burned as f64) / (self.initial_supply as f64)) * 100.0
        } else {
            0.0
        };

        DeflationStats {
            cumulative_work: self.cumulative_work,
            total_burned: self.total_burned,
            circulating_supply: self.circulating_supply,
            initial_supply: self.initial_supply,
            current_burn_rate: self.current_burn_rate(),
            smoothed_burn_rate: self.smoothed_burn_rate(),
            deflation_pct,
        }
    }
}

/// Result of work recording
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BurnResult {
    pub burn_rate: f64,
    pub burn_amount: u128,
    pub work_recorded: f64,
}

/// Deflation statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeflationStats {
    pub cumulative_work: f64,
    pub total_burned: u128,
    pub circulating_supply: u128,
    pub initial_supply: u128,
    pub current_burn_rate: f64,
    pub smoothed_burn_rate: f64,
    pub deflation_pct: f64,
}

/// Fee structure (dimensionless)
/// fee = (tx_complexity / avg_block_complexity) × base_fee
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeCalculator {
    /// Base fee in tokens
    pub base_fee: u128,
    /// Average block complexity (rolling average)
    pub avg_block_complexity: f64,
    /// Fee burn percentage (0.0 - 1.0)
    pub burn_percentage: f64,
}

impl FeeCalculator {
    /// Create new fee calculator
    pub fn new(base_fee: u128) -> Self {
        FeeCalculator {
            base_fee,
            avg_block_complexity: 1.0,
            burn_percentage: 0.5, // 50% of fees burned
        }
    }

    /// Calculate transaction fee (dimensionless)
    pub fn calculate_fee(&self, tx_complexity: f64) -> FeeResult {
        let complexity_ratio = tx_complexity / self.avg_block_complexity;
        let total_fee = ((self.base_fee as f64) * complexity_ratio) as u128;
        let burn_amount = ((total_fee as f64) * self.burn_percentage) as u128;
        let validator_reward = total_fee - burn_amount;

        FeeResult {
            total_fee,
            burn_amount,
            validator_reward,
            complexity_ratio,
        }
    }

    /// Update average complexity (called per block)
    pub fn update_avg_complexity(&mut self, block_complexity: f64) {
        // Exponential moving average with decay
        self.avg_block_complexity = self.avg_block_complexity * (1.0 - ETA) + block_complexity * ETA;
    }
}

/// Fee calculation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeResult {
    pub total_fee: u128,
    pub burn_amount: u128,
    pub validator_reward: u128,
    pub complexity_ratio: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_burn_rate_increases_with_work() {
        let mut engine = DeflationEngine::new(1_000_000_000_000);
        
        let rate1 = engine.current_burn_rate();
        engine.record_work(1000.0);
        let rate2 = engine.current_burn_rate();
        
        assert!(rate2 > rate1);
    }

    #[test]
    fn test_burn_amount_bounded() {
        let mut engine = DeflationEngine::new(1_000_000);
        engine.cumulative_work = 1e12; // Very high work
        
        let result = engine.record_work(1e6);
        
        // Burn should never exceed 0.1% of supply
        assert!(result.burn_amount <= engine.circulating_supply / 1000);
    }

    #[test]
    fn test_fee_calculation() {
        let calculator = FeeCalculator::new(1000);
        
        // Average complexity transaction
        let fee1 = calculator.calculate_fee(1.0);
        assert_eq!(fee1.total_fee, 1000);
        
        // Double complexity transaction
        let fee2 = calculator.calculate_fee(2.0);
        assert_eq!(fee2.total_fee, 2000);
        
        // Half complexity transaction
        let fee3 = calculator.calculate_fee(0.5);
        assert_eq!(fee3.total_fee, 500);
    }

    #[test]
    fn test_fee_burn_split() {
        let calculator = FeeCalculator::new(1000);
        let fee = calculator.calculate_fee(1.0);
        
        // 50% should be burned
        assert_eq!(fee.burn_amount, 500);
        assert_eq!(fee.validator_reward, 500);
        assert_eq!(fee.total_fee, fee.burn_amount + fee.validator_reward);
    }
}

