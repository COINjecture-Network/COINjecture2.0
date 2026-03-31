// Merkle Proof Utilities
// Standalone functions for building and verifying Merkle proofs
#![allow(dead_code)]

/// Build a Merkle proof for a transaction in a block
/// Returns the authentication path with direction flags
pub(crate) fn build_merkle_proof(
    transactions: &[coinject_core::Transaction],
    target_tx_hash: &coinject_core::Hash,
) -> Vec<(coinject_core::Hash, bool)> {
    use sha2::{Digest, Sha256};

    if transactions.is_empty() {
        return Vec::new();
    }

    // Get transaction hashes
    let mut leaves: Vec<coinject_core::Hash> = transactions.iter().map(|tx| tx.hash()).collect();

    // Find target index
    let target_index = match leaves.iter().position(|h| h == target_tx_hash) {
        Some(idx) => idx,
        None => return Vec::new(), // Transaction not found
    };

    // Build proof bottom-up
    let mut proof = Vec::new();
    let mut current_index = target_index;

    while leaves.len() > 1 {
        // Pad to even length
        if leaves.len() % 2 == 1 {
            leaves.push(*leaves.last().unwrap());
        }

        // Collect sibling
        let sibling_index = if current_index % 2 == 0 {
            current_index + 1
        } else {
            current_index - 1
        };

        let is_right = current_index % 2 == 0;
        proof.push((leaves[sibling_index], is_right));

        // Build next level
        let mut next_level = Vec::new();
        for i in (0..leaves.len()).step_by(2) {
            let left = &leaves[i];
            let right = &leaves[i + 1];

            let mut hasher = Sha256::new();
            hasher.update(b"MERKLE_NODE");
            hasher.update(left.as_bytes());
            hasher.update(right.as_bytes());
            next_level.push(coinject_core::Hash::from_bytes(hasher.finalize().into()));
        }

        leaves = next_level;
        current_index /= 2;
    }

    proof
}

/// Verify a Merkle proof against a root
pub(crate) fn verify_merkle_proof(
    tx_hash: &coinject_core::Hash,
    proof: &[(coinject_core::Hash, bool)],
    expected_root: &coinject_core::Hash,
) -> bool {
    use sha2::{Digest, Sha256};

    let mut current = *tx_hash;

    for (sibling, is_right) in proof {
        let mut hasher = Sha256::new();
        hasher.update(b"MERKLE_NODE");

        if *is_right {
            // Current is on the left, sibling is on the right
            hasher.update(current.as_bytes());
            hasher.update(sibling.as_bytes());
        } else {
            // Sibling is on the left, current is on the right
            hasher.update(sibling.as_bytes());
            hasher.update(current.as_bytes());
        }

        current = coinject_core::Hash::from_bytes(hasher.finalize().into());
    }

    &current == expected_root
}
