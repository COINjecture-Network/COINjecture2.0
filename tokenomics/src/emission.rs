// =============================================================================
// Dynamic Emission System
// emission_rate(t) = η · |ψ(t)| = (1/√2) · consensus_magnitude(t)
// =============================================================================
// Ties token emission directly to network consensus strength

use crate::dimensions::ETA;
use serde::{Deserialize, Serialize};

/// Consensus state for emission calculation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusState {
    /// Current block height
    pub block_height: u64,
    /// Consensus magnitude |ψ(t)| - derived from peer agreement
    pub psi_magnitude: f64,
    /// Peer count
    pub peer_count: u32,
    /// Agreement percentage (0.0 - 1.0)
    pub agreement: f64,
    /// Network hash rate estimate
    pub hash_rate: f64,
}

impl ConsensusState {
    /// Calculate |ψ(t)| from network metrics
    /// |ψ| = √(agreement² + normalized_hashrate²)
    pub fn calculate_psi_magnitude(&self, baseline_hashrate: f64) -> f64 {
        let normalized_hashrate = if baseline_hashrate > 0.0 {
            (self.hash_rate / baseline_hashrate).min(2.0)
        } else {
            1.0
        };
        
        // Combine agreement and hashrate into magnitude
        (self.agreement.powi(2) + normalized_hashrate.powi(2)).sqrt() / 2.0_f64.sqrt()
    }
}

/// Dynamic emission calculator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmissionCalculator {
    /// Base emission rate (tokens per block at |ψ| = 1)
    pub base_emission: u128,
    /// Minimum emission (floor)
    pub min_emission: u128,
    /// Maximum emission (ceiling)
    pub max_emission: u128,
    /// Baseline hash rate for normalization
    pub baseline_hashrate: f64,
    /// Halving interval (blocks)
    pub halving_interval: u64,
    /// Number of halvings occurred
    pub halvings: u32,
}

impl EmissionCalculator {
    /// Create new emission calculator
    pub fn new(base_emission: u128) -> Self {
        EmissionCalculator {
            base_emission,
            min_emission: base_emission / 100, // 1% floor
            max_emission: base_emission * 2,    // 200% ceiling
            baseline_hashrate: 1000.0,
            halving_interval: 210_000, // Similar to Bitcoin
            halvings: 0,
        }
    }

    /// Calculate emission rate: emission = η · |ψ(t)| · base / (2^halvings)
    pub fn calculate_emission(&self, consensus: &ConsensusState) -> u128 {
        let psi = consensus.calculate_psi_magnitude(self.baseline_hashrate);
        
        // emission_rate = η · |ψ(t)| · base_emission
        let raw_emission = (ETA * psi * self.base_emission as f64) as u128;
        
        // Apply halving
        let halved = raw_emission >> self.halvings;
        
        // Clamp to min/max
        halved.clamp(self.min_emission >> self.halvings, self.max_emission >> self.halvings)
    }

    /// Check and apply halving if needed
    pub fn check_halving(&mut self, block_height: u64) {
        let expected_halvings = (block_height / self.halving_interval) as u32;
        if expected_halvings > self.halvings {
            self.halvings = expected_halvings;
            println!("📉 Emission halving #{} at block {}", self.halvings, block_height);
        }
    }

    /// Get emission info
    pub fn emission_info(&self, consensus: &ConsensusState) -> EmissionInfo {
        let current = self.calculate_emission(consensus);
        let psi = consensus.calculate_psi_magnitude(self.baseline_hashrate);
        
        EmissionInfo {
            current_emission: current,
            base_emission: self.base_emission >> self.halvings,
            psi_magnitude: psi,
            eta_factor: ETA,
            halvings: self.halvings,
            next_halving_block: (self.halvings as u64 + 1) * self.halving_interval,
        }
    }
}

/// Emission info for reporting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmissionInfo {
    pub current_emission: u128,
    pub base_emission: u128,
    pub psi_magnitude: f64,
    pub eta_factor: f64,
    pub halvings: u32,
    pub next_halving_block: u64,
}

/// Emission distribution across dimensional pools
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmissionDistribution {
    /// D1 (Genesis/Mining): 28.7%
    pub mining: u128,
    /// D2 (Network Coupling): 24.9%  
    pub validators: u128,
    /// D3 (Staking): 21.5%
    pub staking: u128,
    /// D4 (Governance): 17.7%
    pub governance: u128,
    /// D5 (Bounties): 14.3%
    pub bounties: u128,
}

impl EmissionDistribution {
    /// Calculate distribution from total emission
    pub fn from_total(total: u128) -> Self {
        // Based on dimensional scales
        let d1 = 1.000;
        let d2 = 0.867;
        let d3 = 0.750;
        let d4 = 0.618;
        let d5 = 0.500;
        let sum = d1 + d2 + d3 + d4 + d5;
        
        EmissionDistribution {
            mining: ((total as f64) * (d1 / sum)) as u128,
            validators: ((total as f64) * (d2 / sum)) as u128,
            staking: ((total as f64) * (d3 / sum)) as u128,
            governance: ((total as f64) * (d4 / sum)) as u128,
            bounties: ((total as f64) * (d5 / sum)) as u128,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_emission_calculation() {
        let calculator = EmissionCalculator::new(1_000_000);
        
        let consensus = ConsensusState {
            block_height: 100,
            psi_magnitude: 1.0,
            peer_count: 10,
            agreement: 0.9,
            hash_rate: 1000.0,
        };
        
        let emission = calculator.calculate_emission(&consensus);
        
        // With |ψ| ≈ 1.0 and η = 0.707, emission should be ~70.7% of base
        assert!(emission > 0);
        assert!(emission < 1_000_000);
    }

    #[test]
    fn test_halving() {
        let mut calculator = EmissionCalculator::new(1_000_000);
        calculator.halving_interval = 100;
        
        let consensus = ConsensusState {
            block_height: 0,
            psi_magnitude: 1.0,
            peer_count: 10,
            agreement: 1.0,
            hash_rate: 1000.0,
        };
        
        let emission_before = calculator.calculate_emission(&consensus);
        
        calculator.check_halving(100);
        assert_eq!(calculator.halvings, 1);
        
        let emission_after = calculator.calculate_emission(&consensus);
        
        // After halving, emission should be roughly half
        assert!(emission_after < emission_before);
        assert!(emission_after > emission_before / 4);
    }

    #[test]
    fn test_emission_distribution() {
        let dist = EmissionDistribution::from_total(1_000_000);
        
        // Mining should get the largest share
        assert!(dist.mining > dist.validators);
        assert!(dist.validators > dist.staking);
        
        // Total should be close to input
        let total = dist.mining + dist.validators + dist.staking + dist.governance + dist.bounties;
        assert!(total > 990_000);
        assert!(total <= 1_000_000);
    }
}

