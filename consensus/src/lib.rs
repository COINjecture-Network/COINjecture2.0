// COINjecture Consensus Engine
// Work score calculation and difficulty adjustment

pub mod work_score;
pub mod miner;

pub use work_score::*;
pub use miner::*;

pub mod difficulty;
pub use difficulty::*;
