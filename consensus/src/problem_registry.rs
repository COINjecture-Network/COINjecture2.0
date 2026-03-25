//! # Problem Registry & Trait System
//!
//! Extensible framework for ANY NP-complete, co-NP-complete, or NP-hard problem type.
//!
//! COMPLIANCE: Empirical ✓ | Self-referential ✓ | Dimensionless ✓
//!
//! Adding a new problem type requires ONLY:
//!   1. Implement ProblemDescriptor for your type
//!   2. Register it with ProblemRegistry::register()
//!
//! No changes to DifficultyAdjuster, WorkScoreCalculator, or any other system code.
//!
//! # Mathematical Framework and Seeded Defaults
//!
//! The μ-framework operates in dimensionless time (τ). The Satoshi constant
//! η = λ = 1/√2 governs the *shape* of system dynamics (critically damped,
//! no oscillation). It does NOT determine wall-clock speed.
//!
//! - η = λ = 1/√2 (from unit circle constraint |μ|² = η² + λ² = 1)
//! - τ_c = η⁻¹ = √2 (dimensionless characteristic time)
//!
//! The block time of 10 seconds is an engineering parameter chosen for
//! practical network operation: long enough for NP solving to be meaningful,
//! short enough for reasonable transaction finality, and enough overhead
//! margin for verification plus propagation. It is independent of η —
//! changing the block time changes the clock speed at which the dimensionless
//! dynamics manifest, but not the dynamics themselves.
//!
//! Scaling exponents are initial seeds refined by empirical observation:
//! - SubsetSum (0.8): exponential family, 2^(0.8n) expected scaling
//! - SAT (0.7): DPLL-family solvers, slightly harder scaling per variable
//! - TSP (0.5): factorial family in log-space, hardest scaling per city
//! - Factorization (0.33): sub-exponential L_n[1/3, c] (GNFS family)
//!
//! After the bootstrap phase (blocks 0–19), the network refines these from
//! observed solve times. See docs/BOOTSTRAP.md for details.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use tokio::sync::RwLock;

// ---------------------------------------------------------------------------
// Core enums
// ---------------------------------------------------------------------------

/// Complexity class — determines how the system reasons about scaling behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ComplexityClass {
    /// NP-complete (decision problems, e.g. SAT, SubsetSum, GraphColoring)
    NpComplete,
    /// co-NP-complete (e.g. tautology checking)
    CoNpComplete,
    /// NP-hard optimization (e.g. TSP, SVP)
    NpHard,
}

impl fmt::Display for ComplexityClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ComplexityClass::NpComplete => write!(f, "NP-Complete"),
            ComplexityClass::CoNpComplete => write!(f, "co-NP-Complete"),
            ComplexityClass::NpHard => write!(f, "NP-Hard"),
        }
    }
}

/// How verification cost grows with problem size.
#[derive(Debug, Clone, Copy)]
pub enum VerificationCost {
    /// O(1) — e.g. integer factorization (just multiply p*q)
    Constant,
    /// O(n) — e.g. TSP (walk the tour), SAT (evaluate assignment)
    Linear,
    /// O(n log n)
    NLogN,
    /// O(V + E) — graph traversal
    GraphLinear,
    /// O(n^k) for some small k
    Polynomial(f64),
}

impl VerificationCost {
    /// Estimate verification time ratio relative to size 1.
    /// Returns a dimensionless scaling factor.
    pub fn scaling_factor(&self, size: usize) -> f64 {
        let n = size as f64;
        match self {
            VerificationCost::Constant => 1.0,
            VerificationCost::Linear => n,
            VerificationCost::NLogN => n * n.ln().max(1.0),
            VerificationCost::GraphLinear => n,
            VerificationCost::Polynomial(k) => n.powf(*k),
        }
    }
}

// ---------------------------------------------------------------------------
// The core trait
// ---------------------------------------------------------------------------

/// Describes the computational characteristics of a problem type.
///
/// This is the ONLY thing you need to implement to add a new problem type
/// to COINjecture. Everything else (difficulty adjustment, work score
/// calculation, size limits) derives from these descriptors automatically.
///
/// All methods return dimensionless values or ratios — no arbitrary units.
pub trait ProblemDescriptor: Send + Sync {
    /// Unique identifier string (e.g. "SubsetSum", "SAT", "TSP")
    fn name(&self) -> &str;

    /// Complexity class of this problem
    fn complexity_class(&self) -> ComplexityClass;

    /// Scaling exponent: how solve time grows with problem size.
    ///
    /// For exponential problems (2^n): returns ~1.0
    /// For factorial problems (n!):    returns ~0.5  (in log-space)
    /// For sub-exponential (L_n[1/3]): returns ~0.33
    fn scaling_exponent(&self) -> f64;

    /// How verification cost scales with problem size.
    fn verification_cost(&self) -> VerificationCost;

    /// Size-to-base-size ratio: how this problem's "natural" size at
    /// a given difficulty compares to the network's canonical size unit.
    /// SubsetSum = 1.0 by convention. TSP = 0.35, SAT = 0.75, etc.
    fn size_ratio(&self) -> f64;

    /// Base difficulty weight for work score calculation.
    /// SubsetSum = 1.0 by convention.
    fn base_difficulty_weight(&self) -> f64;

    /// Whether this problem supports quality gradations (optimization)
    /// or is binary (decision: correct/incorrect).
    fn has_quality_gradient(&self) -> bool;

    /// Maximum safe problem size — hard ceiling.
    fn absolute_max_size(&self) -> usize;

    /// Minimum meaningful problem size.
    fn absolute_min_size(&self) -> usize {
        3
    }

    /// Optional: generate a problem instance deterministically from a seed.
    fn generate_instance(&self, _size: usize, _seed: &[u8]) -> Option<Vec<u8>> {
        None
    }

    /// Optional: verify a solution. Returns quality score in [0.0, 1.0].
    fn verify_solution(&self, _instance: &[u8], _solution: &[u8]) -> Option<f64> {
        None
    }
}

// ---------------------------------------------------------------------------
// Built-in problem descriptors
// ---------------------------------------------------------------------------

pub struct SubsetSumDescriptor;
impl ProblemDescriptor for SubsetSumDescriptor {
    fn name(&self) -> &str {
        "SubsetSum"
    }
    fn complexity_class(&self) -> ComplexityClass {
        ComplexityClass::NpComplete
    }
    fn scaling_exponent(&self) -> f64 {
        0.8
    }
    fn verification_cost(&self) -> VerificationCost {
        VerificationCost::Linear
    }
    fn size_ratio(&self) -> f64 {
        1.0
    }
    fn base_difficulty_weight(&self) -> f64 {
        1.0
    }
    fn has_quality_gradient(&self) -> bool {
        false
    }
    fn absolute_max_size(&self) -> usize {
        60
    }
}

pub struct SatDescriptor;
impl ProblemDescriptor for SatDescriptor {
    fn name(&self) -> &str {
        "SAT"
    }
    fn complexity_class(&self) -> ComplexityClass {
        ComplexityClass::NpComplete
    }
    fn scaling_exponent(&self) -> f64 {
        0.7
    }
    fn verification_cost(&self) -> VerificationCost {
        VerificationCost::Linear
    }
    fn size_ratio(&self) -> f64 {
        0.75
    }
    fn base_difficulty_weight(&self) -> f64 {
        1.2
    }
    fn has_quality_gradient(&self) -> bool {
        false
    }
    fn absolute_max_size(&self) -> usize {
        120
    }
}

pub struct TspDescriptor;
impl ProblemDescriptor for TspDescriptor {
    fn name(&self) -> &str {
        "TSP"
    }
    fn complexity_class(&self) -> ComplexityClass {
        ComplexityClass::NpHard
    }
    fn scaling_exponent(&self) -> f64 {
        0.5
    }
    fn verification_cost(&self) -> VerificationCost {
        VerificationCost::Linear
    }
    fn size_ratio(&self) -> f64 {
        0.35
    }
    fn base_difficulty_weight(&self) -> f64 {
        1.5
    }
    fn has_quality_gradient(&self) -> bool {
        true
    }
    fn absolute_max_size(&self) -> usize {
        30
    }
}

pub struct GraphColoringDescriptor;
impl ProblemDescriptor for GraphColoringDescriptor {
    fn name(&self) -> &str {
        "GraphColoring"
    }
    fn complexity_class(&self) -> ComplexityClass {
        ComplexityClass::NpComplete
    }
    fn scaling_exponent(&self) -> f64 {
        0.65
    }
    fn verification_cost(&self) -> VerificationCost {
        VerificationCost::GraphLinear
    }
    fn size_ratio(&self) -> f64 {
        0.6
    }
    fn base_difficulty_weight(&self) -> f64 {
        1.1
    }
    fn has_quality_gradient(&self) -> bool {
        true
    }
    fn absolute_max_size(&self) -> usize {
        80
    }
}

pub struct FactorizationDescriptor;
impl ProblemDescriptor for FactorizationDescriptor {
    fn name(&self) -> &str {
        "Factorization"
    }
    fn complexity_class(&self) -> ComplexityClass {
        ComplexityClass::NpHard
    }
    fn scaling_exponent(&self) -> f64 {
        0.33
    }
    fn verification_cost(&self) -> VerificationCost {
        VerificationCost::Constant
    }
    fn size_ratio(&self) -> f64 {
        0.5
    }
    fn base_difficulty_weight(&self) -> f64 {
        1.8
    }
    fn has_quality_gradient(&self) -> bool {
        false
    }
    fn absolute_max_size(&self) -> usize {
        200
    }
    fn absolute_min_size(&self) -> usize {
        16
    }
}

pub struct SvpDescriptor;
impl ProblemDescriptor for SvpDescriptor {
    fn name(&self) -> &str {
        "SVP"
    }
    fn complexity_class(&self) -> ComplexityClass {
        ComplexityClass::NpHard
    }
    fn scaling_exponent(&self) -> f64 {
        0.5
    }
    fn verification_cost(&self) -> VerificationCost {
        VerificationCost::Constant
    }
    fn size_ratio(&self) -> f64 {
        0.4
    }
    fn base_difficulty_weight(&self) -> f64 {
        2.0
    }
    fn has_quality_gradient(&self) -> bool {
        true
    }
    fn absolute_max_size(&self) -> usize {
        40
    }
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

pub struct ProblemRegistry {
    descriptors: HashMap<String, Arc<dyn ProblemDescriptor>>,
}

impl Default for ProblemRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ProblemRegistry {
    /// Create registry pre-loaded with built-in problem types.
    pub fn new() -> Self {
        let mut registry = ProblemRegistry {
            descriptors: HashMap::new(),
        };
        registry.register(Arc::new(SubsetSumDescriptor));
        registry.register(Arc::new(SatDescriptor));
        registry.register(Arc::new(TspDescriptor));
        registry.register(Arc::new(GraphColoringDescriptor));
        registry.register(Arc::new(FactorizationDescriptor));
        registry.register(Arc::new(SvpDescriptor));
        registry
    }

    pub fn empty() -> Self {
        ProblemRegistry {
            descriptors: HashMap::new(),
        }
    }

    pub fn register(&mut self, descriptor: Arc<dyn ProblemDescriptor>) {
        self.descriptors
            .insert(descriptor.name().to_string(), descriptor);
    }

    pub fn get(&self, name: &str) -> Option<&Arc<dyn ProblemDescriptor>> {
        self.descriptors.get(name)
    }

    pub fn problem_types(&self) -> Vec<&str> {
        self.descriptors.keys().map(|s| s.as_str()).collect()
    }

    pub fn len(&self) -> usize {
        self.descriptors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.descriptors.is_empty()
    }
}

pub type SharedRegistry = Arc<RwLock<ProblemRegistry>>;

pub fn default_registry() -> SharedRegistry {
    Arc::new(RwLock::new(ProblemRegistry::new()))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_has_builtins() {
        let registry = ProblemRegistry::new();
        assert!(registry.get("SubsetSum").is_some());
        assert!(registry.get("SAT").is_some());
        assert!(registry.get("TSP").is_some());
        assert!(registry.get("GraphColoring").is_some());
        assert!(registry.get("Factorization").is_some());
        assert!(registry.get("SVP").is_some());
        assert_eq!(registry.len(), 6);
    }

    #[test]
    fn test_register_custom_problem() {
        struct CustomProblem;
        impl ProblemDescriptor for CustomProblem {
            fn name(&self) -> &str {
                "CustomNP"
            }
            fn complexity_class(&self) -> ComplexityClass {
                ComplexityClass::NpComplete
            }
            fn scaling_exponent(&self) -> f64 {
                0.6
            }
            fn verification_cost(&self) -> VerificationCost {
                VerificationCost::Linear
            }
            fn size_ratio(&self) -> f64 {
                0.5
            }
            fn base_difficulty_weight(&self) -> f64 {
                1.3
            }
            fn has_quality_gradient(&self) -> bool {
                false
            }
            fn absolute_max_size(&self) -> usize {
                50
            }
        }

        let mut registry = ProblemRegistry::new();
        registry.register(Arc::new(CustomProblem));
        assert!(registry.get("CustomNP").is_some());
        assert_eq!(registry.len(), 7);
    }

    #[test]
    fn test_scaling_exponents_ordered() {
        let registry = ProblemRegistry::new();
        let tsp = registry.get("TSP").unwrap().scaling_exponent();
        let sat = registry.get("SAT").unwrap().scaling_exponent();
        let ss = registry.get("SubsetSum").unwrap().scaling_exponent();
        assert!(tsp <= sat, "TSP should scale harder than SAT");
        assert!(sat <= ss, "SAT should scale harder than SubsetSum");
    }

    #[test]
    fn test_verification_cost_scaling() {
        assert_eq!(VerificationCost::Constant.scaling_factor(100), 1.0);
        assert_eq!(VerificationCost::Linear.scaling_factor(100), 100.0);
        assert!(
            (VerificationCost::Polynomial(2.0).scaling_factor(10) - 100.0).abs() < f64::EPSILON
        );
    }
}
