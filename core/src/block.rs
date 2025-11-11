use crate::{
    Address, BlockHeight, CoinbaseTransaction, Commitment, Hash, SolutionReveal, Timestamp,
    Transaction, WorkScore,
};
use serde::{Deserialize, Serialize};

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
}

impl BlockHeader {
    /// Calculate header hash
    pub fn hash(&self) -> Hash {
        let serialized = bincode::serialize(self).unwrap_or_default();
        Hash::new(&serialized)
    }

    /// Check if header meets difficulty target
    pub fn meets_difficulty(&self, target: &Hash) -> bool {
        let hash = self.hash();
        hash.as_bytes() < target.as_bytes()
    }

    /// Epoch salt derived from parent hash (prevents pre-mining)
    pub fn epoch_salt(&self) -> Hash {
        self.prev_hash
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
        self.blocks.last().unwrap()
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
