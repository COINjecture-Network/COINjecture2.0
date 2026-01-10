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
pub mod network_metrics;  // NEW: Central oracle for network-derived values
pub mod rewards;
pub mod distributor;

// Advanced tokenomics modules
pub mod pools;
pub mod emission;
pub mod staking;
pub mod bounty_pricing;
pub mod deflation;
pub mod amm;
pub mod governance;

// Re-exports
pub use dimensions::*;
pub use network_metrics::*;  // Export NetworkMetrics oracle
pub use rewards::*;
pub use distributor::*;
pub use pools::*;
pub use emission::*;
pub use staking::*;
pub use bounty_pricing::*;
pub use deflation::*;
pub use amm::*;
pub use governance::*;

// Re-export dimensionless constants from core (single source of truth)
// Note: These are re-exported from core via `pub use dimensional::*;`
pub use coinject_core::{ETA, LAMBDA, TAU_C};
pub use coinject_core::golden::PHI_INV; // Canonical source
