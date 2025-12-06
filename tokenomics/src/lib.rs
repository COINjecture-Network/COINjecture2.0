// =============================================================================
// COINjecture Tokenomics
// η = λ = 1/√2 Exponential Dimensional Distribution
// =============================================================================
//
// All tokenomics parameters are either:
// - A direct function of η = λ = 1/√2
// - Derived from the dimensional scales D_n = e^(-τn/√2)
// - Measured against network consensus state |ψ(t)|
// - Bounded by the Viviani oracle Δ ≤ 0.3
//
// This creates a SELF-STABILIZING economic system where market forces
// naturally converge to the same critical equilibrium that governs consensus.

pub mod dimensions;
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
pub use rewards::*;
pub use distributor::*;
pub use pools::*;
pub use emission::*;
pub use staking::*;
pub use bounty_pricing::*;
pub use deflation::*;
pub use amm::*;
pub use governance::*;
