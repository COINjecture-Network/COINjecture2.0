// COINjecture Consensus Engine
// Work score calculation, difficulty adjustment, and epoch coordination

// BlockProduced carries a full Block by value — boxing would require updating every match
// arm across the codebase for marginal stack benefit.
#![allow(clippy::large_enum_variant)]
// Continuation lines in doc comments are intentionally aligned for readability.
#![allow(clippy::doc_overindented_list_items)]
// Miner uses index-based DP algorithms (SubsetSum, SAT, TSP) where loop index is used
// both for array access and arithmetic (e.g. bit-shifting), making enumerate refactors verbose.
#![allow(clippy::needless_range_loop)]
// build_block_from_solution and SAT solver functions are public APIs with many required params.
#![allow(clippy::too_many_arguments)]

pub mod coordinator;
pub mod miner;
pub mod problem_registry;
pub mod work_score;

pub use miner::build_block_from_solution;
pub use miner::*;
pub use problem_registry::{
    default_registry, ComplexityClass, ProblemDescriptor, ProblemRegistry, SharedRegistry,
    VerificationCost,
};
pub use work_score::*;

pub mod difficulty;
pub use difficulty::*;

// Coordinator exports
pub use coordinator::{
    CommitCollector, CoordinatorCommand, CoordinatorConfig, CoordinatorEvent, CoordinatorState,
    EpochCoordinator, EpochPhase, EpochState, SolutionCommit,
};
