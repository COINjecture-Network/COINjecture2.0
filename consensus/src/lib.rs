// COINjecture Consensus Engine
// Work score calculation, difficulty adjustment, and epoch coordination

pub mod work_score;
pub mod miner;
pub mod coordinator;

pub use work_score::*;
pub use miner::*;
pub use miner::build_block_from_solution;

pub mod difficulty;
pub use difficulty::*;

// Coordinator exports
pub use coordinator::{
    EpochCoordinator, CoordinatorConfig, CoordinatorEvent, CoordinatorCommand,
    EpochPhase, EpochState, CommitCollector, SolutionCommit, CoordinatorState,
};
