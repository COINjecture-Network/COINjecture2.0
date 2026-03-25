use crate::{
    Address, BlockHeight, CoinbaseTransaction, Commitment, Hash, SolutionReveal, Timestamp,
    Transaction, WorkScore,
};
use serde::{Deserialize, Serialize};

// =============================================================================
// Block Version Constants
// =============================================================================

/// Block version 1: Standard hashing (original implementation)
/// - Commitments: H(problem || salt || H(solution))
/// - Merkle tree: H(left || right)
/// - MMR: H("MMR_NODE" || height || left || right)
pub const BLOCK_VERSION_STANDARD: u32 = 1;

/// Block version 2: GoldenSeed-enhanced hashing
/// - Commitments: H(problem || salt || golden_stream || H(solution))
/// - Merkle tree: H("MERKLE_NODE" || golden_key || level || left || right)
/// - MMR: H("MMR_NODE" || height || golden_key || left || right)
///
/// Golden streams are derived from handshake-established genesis_hash.
/// See: GoldenSeed Merkle Tree Integration Design Plan
pub const BLOCK_VERSION_GOLDEN: u32 = 2;

/// Block header (mined with commitment)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockHeader {
    /// Block version
    pub version: u32,
    /// Height in the chain
    pub height: BlockHeight,
    /// Hash of previous block
    pub prev_hash: Hash,
    /// Timestamp (Unix epoch)
    pub timestamp: Timestamp,
    /// Merkle root of transactions
    pub transactions_root: Hash,
    /// Merkle root of solutions (prunable)
    pub solutions_root: Hash,
    /// Commitment to NP-hard problem solution
    pub commitment: Commitment,
    /// Work score for this block
    pub work_score: WorkScore,
    /// Miner's address
    pub miner: Address,
    /// Nonce for header mining
    pub nonce: u64,

    // PoUW Transparency Metrics (WEB4)
    /// Time to find solution (microseconds)
    pub solve_time_us: u64,
    /// Time to verify solution (microseconds) - should be fast!
    pub verify_time_us: u64,
    /// Time asymmetry ratio (solve_time / verify_time) - proves useful work
    pub time_asymmetry_ratio: f64,
    /// Solution quality (0.0 to 1.0) - optimality measure
    pub solution_quality: f64,
    /// Problem complexity weight - difficulty measure
    pub complexity_weight: f64,
    /// Estimated energy consumption (Joules) - transparency metric
    pub energy_estimate_joules: f64,
}

impl BlockHeader {
    /// Calculate header hash using bincode serialization (server-side)
    pub fn hash(&self) -> Hash {
        let serialized = bincode::serialize(self).unwrap_or_default();
        Hash::new(&serialized)
    }

    /// Calculate header hash using JSON serialization (client-side compatibility)
    /// This enables web-based miners to submit blocks without needing bincode
    pub fn hash_from_json(&self) -> Hash {
        let serialized = serde_json::to_vec(self).unwrap_or_default();
        Hash::new(&serialized)
    }

    /// Check if header meets difficulty target (tries both bincode and JSON)
    pub fn meets_difficulty(&self, target: &Hash) -> bool {
        // Try bincode hash first (server-side mining)
        let hash_bincode = self.hash();
        if hash_bincode.as_bytes() < target.as_bytes() {
            return true;
        }

        // Try JSON hash (client-side mining from web browsers)
        let hash_json = self.hash_from_json();
        hash_json.as_bytes() < target.as_bytes()
    }

    /// Epoch salt derived from parent hash (prevents pre-mining)
    pub fn epoch_salt(&self) -> Hash {
        self.prev_hash
    }

    // =========================================================================
    // GoldenSeed Enhancement Methods
    // =========================================================================

    /// Check if this block uses golden-enhanced hashing
    ///
    /// Returns true if block version >= BLOCK_VERSION_GOLDEN (2).
    /// Golden-enhanced blocks use:
    /// - Enhanced commitments with golden stream
    /// - Enhanced merkle trees with golden keys
    /// - Enhanced MMR with golden keys
    #[inline]
    pub fn uses_golden_enhancements(&self) -> bool {
        self.version >= BLOCK_VERSION_GOLDEN
    }

    /// Check if this block uses standard (v1) hashing
    #[inline]
    pub fn uses_standard_hashing(&self) -> bool {
        self.version < BLOCK_VERSION_GOLDEN
    }
}

/// Complete block with header, transactions, and solution reveal
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Block {
    /// Block header (includes commitment)
    pub header: BlockHeader,
    /// Coinbase transaction (block reward)
    pub coinbase: CoinbaseTransaction,
    /// Regular transactions
    pub transactions: Vec<Transaction>,
    /// Solution reveal (broadcast after header)
    pub solution_reveal: SolutionReveal,
}

impl Block {
    /// Create genesis block
    pub fn genesis(genesis_address: Address) -> Self {
        let commitment = Commitment {
            hash: Hash::ZERO,
            problem_hash: Hash::ZERO,
        };

        let header = BlockHeader {
            version: 1,
            height: 0,
            prev_hash: Hash::ZERO,
            timestamp: 0,
            transactions_root: Hash::ZERO,
            solutions_root: Hash::ZERO,
            commitment,
            work_score: 0.0,
            miner: genesis_address,
            nonce: 0,
            // Genesis block has no PoUW metrics (no mining required)
            solve_time_us: 0,
            verify_time_us: 0,
            time_asymmetry_ratio: 0.0,
            solution_quality: 0.0,
            complexity_weight: 0.0,
            energy_estimate_joules: 0.0,
        };

        let coinbase = CoinbaseTransaction::new(genesis_address, 0, 0);

        // Genesis has no solution reveal (placeholder)
        let solution_reveal = SolutionReveal {
            problem: crate::problem::ProblemType::Custom {
                problem_id: Hash::ZERO,
                data: vec![],
            },
            solution: crate::problem::Solution::Custom(vec![]),
            commitment: Commitment {
                hash: Hash::ZERO,
                problem_hash: Hash::ZERO,
            },
        };

        Block {
            header,
            coinbase,
            transactions: vec![],
            solution_reveal,
        }
    }

    /// Calculate block hash (just header hash)
    pub fn hash(&self) -> Hash {
        self.header.hash()
    }

    /// Verify block validity
    pub fn verify(&self) -> bool {
        // 1. Verify solution reveal matches commitment
        if !self
            .solution_reveal
            .verify(&self.header.epoch_salt())
        {
            return false;
        }

        // 2. Verify all transactions
        if !self.transactions.iter().all(|tx| tx.is_valid()) {
            return false;
        }

        // 3. Verify coinbase height matches header
        if self.coinbase.height != self.header.height {
            return false;
        }

        // 4. Verify transaction merkle root
        let tx_hashes: Vec<Vec<u8>> = self
            .transactions
            .iter()
            .map(|tx| tx.hash().to_vec())
            .collect();
        let tx_root = crate::crypto::MerkleTree::new(tx_hashes).root();
        if tx_root != self.header.transactions_root {
            return false;
        }

        true
    }

    /// Get total fees from transactions
    pub fn total_fees(&self) -> u128 {
        self.transactions.iter().map(|tx| tx.fee()).sum()
    }
}

/// Blockchain state
pub struct Blockchain {
    /// All blocks (height -> block)
    pub blocks: Vec<Block>,
    /// Current difficulty target
    pub difficulty_target: Hash,
}

impl Blockchain {
    pub fn new(genesis_address: Address) -> Self {
        let genesis = Block::genesis(genesis_address);
        Blockchain {
            blocks: vec![genesis],
            difficulty_target: Hash::from_bytes([0xFF; 32]), // Easy target initially
        }
    }

    pub fn height(&self) -> BlockHeight {
        self.blocks.len() as BlockHeight - 1
    }

    pub fn tip(&self) -> &Block {
        self.blocks.last().expect("blockchain invariant: chain always contains at least the genesis block")
    }

    pub fn get_block(&self, height: BlockHeight) -> Option<&Block> {
        self.blocks.get(height as usize)
    }

    /// Add block to chain (assumes validation already done)
    pub fn add_block(&mut self, block: Block) {
        self.blocks.push(block);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_genesis_block() {
        let genesis_addr = Address::from_bytes([0u8; 32]);
        let blockchain = Blockchain::new(genesis_addr);

        assert_eq!(blockchain.height(), 0);
        assert_eq!(blockchain.tip().header.height, 0);
        assert_eq!(blockchain.tip().header.prev_hash, Hash::ZERO);
    }
}
