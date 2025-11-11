// Genesis Block Creation
// Hard-coded initial blockchain state

use coinject_core::{
    Address, Block, BlockHeader, CoinbaseTransaction, Commitment, Hash, ProblemType, Solution,
    SolutionReveal,
};

/// Genesis block configuration
pub struct GenesisConfig {
    pub genesis_address: Address,
    pub initial_supply: u128,
}

impl Default for GenesisConfig {
    fn default() -> Self {
        // Default genesis address (all zeros except last byte = 1)
        let mut addr_bytes = [0u8; 32];
        addr_bytes[31] = 1;

        GenesisConfig {
            genesis_address: Address::from_bytes(addr_bytes),
            initial_supply: 21_000_000_000_000_000, // 21 million BEANS with 9 decimals
        }
    }
}

/// Create the genesis block for Network B
pub fn create_genesis_block(config: GenesisConfig) -> Block {
    // Genesis problem: Simple SubsetSum that's trivially solvable
    let problem = ProblemType::SubsetSum {
        numbers: vec![1, 2, 3, 4, 5],
        target: 9,
    };

    // Genesis solution: [2, 3, 4] = 2 + 3 + 4 = 9
    let solution = Solution::SubsetSum(vec![1, 2, 3]);

    // Genesis commitment (deterministic)
    let epoch_salt = Hash::new(b"coinject-genesis-epoch");
    let commitment = Commitment::create(&problem, &solution, &epoch_salt);

    // Genesis timestamp: January 1, 2025 00:00:00 UTC
    let genesis_timestamp = 1735689600i64;

    // Genesis block header
    let header = BlockHeader {
        version: 1,
        height: 0,
        prev_hash: Hash::ZERO,
        timestamp: genesis_timestamp,
        transactions_root: Hash::ZERO,
        solutions_root: Hash::new(&bincode::serialize(&solution).unwrap_or_default()),
        commitment: commitment.clone(),
        work_score: 1.0, // Genesis has minimal work score
        miner: config.genesis_address,
        nonce: 0,
    };

    // Genesis coinbase: Issue initial supply
    let coinbase = CoinbaseTransaction::new(
        config.genesis_address,
        config.initial_supply,
        0, // height 0
    );

    // Genesis solution reveal
    let solution_reveal = SolutionReveal {
        problem,
        solution,
        commitment,
    };

    Block {
        header,
        coinbase,
        transactions: vec![],
        solution_reveal,
    }
}

/// Hard-coded genesis block hash for network identification
pub fn genesis_hash() -> Hash {
    // This will be computed from the actual genesis block
    // For now, return a deterministic value
    let genesis = create_genesis_block(GenesisConfig::default());
    genesis.header.hash()
}

/// Verify a block is the valid genesis block
pub fn is_valid_genesis(block: &Block) -> bool {
    // Must be height 0
    if block.header.height != 0 {
        return false;
    }

    // Must have zero prev_hash
    if block.header.prev_hash != Hash::ZERO {
        return false;
    }

    // Must have no transactions (only coinbase)
    if !block.transactions.is_empty() {
        return false;
    }

    // Verify the solution
    if !block.solution_reveal.solution.verify(&block.solution_reveal.problem) {
        return false;
    }

    // Verify the commitment
    let epoch_salt = Hash::new(b"coinject-genesis-epoch");
    if !block
        .solution_reveal
        .commitment
        .verify(&block.solution_reveal.problem, &block.solution_reveal.solution, &epoch_salt)
    {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_genesis_creation() {
        let genesis = create_genesis_block(GenesisConfig::default());

        assert_eq!(genesis.header.height, 0);
        assert_eq!(genesis.header.prev_hash, Hash::ZERO);
        assert!(genesis.transactions.is_empty());
        assert!(is_valid_genesis(&genesis));
    }

    #[test]
    fn test_genesis_solution() {
        let genesis = create_genesis_block(GenesisConfig::default());

        // Verify the solution is correct
        assert!(genesis.solution_reveal.solution.verify(&genesis.solution_reveal.problem));
    }

    #[test]
    fn test_genesis_hash_deterministic() {
        let hash1 = genesis_hash();
        let hash2 = genesis_hash();

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_invalid_genesis() {
        let mut genesis = create_genesis_block(GenesisConfig::default());

        // Corrupt the height
        genesis.header.height = 1;
        assert!(!is_valid_genesis(&genesis));
    }
}
