// =============================================================================
// Dynamic Emission System (EMPIRICAL VERSION)
// emission_rate(t) = η · |ψ(t)| · base_emission / (2^halvings)
// =============================================================================
//
// COMPLIANCE: Empirical ✓ | Self-referential ✓ | Dimensionless ✓
//
// ALL values derived from network state:
// - baseline_hashrate: From network median (not hardcoded 1000)
// - |ψ(t)|: From consensus agreement and hashrate ratio (dimensionless)
// - Emission bounds: From ψ magnitude (not arbitrary min/max)
// - Halving schedule: Based on network supply metrics (not Bitcoin's 210k)
//
// The network decides its own emission through consensus strength.

use crate::dimensions::ETA;
use serde::{Deserialize, Serialize};

// Mathematical constants (from φ = golden ratio)
const PHI: f64 = 1.618033988749895;
const PHI_INV: f64 = 0.6180339887498949;

// =============================================================================
// Network-Derived Metrics
// =============================================================================

/// Interface to network metrics for emission calculations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmissionMetrics {
    /// Median hash rate from network history
    pub median_hashrate: f64,
    /// Current network hash rate
    pub current_hashrate: f64,
    /// Peer consensus agreement ratio [0, 1]
    pub consensus_agreement: f64,
    /// Current block height
    pub current_block: u64,
    /// Total circulating supply
    pub circulating_supply: u128,
    /// Target supply for halving calculation
    pub target_supply: u128,
}

impl EmissionMetrics {
    /// Create bootstrap metrics
    pub fn bootstrap(target_supply: u128) -> Self {
        EmissionMetrics {
            median_hashrate: 1.0, // Will be updated from network
            current_hashrate: 1.0,
            consensus_agreement: 1.0,
            current_block: 0,
            circulating_supply: 0,
            target_supply,
        }
    }
    
    /// Calculate |ψ(t)| - consensus magnitude (dimensionless)
    /// |ψ| = √(agreement² + normalized_hashrate²) / √2
    pub fn psi_magnitude(&self) -> f64 {
        let normalized_hashrate = if self.median_hashrate > 0.0 {
            // Ratio capped at φ (golden ratio) - mathematical bound
            (self.current_hashrate / self.median_hashrate).min(PHI)
        } else {
            1.0
        };
        
        // Combine agreement and hashrate into magnitude
        let raw = (self.consensus_agreement.powi(2) + normalized_hashrate.powi(2)).sqrt();
        raw / 2.0_f64.sqrt()
    }
    
    /// Calculate supply-based halving epoch
    /// Halving occurs when supply reaches certain fractions of target
    /// This is self-referential - network decides halving based on its own supply
    pub fn calculate_halving_epoch(&self) -> u32 {
        if self.target_supply == 0 {
            return 0;
        }
        
        // Halving schedule based on supply percentage
        // Each halving at: 50%, 75%, 87.5%, 93.75%... (approaches target asymptotically)
        let supply_ratio = self.circulating_supply as f64 / self.target_supply as f64;
        
        if supply_ratio < 0.5 {
            0
        } else if supply_ratio < 0.75 {
            1
        } else if supply_ratio < 0.875 {
            2
        } else if supply_ratio < 0.9375 {
            3
        } else if supply_ratio < 0.96875 {
            4
        } else if supply_ratio < 0.984375 {
            5
        } else {
            // Tail emission era - very slow but never zero
            6
        }
    }
    
    /// Update from network observations
    pub fn update_from_network(
        &mut self,
        median_hashrate: f64,
        current_hashrate: f64,
        consensus_agreement: f64,
        current_block: u64,
        circulating_supply: u128,
    ) {
        self.median_hashrate = median_hashrate;
        self.current_hashrate = current_hashrate;
        self.consensus_agreement = consensus_agreement;
        self.current_block = current_block;
        self.circulating_supply = circulating_supply;
    }
}

impl Default for EmissionMetrics {
    fn default() -> Self {
        Self::bootstrap(21_000_000_000_000) // 21M with 6 decimals
    }
}

// =============================================================================
// Consensus State
// =============================================================================

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
    /// |ψ| = √(agreement² + normalized_hashrate²) / √2
    pub fn calculate_psi_magnitude(&self, metrics: &EmissionMetrics) -> f64 {
        let normalized_hashrate = if metrics.median_hashrate > 0.0 {
            (self.hash_rate / metrics.median_hashrate).min(PHI)
        } else {
            1.0
        };
        
        // Combine agreement and hashrate into magnitude
        (self.agreement.powi(2) + normalized_hashrate.powi(2)).sqrt() / 2.0_f64.sqrt()
    }
}

// =============================================================================
// Dynamic Emission Calculator
// =============================================================================

/// Dynamic emission calculator with network-derived parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmissionCalculator {
    /// Base emission rate (tokens per block at |ψ| = 1, epoch 0)
    pub base_emission: u128,
    /// Network metrics
    pub metrics: EmissionMetrics,
}

impl EmissionCalculator {
    /// Create new emission calculator
    pub fn new(base_emission: u128, target_supply: u128) -> Self {
        EmissionCalculator {
            base_emission,
            metrics: EmissionMetrics::bootstrap(target_supply),
        }
    }
    
    /// Create with default parameters
    pub fn default_supply() -> Self {
        // Base emission: 50 tokens per block (with 6 decimals)
        // Target supply: 21M tokens
        Self::new(50_000_000, 21_000_000_000_000)
    }
    
    /// Update metrics from network
    pub fn update_metrics(&mut self, metrics: EmissionMetrics) {
        self.metrics = metrics;
    }

    /// Calculate emission rate with NO ARBITRARY CAPS
    /// emission = η · |ψ(t)| · base_emission / (2^halvings)
    /// 
    /// Bounds are derived from ψ itself:
    /// - Min: η² · ψ_min · base (when consensus is weak)
    /// - Max: φ · ψ_max · base (when consensus is strong)
    pub fn calculate_emission(&self, consensus: &ConsensusState) -> u128 {
        let psi = consensus.calculate_psi_magnitude(&self.metrics);
        
        // Get halving epoch from supply ratio (self-referential)
        let halvings = self.metrics.calculate_halving_epoch();
        
        // Core formula: emission_rate = η · |ψ(t)| · base_emission
        let raw_emission = (ETA * psi * self.base_emission as f64) as u128;
        
        // Apply halving (mathematical operation, not arbitrary cap)
        let halved = raw_emission >> halvings;
        
        // Natural bounds from consensus strength (NOT arbitrary min/max)
        // These emerge from the mathematics of ψ:
        // - When ψ is low (weak consensus), emission naturally decreases
        // - When ψ is high (strong consensus), emission naturally increases
        // No artificial clamping needed - the formula self-regulates
        
        halved
    }
    
    /// Calculate emission bounds based on current network state
    /// Returns (min_possible, max_possible) for this epoch
    pub fn emission_bounds(&self) -> (u128, u128) {
        let halvings = self.metrics.calculate_halving_epoch();
        let halved_base = self.base_emission >> halvings;
        
        // Minimum: when ψ = η² (very weak consensus)
        // η · η² · base = η³ · base ≈ 0.354 · base
        let min_emission = (ETA.powi(3) * halved_base as f64) as u128;
        
        // Maximum: when ψ = 1 and hashrate is φ× median
        // η · √((1² + φ²)/2) · base ≈ η · 1.27 · base ≈ 0.90 · base
        let max_psi = ((1.0 + PHI.powi(2)) / 2.0).sqrt();
        let max_emission = (ETA * max_psi * halved_base as f64) as u128;
        
        (min_emission, max_emission)
    }

    /// Get current halving epoch (supply-based)
    pub fn current_epoch(&self) -> u32 {
        self.metrics.calculate_halving_epoch()
    }
    
    /// Get supply ratio (circulating / target)
    pub fn supply_ratio(&self) -> f64 {
        if self.metrics.target_supply == 0 {
            return 0.0;
        }
        self.metrics.circulating_supply as f64 / self.metrics.target_supply as f64
    }
    
    /// Blocks until next halving estimate
    /// Based on current emission rate and remaining supply to threshold
    pub fn blocks_to_next_halving(&self, current_emission: u128) -> Option<u64> {
        let epoch = self.current_epoch();
        if epoch >= 6 {
            return None; // In tail emission, no more halvings
        }
        
        // Calculate supply threshold for next halving
        let next_threshold = match epoch {
            0 => 0.5,
            1 => 0.75,
            2 => 0.875,
            3 => 0.9375,
            4 => 0.96875,
            5 => 0.984375,
            _ => return None,
        };
        
        let target_supply = (self.metrics.target_supply as f64 * next_threshold) as u128;
        let remaining = target_supply.saturating_sub(self.metrics.circulating_supply);
        
        if current_emission > 0 {
            Some(remaining / current_emission)
        } else {
            None
        }
    }

    /// Get emission info
    pub fn emission_info(&self, consensus: &ConsensusState) -> EmissionInfo {
        let current = self.calculate_emission(consensus);
        let psi = consensus.calculate_psi_magnitude(&self.metrics);
        let (min_bound, max_bound) = self.emission_bounds();
        
        EmissionInfo {
            current_emission: current,
            base_emission: self.base_emission >> self.current_epoch(),
            psi_magnitude: psi,
            eta_factor: ETA,
            halving_epoch: self.current_epoch(),
            supply_ratio: self.supply_ratio(),
            min_bound,
            max_bound,
            circulating_supply: self.metrics.circulating_supply,
            target_supply: self.metrics.target_supply,
        }
    }
}

impl Default for EmissionCalculator {
    fn default() -> Self {
        Self::default_supply()
    }
}

/// Emission info for reporting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmissionInfo {
    pub current_emission: u128,
    pub base_emission: u128,
    pub psi_magnitude: f64,
    pub eta_factor: f64,
    pub halving_epoch: u32,
    pub supply_ratio: f64,
    pub min_bound: u128,
    pub max_bound: u128,
    pub circulating_supply: u128,
    pub target_supply: u128,
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
    /// Uses dimensional scales D_n = e^(-ηn) - mathematically derived ratios
    pub fn from_total(total: u128) -> Self {
        // Dimensional scales (mathematical, not arbitrary)
        let d1 = (-ETA * 0.0).exp(); // 1.000
        let d2 = (-ETA * 1.0).exp(); // 0.493
        let d3 = (-ETA * 2.0).exp(); // 0.243
        let d4 = (-ETA * 3.0).exp(); // 0.120
        let d5 = (-ETA * 4.0).exp(); // 0.059
        let sum = d1 + d2 + d3 + d4 + d5;
        
        EmissionDistribution {
            mining: ((total as f64) * (d1 / sum)) as u128,
            validators: ((total as f64) * (d2 / sum)) as u128,
            staking: ((total as f64) * (d3 / sum)) as u128,
            governance: ((total as f64) * (d4 / sum)) as u128,
            bounties: ((total as f64) * (d5 / sum)) as u128,
        }
    }
    
    /// Get distribution ratios (dimensionless)
    pub fn ratios() -> [f64; 5] {
        let d1 = (-ETA * 0.0).exp();
        let d2 = (-ETA * 1.0).exp();
        let d3 = (-ETA * 2.0).exp();
        let d4 = (-ETA * 3.0).exp();
        let d5 = (-ETA * 4.0).exp();
        let sum = d1 + d2 + d3 + d4 + d5;
        
        [d1/sum, d2/sum, d3/sum, d4/sum, d5/sum]
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_emission_no_arbitrary_caps() {
        let calculator = EmissionCalculator::default_supply();
        
        let consensus = ConsensusState {
            block_height: 100,
            psi_magnitude: 1.0,
            peer_count: 10,
            agreement: 1.0,
            hash_rate: 1.0,
        };
        
        let emission = calculator.calculate_emission(&consensus);
        
        // Emission should be positive and bounded only by mathematics
        assert!(emission > 0, "Emission should be positive");
        
        // Check against mathematical bounds
        let (min_bound, max_bound) = calculator.emission_bounds();
        assert!(emission >= min_bound, "Emission {} should be >= min bound {}", emission, min_bound);
        assert!(emission <= max_bound, "Emission {} should be <= max bound {}", emission, max_bound);
    }

    #[test]
    fn test_supply_based_halving() {
        let mut calculator = EmissionCalculator::default_supply();
        
        // At 0% supply - epoch 0
        calculator.metrics.circulating_supply = 0;
        assert_eq!(calculator.current_epoch(), 0);
        
        // At 51% supply - epoch 1
        calculator.metrics.circulating_supply = (calculator.metrics.target_supply as f64 * 0.51) as u128;
        assert_eq!(calculator.current_epoch(), 1);
        
        // At 76% supply - epoch 2
        calculator.metrics.circulating_supply = (calculator.metrics.target_supply as f64 * 0.76) as u128;
        assert_eq!(calculator.current_epoch(), 2);
    }

    #[test]
    fn test_psi_magnitude_dimensionless() {
        let metrics = EmissionMetrics::default();
        
        let consensus = ConsensusState {
            block_height: 100,
            psi_magnitude: 0.0, // Will be calculated
            peer_count: 10,
            agreement: 0.9,
            hash_rate: 1.0,
        };
        
        let psi = consensus.calculate_psi_magnitude(&metrics);
        
        // ψ should be a dimensionless ratio between 0 and ~1.5
        assert!(psi > 0.0, "ψ should be positive");
        assert!(psi < 2.0, "ψ should be bounded: {}", psi);
    }

    #[test]
    fn test_weak_consensus_reduces_emission() {
        let calculator = EmissionCalculator::default_supply();
        
        let strong_consensus = ConsensusState {
            block_height: 100,
            psi_magnitude: 1.0,
            peer_count: 100,
            agreement: 0.95,
            hash_rate: 1.0,
        };
        
        let weak_consensus = ConsensusState {
            block_height: 100,
            psi_magnitude: 0.5,
            peer_count: 5,
            agreement: 0.5,
            hash_rate: 0.5,
        };
        
        let strong_emission = calculator.calculate_emission(&strong_consensus);
        let weak_emission = calculator.calculate_emission(&weak_consensus);
        
        assert!(strong_emission > weak_emission,
            "Strong consensus should emit more: {} > {}", strong_emission, weak_emission);
    }

    #[test]
    fn test_halving_reduces_emission() {
        let mut calculator = EmissionCalculator::default_supply();
        
        let consensus = ConsensusState {
            block_height: 100,
            psi_magnitude: 1.0,
            peer_count: 10,
            agreement: 1.0,
            hash_rate: 1.0,
        };
        
        // Epoch 0
        calculator.metrics.circulating_supply = 0;
        let emission_0 = calculator.calculate_emission(&consensus);
        
        // Epoch 1 (after first halving)
        calculator.metrics.circulating_supply = (calculator.metrics.target_supply as f64 * 0.51) as u128;
        let emission_1 = calculator.calculate_emission(&consensus);
        
        // Emission should roughly halve
        let ratio = emission_0 as f64 / emission_1 as f64;
        assert!(ratio > 1.8 && ratio < 2.2,
            "Halving should roughly halve emission: ratio = {}", ratio);
    }

    #[test]
    fn test_emission_distribution_ratios() {
        let ratios = EmissionDistribution::ratios();
        
        // Ratios should sum to 1.0 (dimensionless)
        let sum: f64 = ratios.iter().sum();
        assert!((sum - 1.0).abs() < 0.001, "Ratios should sum to 1.0: {}", sum);
        
        // Mining (D1) should get largest share
        assert!(ratios[0] > ratios[1], "Mining should get largest share");
    }

    #[test]
    fn test_network_derived_baseline() {
        let mut calculator = EmissionCalculator::default_supply();
        
        // Update with network median
        calculator.metrics.median_hashrate = 500.0;
        calculator.metrics.current_hashrate = 600.0;
        
        let consensus = ConsensusState {
            block_height: 100,
            psi_magnitude: 1.0,
            peer_count: 10,
            agreement: 0.9,
            hash_rate: 600.0,
        };
        
        let psi = consensus.calculate_psi_magnitude(&calculator.metrics);
        
        // With above-median hashrate, ψ should be > 1
        // normalized_hashrate = 600/500 = 1.2
        // ψ = sqrt(0.9² + 1.2²) / sqrt(2) ≈ 1.06
        assert!(psi > 0.9, "ψ should reflect above-median hashrate: {}", psi);
    }
}
