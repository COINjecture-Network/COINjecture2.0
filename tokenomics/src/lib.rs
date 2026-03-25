// =============================================================================
// COINjecture Tokenomics - EMPIRICAL VERSION
// η = λ = 1/√2 Exponential Dimensional Distribution
// =============================================================================
//
// DESIGN PRINCIPLES (Sarah's Compliance Requirements):
//
// 1. EMPIRICAL: All values derived from actual network data, not hardcoded
// 2. SELF-REFERENTIAL: System references its own state, not external constants
// 3. DIMENSIONLESS: Pure ratios without absolute units
//
// All tokenomics parameters are either:
// - A direct function of η = λ = 1/√2 (mathematical constant)
// - Derived from the dimensional scales D_n = e^(-τn/√2)
// - Measured against network consensus state |ψ(t)|
// - Computed from network_metrics oracle (medians, percentiles)
//
// This creates a SELF-STABILIZING economic system where:
// - The network decides its own limits through adaptive resilience
// - No arbitrary constants - all values are network-derived
// - Market forces naturally converge to critical equilibrium

pub mod dimensions;
pub mod distributor;
pub mod network_metrics; // NEW: Central oracle for network-derived values
pub mod rewards;

// Advanced tokenomics modules
pub mod amm;
pub mod bounty_pricing;
pub mod deflation;
pub mod emission;
pub mod governance;
pub mod pools;
pub mod staking;

// Re-exports
pub use amm::*;
pub use bounty_pricing::*;
pub use deflation::*;
pub use dimensions::*;
pub use distributor::*;
pub use emission::*;
pub use governance::*;
pub use network_metrics::*; // Export NetworkMetrics oracle
pub use pools::*;
pub use rewards::*;
pub use staking::*;

// Re-export dimensionless constants from core (single source of truth)
// Note: These are re-exported from core via `pub use dimensional::*;` and `pub use golden::*;`
pub use coinject_core::{ETA, LAMBDA, PHI, PHI_INV, TAU_C};
