// Dimensional Reward Distribution
// Splits block rewards across 8 dimensions with time-locked vesting

use crate::dimensions::Dimension;
use coinject_core::{Address, Balance};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Allocation across 8 dimensions with time locks
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DimensionalAllocation {
    pub immediate: Balance,           // D1: Immediate liquidity (56.1%)
    pub short_term: Balance,          // D2: 7-day lock (48.6%)
    pub bounty_pool: Balance,         // D3: 14-day lock (42.1%)
    pub treasury: Balance,            // D4: 24-day lock (34.7%)
    pub development: Balance,         // D5: 35-day lock (28.1%)
    pub long_term: Balance,           // D6: 48-day lock (21.4%)
    pub strategic: Balance,           // D7: 69-day lock (14.0%)
    pub foundation: Balance,          // D8: 96-day lock (8.2%)
}

impl DimensionalAllocation {
    /// Total across all dimensions
    pub fn total(&self) -> Balance {
        self.immediate
            + self.short_term
            + self.bounty_pool
            + self.treasury
            + self.development
            + self.long_term
            + self.strategic
            + self.foundation
    }

    /// Get dimension by index (1-8)
    pub fn get_dimension(&self, index: u8) -> Balance {
        match index {
            1 => self.immediate,
            2 => self.short_term,
            3 => self.bounty_pool,
            4 => self.treasury,
            5 => self.development,
            6 => self.long_term,
            7 => self.strategic,
            8 => self.foundation,
            _ => 0,
        }
    }
}

/// Distributes block rewards across 8 dimensions
pub struct DimensionalDistributor {
    dimensions: Vec<Dimension>,
}

impl DimensionalDistributor {
    pub fn new() -> Self {
        DimensionalDistributor {
            dimensions: Dimension::all(),
        }
    }

    /// Distribute total reward across dimensions using exponential allocation
    /// Each dimension D_n receives: allocation_n * total_reward
    pub fn distribute(&self, total_reward: Balance) -> DimensionalAllocation {
        // Calculate normalized sum of all allocations
        let total_allocation: f64 = self.dimensions.iter().map(|d| d.allocation).sum();

        // Distribute proportionally
        let allocations: Vec<Balance> = self
            .dimensions
            .iter()
            .map(|d| {
                let percentage = d.allocation / total_allocation;
                (total_reward as f64 * percentage) as Balance
            })
            .collect();

        DimensionalAllocation {
            immediate: allocations[0],
            short_term: allocations[1],
            bounty_pool: allocations[2],
            treasury: allocations[3],
            development: allocations[4],
            long_term: allocations[5],
            strategic: allocations[6],
            foundation: allocations[7],
        }
    }

    /// Calculate total unlocked amount after given days
    pub fn unlocked_after_days(&self, allocation: &DimensionalAllocation, days: u64) -> Balance {
        let mut total = 0;

        for (i, dim) in self.dimensions.iter().enumerate() {
            let dim_allocation = allocation.get_dimension((i + 1) as u8);
            let unlock_fraction = dim.unlock_at_time(days);
            total += (dim_allocation as f64 * unlock_fraction) as Balance;
        }

        total
    }

    /// Get allocation percentages for display
    pub fn get_allocation_percentages(&self) -> HashMap<String, f64> {
        let total_allocation: f64 = self.dimensions.iter().map(|d| d.allocation).sum();

        self.dimensions
            .iter()
            .map(|d| {
                let percentage = (d.allocation / total_allocation) * 100.0;
                (d.function.clone(), percentage)
            })
            .collect()
    }
}

impl Default for DimensionalDistributor {
    fn default() -> Self {
        Self::new()
    }
}

/// Vesting schedule entry for a specific dimension
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VestingEntry {
    pub recipient: Address,
    pub dimension_index: u8,
    pub total_amount: Balance,
    pub vested_at_height: u64,
    pub vested_at_timestamp: i64,
}

impl VestingEntry {
    /// Calculate unlocked amount based on elapsed time
    pub fn unlocked_amount(&self, current_timestamp: i64) -> Balance {
        let dimensions = Dimension::all();
        let dim = &dimensions[(self.dimension_index - 1) as usize];

        let elapsed_seconds = (current_timestamp - self.vested_at_timestamp).max(0);
        let elapsed_days = elapsed_seconds / 86400;

        let unlock_fraction = dim.unlock_at_time(elapsed_days as u64);
        (self.total_amount as f64 * unlock_fraction) as Balance
    }

    /// Check if fully unlocked
    pub fn is_fully_unlocked(&self, current_timestamp: i64) -> bool {
        let unlocked = self.unlocked_amount(current_timestamp);
        unlocked >= self.total_amount
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dimensional_distribution() {
        let distributor = DimensionalDistributor::new();
        let total_reward = 1000;

        let allocation = distributor.distribute(total_reward);

        // Verify total is conserved (within rounding error)
        let distributed_total = allocation.total();
        assert!(
            (distributed_total as i64 - total_reward as i64).abs() <= 8,
            "Total should be conserved"
        );

        // D1 (immediate) should have highest allocation
        assert!(
            allocation.immediate > allocation.foundation,
            "D1 should be greater than D8"
        );

        // Verify exponential decay pattern
        assert!(allocation.short_term < allocation.immediate);
        assert!(allocation.bounty_pool < allocation.short_term);
    }

    #[test]
    fn test_unlock_over_time() {
        let distributor = DimensionalDistributor::new();
        let allocation = distributor.distribute(1000);

        // Immediately, only D1 is unlocked (0 lock period)
        let unlocked_0 = distributor.unlocked_after_days(&allocation, 0);
        assert_eq!(unlocked_0, 0);

        // After 7 days, D2 starts unlocking
        let unlocked_7 = distributor.unlocked_after_days(&allocation, 7);
        assert!(unlocked_7 > 0);

        // After 100 days, most should be unlocked
        let unlocked_100 = distributor.unlocked_after_days(&allocation, 100);
        assert!(unlocked_100 > unlocked_7);
    }

    #[test]
    fn test_vesting_entry() {
        let entry = VestingEntry {
            recipient: Address::from_bytes([1u8; 32]),
            dimension_index: 2, // 7-day lock
            total_amount: 1000,
            vested_at_height: 100,
            vested_at_timestamp: 0,
        };

        // Immediately: locked
        assert_eq!(entry.unlocked_amount(0), 0);

        // After 7 days + 1 day = 8 days
        let eight_days_later = 8 * 86400;
        let unlocked = entry.unlocked_amount(eight_days_later);
        assert!(unlocked > 0 && unlocked < 1000);

        // After very long time: fully unlocked
        let long_time = 365 * 86400;
        let unlocked_long = entry.unlocked_amount(long_time);
        assert!(unlocked_long >= 1000);
    }

    #[test]
    fn test_allocation_percentages() {
        let distributor = DimensionalDistributor::new();
        let percentages = distributor.get_allocation_percentages();

        assert_eq!(percentages.len(), 8);

        // Verify total percentages sum to ~100%
        let total: f64 = percentages.values().sum();
        assert!((total - 100.0).abs() < 0.1);
    }
}
