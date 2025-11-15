//! Exponential Dimensional Tokenomics Framework
//!
//! Implements the complete mathematical framework from the COINjecture whitepaper:
//! "Exponential Dimensional Tokenomics: A Mathematical Framework for Multi-Scale Cryptocurrency Stability"
//!
//! Key concepts:
//! - Satoshi Constant: η = λ = 1/√2 ≈ 0.707107
//! - Dimensional scales: D_n = e^(-η·τ_n)
//! - Complex eigenvalue dynamics: ψ(t) = e^(-ηt)e^(iλt)
//! - Viviani Oracle performance metric
//! - Self-referenced, dimensionless financial primitives

use serde::{Deserialize, Serialize};
use std::f64::consts::{E, SQRT_2};

/// The Satoshi Constant: η = λ = 1/√2 (Theorem 1 from whitepaper)
/// This is the unique critical equilibrium providing optimal stability without oscillation
pub const ETA: f64 = std::f64::consts::FRAC_1_SQRT_2; // 1/√2 ≈ 0.707107
pub const LAMBDA: f64 = std::f64::consts::FRAC_1_SQRT_2; // λ = η at critical equilibrium

/// Consensus time constant τ_c = 1/η = √2 (dimensionless time unit)
pub const TAU_C: f64 = SQRT_2;

/// Viviani Oracle: Performance reading at critical equilibrium (Section 2.5)
/// Δ = 0.231 represents 23.1% operation above conservative stability bound
pub const ORACLE_DELTA: f64 = 0.231;

/// Golden ratio inverse φ^(-1) = (√5 - 1) / 2 ≈ 0.618034
pub const PHI_INV: f64 = 0.618033988749895; // (√5 - 1) / 2

/// Golden ratio squared inverse φ^(-2) = (3 - √5) / 2 ≈ 0.382
pub const PHI_INV_2: f64 = 0.381966011250105; // (3 - √5) / 2

/// Eight dimensional economic scales as defined in Theorem 8
/// Each represents a dimensionless time snapshot of exponential decay
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct DimensionalScales {
    /// D₁: Genesis scale (τ=0.00, D=1.000) - Immediate liquidity
    pub d1: f64,
    /// D₂: Coupling scale (τ=0.20, D=0.867) - Short-term staking
    pub d2: f64,
    /// D₃: First harmonic (τ=0.41, D=0.750) - Primary liquidity
    pub d3: f64,
    /// D₄: Golden ratio scale (τ=0.68, D=0.618) - Treasury reserve
    pub d4: f64,
    /// D₅: Half-scale (τ=0.98, D=0.500) - Secondary liquidity
    pub d5: f64,
    /// D₆: Second golden scale (τ=1.36, D=0.382) - Long-term vesting
    pub d6: f64,
    /// D₇: Quarter-scale (τ=1.96, D=0.250) - Strategic reserve
    pub d7: f64,
    /// D₈: Euler scale (τ=2.72, D=0.146) - Foundation endowment
    pub d8: f64,
}

/// Dimensionless time points τ_n for each dimensional scale (Table 1)
pub const TAU_POINTS: [f64; 8] = [
    0.00, // D1: Genesis
    0.20, // D2: Coupling
    0.41, // D3: First harmonic
    0.68, // D4: Golden ratio
    0.98, // D5: Half-scale
    1.36, // D6: Second golden
    1.96, // D7: Quarter-scale
    2.72, // D8: Euler
];

impl DimensionalScales {
    /// Calculate dimensional scales from exponential decay: D_n = e^(-η·τ_n)
    /// Section 3.2, Theorem 8
    pub fn calculate() -> Self {
        Self {
            d1: Self::scale_at_tau(TAU_POINTS[0]), // e^0 = 1.000
            d2: Self::scale_at_tau(TAU_POINTS[1]), // 0.867
            d3: Self::scale_at_tau(TAU_POINTS[2]), // 0.750
            d4: Self::scale_at_tau(TAU_POINTS[3]), // 0.618 (φ^-1)
            d5: Self::scale_at_tau(TAU_POINTS[4]), // 0.500 (2^-1)
            d6: Self::scale_at_tau(TAU_POINTS[5]), // 0.382 (φ^-2)
            d7: Self::scale_at_tau(TAU_POINTS[6]), // 0.250 (2^-2)
            d8: Self::scale_at_tau(TAU_POINTS[7]), // 0.146 (e^-e/√2)
        }
    }

    /// Calculate scale at dimensionless time τ: D(τ) = e^(-η·τ)
    #[inline]
    pub fn scale_at_tau(tau: f64) -> f64 {
        (-ETA * tau).exp()
    }

    /// Get scale by dimension index (0-7 for D1-D8)
    pub fn get(&self, dimension: usize) -> Option<f64> {
        match dimension {
            0 => Some(self.d1),
            1 => Some(self.d2),
            2 => Some(self.d3),
            3 => Some(self.d4),
            4 => Some(self.d5),
            5 => Some(self.d6),
            6 => Some(self.d7),
            7 => Some(self.d8),
            _ => None,
        }
    }

    /// Calculate normalized scales satisfying conservation constraint (Section 5.2)
    /// Σ D̃_n² = 1
    pub fn normalized(&self) -> Self {
        let sum_squares = self.d1.powi(2)
            + self.d2.powi(2)
            + self.d3.powi(2)
            + self.d4.powi(2)
            + self.d5.powi(2)
            + self.d6.powi(2)
            + self.d7.powi(2)
            + self.d8.powi(2);

        let norm = sum_squares.sqrt();

        Self {
            d1: self.d1 / norm,
            d2: self.d2 / norm,
            d3: self.d3 / norm,
            d4: self.d4 / norm,
            d5: self.d5 / norm,
            d6: self.d6 / norm,
            d7: self.d7 / norm,
            d8: self.d8 / norm,
        }
    }

    /// Calculate allocation ratios for dimensional pools (Table 2, Section 6.2)
    /// p_n(t) = D̃_n(t) / Σ D̃_k(t)
    pub fn allocation_ratios(&self) -> [f64; 8] {
        let normalized = self.normalized();
        let sum = normalized.d1
            + normalized.d2
            + normalized.d3
            + normalized.d4
            + normalized.d5
            + normalized.d6
            + normalized.d7
            + normalized.d8;

        [
            normalized.d1 / sum, // D1: 0.222
            normalized.d2 / sum, // D2: 0.192
            normalized.d3 / sum, // D3: 0.166
            normalized.d4 / sum, // D4: 0.137
            normalized.d5 / sum, // D5: 0.111
            normalized.d6 / sum, // D6: 0.085
            normalized.d7 / sum, // D7: 0.055
            normalized.d8 / sum, // D8: 0.032
        ]
    }
}

/// Consensus state dynamics ψ(t) = e^(-ηt)e^(iλt)
/// Tracks the complex eigenvalue evolution (Section 2, Theorem 7)
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ConsensusState {
    /// Dimensionless time τ = t/τ_c (measured in units of consensus time constant)
    pub tau: f64,
    /// Magnitude |ψ(τ)| = e^(-ητ) (exponential decay)
    pub magnitude: f64,
    /// Phase angle θ(τ) = λτ (radians)
    pub phase: f64,
}

impl ConsensusState {
    /// Create new consensus state at dimensionless time τ
    /// Section 3.1, Theorem 7
    pub fn at_tau(tau: f64) -> Self {
        Self {
            tau,
            magnitude: (-ETA * tau).exp(), // |ψ(τ)| = e^(-ητ)
            phase: LAMBDA * tau,            // θ(τ) = λτ
        }
    }

    /// Calculate self-referenced dimensional scales: D̃_n(τ) = |ψ(τ)| · D_n
    /// These adapt to network consensus state (Section 6.2, Remark 7)
    pub fn dimensional_scales(&self) -> DimensionalScales {
        let base = DimensionalScales::calculate();
        DimensionalScales {
            d1: self.magnitude * base.d1,
            d2: self.magnitude * base.d2,
            d3: self.magnitude * base.d3,
            d4: self.magnitude * base.d4,
            d5: self.magnitude * base.d5,
            d6: self.magnitude * base.d6,
            d7: self.magnitude * base.d7,
            d8: self.magnitude * base.d8,
        }
    }

    /// Exponential unlock schedule for pool n (Section 6.3, Definition 10)
    /// U_n(τ) = 1 - e^(-η(τ - τ_n)) for τ >= τ_n, else 0
    /// Returns value in [0, 1] representing fraction unlocked
    pub fn unlock_fraction(&self, dimension: usize) -> f64 {
        if dimension >= TAU_POINTS.len() {
            return 0.0;
        }

        let tau_n = TAU_POINTS[dimension];
        if self.tau < tau_n {
            0.0 // Not yet unlocked
        } else {
            1.0 - (-ETA * (self.tau - tau_n)).exp()
        }
    }

    /// Yield rate for pool n (Section 6.4, Theorem 14)
    /// r_n(τ) = η · (D_n/D1) = η · e^(-ητ_n)
    /// Dimensionless, self-referenced to network consensus dynamics
    pub fn yield_rate(&self, dimension: usize) -> f64 {
        if dimension >= TAU_POINTS.len() {
            return 0.0;
        }

        let tau_n = TAU_POINTS[dimension];
        ETA * (-ETA * tau_n).exp() // η · e^(-ητ_n)
    }
}

/// Viviani Oracle: Performance envelope and "rev limiter" (Section 2.5)
/// Measures system performance relative to conservative stability boundary
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct VivianiOracle {
    /// Distance from point to side 1 (λ = 0)
    pub d1: f64,
    /// Distance from point to side 2 (η = 0)
    pub d2: f64,
    /// Distance from point to side 3 (triangle edge)
    pub d3: f64,
    /// Oracle metric Δ = (d1 + d2 + d3) / (√3/2) - 1
    pub delta: f64,
}

impl VivianiOracle {
    /// Calculate oracle reading for current (η, λ) point
    /// Section 2.5, Definition 5
    /// Uses equilateral triangle with vertices: (0,0), (1,0), (0.5, √3/2)
    pub fn calculate(eta: f64, lambda: f64) -> Self {
        // Viviani's constant for equilateral triangle with side length 1
        const ALTITUDE: f64 = 0.866025403784439; // √3/2
        const SQRT_3: f64 = 1.732050807568877; // √3

        // Equilateral triangle vertices: (0,0), (1,0), (0.5, √3/2)
        // Side 1: Bottom edge from (0,0) to (1,0), equation: λ = 0
        let d1 = lambda.abs();

        // Side 2: Left edge from (0,0) to (0.5, √3/2)
        // Line equation: √3·η - λ = 0, or λ = √3·η
        // Distance = |√3·η - λ| / √(3 + 1) = |√3·η - λ| / 2
        let d2 = (SQRT_3 * eta - lambda).abs() / 2.0;

        // Side 3: Right edge from (1,0) to (0.5, √3/2)
        // Line equation: √3·η + λ = √3 (slope = -√3, from (1,0) to (0.5, √3/2))
        // Distance = |√3·η + λ - √3| / 2
        let d3 = (SQRT_3 * eta + lambda - SQRT_3).abs() / 2.0;

        let sum = d1 + d2 + d3;
        let delta = sum / ALTITUDE - 1.0;

        Self { d1, d2, d3, delta }
    }

    /// Get performance regime based on Δ value (Corollary 5)
    pub fn regime(&self) -> PerformanceRegime {
        match self.delta {
            d if d < 0.1 => PerformanceRegime::Idle,
            d if d < 0.2 => PerformanceRegime::Cruise,
            d if d < 0.3 => PerformanceRegime::Performance,
            _ => PerformanceRegime::Redline,
        }
    }
}

/// Performance operating regimes (Section 2.5, Corollary 5)
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum PerformanceRegime {
    /// Δ < 0.1: Conservative, slow convergence
    Idle,
    /// 0.1 ≤ Δ < 0.2: Moderate performance, stable
    Cruise,
    /// 0.2 ≤ Δ < 0.3: Optimal convergence regime (critical equilibrium at Δ=0.231)
    Performance,
    /// Δ ≥ 0.3: Fast but oscillatory, requires active control
    Redline,
}

/// Complete dimensional economics state
/// Unifies consensus dynamics with tokenomics (Section 9.1)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DimensionalEconomics {
    /// Current consensus state ψ(τ)
    pub consensus: ConsensusState,
    /// Dimensional scales D_n
    pub scales: DimensionalScales,
    /// Viviani Oracle performance metric
    pub oracle: VivianiOracle,
}

impl DimensionalEconomics {
    /// Create new dimensional economics state at dimensionless time τ
    pub fn at_tau(tau: f64) -> Self {
        let consensus = ConsensusState::at_tau(tau);
        let scales = consensus.dimensional_scales();
        let oracle = VivianiOracle::calculate(ETA, LAMBDA);

        Self {
            consensus,
            scales,
            oracle,
        }
    }

    /// Allocate total supply across 8 dimensional pools (Section 6.2)
    /// Returns [S_1, S_2, ..., S_8] where Σ S_n = total_supply
    pub fn allocate_supply(&self, total_supply: u128) -> [u128; 8] {
        let ratios = self.scales.allocation_ratios();
        let mut allocations = [0u128; 8];

        let mut allocated = 0u128;
        for i in 0..7 {
            allocations[i] = (total_supply as f64 * ratios[i]) as u128;
            allocated += allocations[i];
        }

        // Give remainder to D8 to ensure exact sum
        allocations[7] = total_supply.saturating_sub(allocated);

        allocations
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_satoshi_constant() {
        // Verify η = λ = 1/√2 (Theorem 1)
        assert!((ETA - LAMBDA).abs() < 1e-10);
        assert!((ETA - std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-10);

        // Verify unit circle constraint: η² + λ² = 1 (Eq. 3)
        let constraint = ETA.powi(2) + LAMBDA.powi(2);
        assert!((constraint - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_dimensional_scales() {
        let scales = DimensionalScales::calculate();

        // D1: Genesis (τ=0.00, D=1.000)
        assert!((scales.d1 - 1.000).abs() < 0.001);

        // D4: Golden ratio (τ=0.68, D≈0.618)
        assert!((scales.d4 - PHI_INV).abs() < 0.001);

        // D5: Half-scale (τ=0.98, D=0.500)
        assert!((scales.d5 - 0.500).abs() < 0.001);

        // D7: Quarter-scale (τ=1.96, D=0.250)
        assert!((scales.d7 - 0.250).abs() < 0.001);
    }

    #[test]
    fn test_conservation_constraint() {
        let scales = DimensionalScales::calculate();
        let normalized = scales.normalized();

        // Verify Σ D̃_n² = 1 (Section 5.2)
        let sum_squares = normalized.d1.powi(2)
            + normalized.d2.powi(2)
            + normalized.d3.powi(2)
            + normalized.d4.powi(2)
            + normalized.d5.powi(2)
            + normalized.d6.powi(2)
            + normalized.d7.powi(2)
            + normalized.d8.powi(2);

        assert!((sum_squares - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_allocation_ratios() {
        let scales = DimensionalScales::calculate();
        let ratios = scales.allocation_ratios();

        // Verify ratios sum to 1
        let sum: f64 = ratios.iter().sum();
        assert!((sum - 1.0).abs() < 1e-10);

        // Verify D1 has highest allocation (≈0.222 from Table 2)
        assert!(ratios[0] > ratios[1]);
        assert!((ratios[0] - 0.222).abs() < 0.01);
    }

    #[test]
    fn test_viviani_oracle() {
        // Test at critical equilibrium (Section 2.5, Theorem 4)
        let oracle = VivianiOracle::calculate(ETA, LAMBDA);

        // Δ should be ≈0.231 (23.1% above conservative bound)
        assert!((oracle.delta - ORACLE_DELTA).abs() < 0.01);

        // Should be in Performance regime
        assert_eq!(oracle.regime(), PerformanceRegime::Performance);
    }

    #[test]
    fn test_consensus_evolution() {
        let state = ConsensusState::at_tau(0.0);

        // At τ=0: |ψ(0)| = 1, θ(0) = 0
        assert!((state.magnitude - 1.0).abs() < 1e-10);
        assert!(state.phase.abs() < 1e-10);

        // At τ=√2: |ψ(√2)| = e^(-1) ≈ 0.368
        let state_tau_c = ConsensusState::at_tau(TAU_C);
        assert!((state_tau_c.magnitude - (1.0 / E)).abs() < 0.001);
    }

    #[test]
    fn test_unlock_schedule() {
        // Test at τ = 7.0 (long enough for D1 to be nearly fully unlocked)
        let state = ConsensusState::at_tau(7.0);

        // D1 (τ=0.00): Should be fully unlocked (U_1(7.0) > 0.99)
        let unlock_d1 = state.unlock_fraction(0);
        assert!(unlock_d1 > 0.99, "D1 unlock at τ=7.0: {}", unlock_d1);

        // Test at τ = 1.5 for middle pools
        let state_mid = ConsensusState::at_tau(1.5);

        // D2 (τ=0.20): Should be significantly unlocked
        let unlock_d2 = state_mid.unlock_fraction(1);
        assert!(unlock_d2 > 0.60, "D2 unlock at τ=1.5: {}", unlock_d2);

        // D5 (τ=0.98): Should be partially unlocked
        let unlock_d5 = state_mid.unlock_fraction(4);
        assert!(unlock_d5 > 0.0 && unlock_d5 < 1.0, "D5 unlock at τ=1.5: {}", unlock_d5);

        // D8 (τ=2.72): Should not be unlocked yet at τ=1.5
        let unlock_d8 = state_mid.unlock_fraction(7);
        assert!(unlock_d8 < 0.01, "D8 unlock at τ=1.5: {}", unlock_d8);
    }

    #[test]
    fn test_yield_rates() {
        let state = ConsensusState::at_tau(0.0);

        // D1 yield should be highest (η · 1.0 = η)
        let yield_d1 = state.yield_rate(0);
        assert!((yield_d1 - ETA).abs() < 1e-10);

        // D5 yield should be half of D1 (η · 0.5)
        let yield_d5 = state.yield_rate(4);
        assert!((yield_d5 - ETA * 0.5).abs() < 0.01);
    }
}
