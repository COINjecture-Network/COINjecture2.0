// Genesis Block Creation
// Hard-coded initial blockchain state
#![allow(dead_code)]

use coinject_core::{
    Address, Block, BlockHeader, CoinbaseTransaction, Commitment, Hash, ProblemType, Solution,
    SolutionReveal,
};
use sha2::{Digest, Sha256};

/// Genesis block configuration
pub struct GenesisConfig {
    pub genesis_address: Address,
    pub initial_supply: u128,
}

impl Default for GenesisConfig {
    fn default() -> Self {
        // Genesis address from genesis_wallet.json
        // Derive address from public key using SHA256 (same as keystore.rs)
        // Public key: df52ac77a92607b348f742aa3542a3f4e72c7dff49c07819d98b459111979090
        // Description: Genesis Wallet - Controls D₈ (Foundation Endowment, 8.2% normalized)
        // Compile-time-constant hex string — decode is always valid and length always 32.
        // The expect messages here are intentionally developer-facing: if this fails, the
        // genesis key constant in source code is wrong and must be corrected before shipping.
        let public_key_hex = "df52ac77a92607b348f742aa3542a3f4e72c7dff49c07819d98b459111979090";
        let public_key_bytes = hex::decode(public_key_hex).expect(
            "BUG: genesis public key hex constant is malformed — fix the constant in genesis.rs",
        );

        assert_eq!(
            public_key_bytes.len(), 32,
            "BUG: genesis public key constant decoded to {} bytes, expected 32 — fix the constant in genesis.rs",
            public_key_bytes.len()
        );

        // Derive address using SHA256 (same as wallet/src/keystore.rs derive_address)
        let mut hasher = Sha256::new();
        hasher.update(&public_key_bytes);
        let address_hash = hasher.finalize();

        let mut addr_bytes = [0u8; 32];
        addr_bytes.copy_from_slice(&address_hash[..32]);

        GenesisConfig {
            genesis_address: Address::from_bytes(addr_bytes),
            initial_supply: 0, // Zero initial supply - tokens created through mining rewards only
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
        // Genesis block has nominal PoUW metrics (trivially solvable)
        solve_time_us: 1,
        verify_time_us: 1,
        time_asymmetry_ratio: 1.0,
        solution_quality: 1.0,         // Perfect solution
        complexity_weight: 1.0,        // Minimal complexity
        energy_estimate_joules: 0.001, // Negligible energy
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

/// Hard-coded genesis block hash for network identification.
///
/// This hash is computed **once** from the canonical genesis parameters and
/// serves as the network's unique identifier. A node that produces a different
/// genesis hash is on an incompatible chain.
pub fn genesis_hash() -> Hash {
    let genesis = create_genesis_block(GenesisConfig::default());
    genesis.header.hash()
}

/// Verify a block is the valid genesis block.
///
/// Performs full structural and cryptographic validation:
/// 1. Height must be 0.
/// 2. `prev_hash` must be `Hash::ZERO`.
/// 3. No user transactions (only coinbase).
/// 4. Solution must correctly solve the genesis problem.
/// 5. Commitment must verify against the deterministic genesis epoch salt.
/// 6. Block hash must match the canonical genesis hash (prevents genesis
///    replacement attacks — an attacker cannot substitute a different genesis
///    block with the same structure).
///
/// The hash check in step 6 is the critical guard: even if all structural
/// checks pass, a different genesis block (e.g., with a modified address or
/// initial supply) will have a different hash and be rejected.
pub fn is_valid_genesis(block: &Block) -> bool {
    // 1. Must be height 0.
    if block.header.height != 0 {
        return false;
    }

    // 2. Must have zero prev_hash.
    if block.header.prev_hash != Hash::ZERO {
        return false;
    }

    // 3. Must have no user transactions (only coinbase).
    if !block.transactions.is_empty() {
        return false;
    }

    // 4. Verify the solution solves the genesis problem.
    if !block
        .solution_reveal
        .solution
        .verify(&block.solution_reveal.problem)
    {
        return false;
    }

    // 5. Verify the commitment against the deterministic genesis epoch salt.
    let epoch_salt = Hash::new(b"coinject-genesis-epoch");
    if !block.solution_reveal.commitment.verify(
        &block.solution_reveal.problem,
        &block.solution_reveal.solution,
        &epoch_salt,
    ) {
        return false;
    }

    // 6. Hash must match canonical genesis (prevents genesis replacement attacks).
    //    An attacker who modifies any field (address, supply, problem, timestamp)
    //    will produce a different hash and fail here.
    let canonical_hash = genesis_hash();
    if block.header.hash() != canonical_hash {
        return false;
    }

    true
}

/// Check whether a block claims to be genesis but is NOT the canonical one.
///
/// Returns `true` if the block is at height 0 but has a different hash than
/// the canonical genesis. This detects genesis-replacement attacks where
/// an attacker tries to substitute a crafted block at height 0.
pub fn is_genesis_attack(block: &Block) -> bool {
    block.header.height == 0 && block.header.hash() != genesis_hash()
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
        assert!(genesis
            .solution_reveal
            .solution
            .verify(&genesis.solution_reveal.problem));
    }

    #[test]
    fn test_genesis_hash_deterministic() {
        let hash1 = genesis_hash();
        let hash2 = genesis_hash();

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_invalid_genesis_wrong_height() {
        let mut genesis = create_genesis_block(GenesisConfig::default());
        genesis.header.height = 1;
        assert!(!is_valid_genesis(&genesis));
    }

    #[test]
    fn test_invalid_genesis_wrong_prev_hash() {
        let mut genesis = create_genesis_block(GenesisConfig::default());
        genesis.header.prev_hash = Hash::new(b"not-zero");
        assert!(!is_valid_genesis(&genesis));
    }

    #[test]
    fn test_genesis_attack_detection() {
        let mut fake = create_genesis_block(GenesisConfig::default());
        // Tamper with the miner address — this changes the block hash.
        fake.header.miner = coinject_core::Address::from_bytes([0xAB; 32]);
        assert!(
            is_genesis_attack(&fake),
            "tampered genesis should be detected"
        );
        assert!(
            !is_valid_genesis(&fake),
            "tampered genesis must not validate"
        );
    }

    #[test]
    fn test_is_genesis_attack_canonical_returns_false() {
        let canonical = create_genesis_block(GenesisConfig::default());
        // Canonical genesis is NOT an attack.
        assert!(!is_genesis_attack(&canonical));
    }
}
