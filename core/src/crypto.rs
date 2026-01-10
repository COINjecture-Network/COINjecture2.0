use crate::{golden::GoldenGenerator, Address, Hash};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};

/// Domain separator for golden-enhanced merkle node hashing
const MERKLE_NODE_DOMAIN: &[u8] = b"MERKLE_NODE";

/// Ed25519 key pair for signing transactions
pub struct KeyPair {
    signing_key: SigningKey,
}

impl KeyPair {
    pub fn generate() -> Self {
        let mut csprng = rand::thread_rng();
        let signing_key = SigningKey::generate(&mut csprng);
        KeyPair { signing_key }
    }

    pub fn sign(&self, message: &[u8]) -> Ed25519Signature {
        let signature = self.signing_key.sign(message);
        Ed25519Signature(signature.to_bytes())
    }

    pub fn public_key(&self) -> PublicKey {
        PublicKey(self.signing_key.verifying_key().to_bytes())
    }

    pub fn address(&self) -> Address {
        Address::from_pubkey(&self.public_key().0)
    }
}

/// Public key (32 bytes)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicKey([u8; 32]);

impl PublicKey {
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        PublicKey(bytes)
    }

    pub fn verify(&self, message: &[u8], signature: &Ed25519Signature) -> bool {
        if let Ok(verifying_key) = VerifyingKey::from_bytes(&self.0) {
            let sig = Signature::from_bytes(&signature.0);
            return verifying_key.verify(message, &sig).is_ok();
        }
        false
    }

    pub fn to_address(&self) -> Address {
        Address::from_pubkey(&self.0)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Ed25519 signature (64 bytes)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ed25519Signature([u8; 64]);

impl Ed25519Signature {
    pub fn from_bytes(bytes: [u8; 64]) -> Self {
        Ed25519Signature(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 64] {
        &self.0
    }
}

// Custom serde for [u8; 64] (serde only supports up to [u8; 32] by default)
impl serde::Serialize for Ed25519Signature {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_bytes(&self.0)
    }
}

impl<'de> serde::Deserialize<'de> for Ed25519Signature {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let bytes: Vec<u8> = serde::Deserialize::deserialize(deserializer)?;
        if bytes.len() != 64 {
            return Err(serde::de::Error::custom("Expected 64 bytes for signature"));
        }
        let mut arr = [0u8; 64];
        arr.copy_from_slice(&bytes);
        Ok(Ed25519Signature(arr))
    }
}

/// Merkle tree for transaction/solution compaction
pub struct MerkleTree {
    #[allow(dead_code)]
    leaves: Vec<Hash>,
    root: Hash,
}

impl MerkleTree {
    pub fn new(data: Vec<Vec<u8>>) -> Self {
        let leaves: Vec<Hash> = data.iter().map(|d| Hash::new(d)).collect();
        let root = Self::calculate_root(&leaves);
        MerkleTree { leaves, root }
    }

    fn calculate_root(leaves: &[Hash]) -> Hash {
        if leaves.is_empty() {
            return Hash::ZERO;
        }
        if leaves.len() == 1 {
            return leaves[0];
        }

        let mut current_level = leaves.to_vec();
        while current_level.len() > 1 {
            let mut next_level = Vec::new();
            for chunk in current_level.chunks(2) {
                let combined = if chunk.len() == 2 {
                    let mut combined = Vec::new();
                    combined.extend_from_slice(chunk[0].as_bytes());
                    combined.extend_from_slice(chunk[1].as_bytes());
                    combined
                } else {
                    chunk[0].to_vec()
                };
                next_level.push(Hash::new(&combined));
            }
            current_level = next_level;
        }
        current_level[0]
    }

    pub fn root(&self) -> Hash {
        self.root
    }

    // =========================================================================
    // GoldenSeed-Enhanced Merkle Tree Methods
    // =========================================================================
    // These methods integrate golden ratio streams derived from the handshake
    // genesis_hash for enhanced self-referential properties.
    // See: GoldenSeed Merkle Tree Integration Design Plan

    /// Create merkle tree with golden-enhanced node hashing
    ///
    /// Node hash: H("MERKLE_NODE" || golden_key || level || left || right)
    ///
    /// The golden_key is derived from the handshake-established genesis_hash,
    /// ensuring all nodes produce identical merkle roots for the same inputs.
    ///
    /// # Arguments
    /// * `data` - Raw data to include in the merkle tree
    /// * `genesis_hash` - Genesis hash from handshake (seed foundation)
    /// * `epoch` - Epoch number (typically `block_height / 100`)
    pub fn new_with_golden(data: Vec<Vec<u8>>, genesis_hash: &Hash, epoch: u64) -> Self {
        let leaves: Vec<Hash> = data.iter().map(|d| Hash::new(d)).collect();
        let root = Self::calculate_root_with_golden(&leaves, genesis_hash, epoch);
        MerkleTree { leaves, root }
    }

    /// Calculate merkle root with golden-enhanced hashing
    fn calculate_root_with_golden(leaves: &[Hash], genesis_hash: &Hash, epoch: u64) -> Hash {
        if leaves.is_empty() {
            return Hash::ZERO;
        }
        if leaves.len() == 1 {
            return leaves[0];
        }

        // Generate golden generator for this epoch
        // Use epoch * 100 as height to ensure same epoch calculation as from_genesis_epoch
        let mut golden_gen = GoldenGenerator::from_genesis_epoch(genesis_hash, epoch * 100);

        let mut current_level = leaves.to_vec();
        let mut level = 0u32;

        while current_level.len() > 1 {
            // Get golden key for this level
            let golden_key = golden_gen.next_bytes();
            let mut next_level = Vec::new();

            for chunk in current_level.chunks(2) {
                let combined = if chunk.len() == 2 {
                    // Enhanced hashing: H("MERKLE_NODE" || golden_key || level || left || right)
                    let mut hasher = blake3::Hasher::new();
                    hasher.update(MERKLE_NODE_DOMAIN);
                    hasher.update(&golden_key);
                    hasher.update(&level.to_le_bytes());
                    hasher.update(chunk[0].as_bytes());
                    hasher.update(chunk[1].as_bytes());
                    Hash::from_bytes(*hasher.finalize().as_bytes())
                } else {
                    // Odd leaf - pass through unchanged
                    chunk[0]
                };
                next_level.push(combined);
            }

            current_level = next_level;
            level += 1;
        }

        current_level[0]
    }

    /// Create merkle tree with golden-ordered leaves
    ///
    /// Uses `golden_sort_key(index)` to deterministically order leaves
    /// before building the tree. This provides consistent ordering across
    /// all nodes with better distribution properties.
    ///
    /// CONSENSUS-SAFE: Uses integer golden multiplication (no floats).
    /// See docs/GOLDEN_PHI_AUDIT.md for rationale.
    ///
    /// # Arguments
    /// * `data` - Raw data to include in the merkle tree
    /// * `genesis_hash` - Genesis hash from handshake (seed foundation)
    /// * `epoch` - Epoch number (typically `block_height / 100`)
    pub fn new_with_golden_ordering(data: Vec<Vec<u8>>, genesis_hash: &Hash, epoch: u64) -> Self {
        // Sort leaves by golden_sort_key(index) for deterministic ordering
        // Uses pure integer arithmetic - consensus safe across all platforms
        let mut indexed_data: Vec<(usize, Vec<u8>)> = data
            .into_iter()
            .enumerate()
            .collect();

        indexed_data.sort_by(|a, b| {
            let key_a = GoldenGenerator::golden_sort_key(a.0 as u64);
            let key_b = GoldenGenerator::golden_sort_key(b.0 as u64);
            // Primary sort by golden key, tie-break by original index
            key_a.cmp(&key_b).then_with(|| a.0.cmp(&b.0))
        });

        // Extract ordered data
        let ordered_data: Vec<Vec<u8>> = indexed_data
            .into_iter()
            .map(|(_, d)| d)
            .collect();

        // Build merkle tree with golden-enhanced hashing
        Self::new_with_golden(ordered_data, genesis_hash, epoch)
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merkle_tree_standard() {
        let data = vec![
            b"tx1".to_vec(),
            b"tx2".to_vec(),
            b"tx3".to_vec(),
            b"tx4".to_vec(),
        ];

        let tree = MerkleTree::new(data.clone());
        let root = tree.root();

        // Root should not be zero
        assert_ne!(root, Hash::ZERO);

        // Same data should produce same root
        let tree2 = MerkleTree::new(data);
        assert_eq!(tree.root(), tree2.root());
    }

    #[test]
    fn test_golden_merkle_deterministic() {
        let data = vec![
            b"tx1".to_vec(),
            b"tx2".to_vec(),
            b"tx3".to_vec(),
            b"tx4".to_vec(),
        ];
        let genesis = Hash::new(b"genesis_block");
        let epoch = 1;

        let tree1 = MerkleTree::new_with_golden(data.clone(), &genesis, epoch);
        let tree2 = MerkleTree::new_with_golden(data, &genesis, epoch);

        // Same inputs should produce same root
        assert_eq!(tree1.root(), tree2.root());
    }

    #[test]
    fn test_golden_vs_standard_different() {
        let data = vec![
            b"tx1".to_vec(),
            b"tx2".to_vec(),
            b"tx3".to_vec(),
            b"tx4".to_vec(),
        ];
        let genesis = Hash::new(b"genesis_block");
        let epoch = 1;

        let standard = MerkleTree::new(data.clone());
        let golden = MerkleTree::new_with_golden(data, &genesis, epoch);

        // Golden and standard should produce different roots
        assert_ne!(standard.root(), golden.root());
    }

    #[test]
    fn test_golden_merkle_epoch_sensitivity() {
        let data = vec![
            b"tx1".to_vec(),
            b"tx2".to_vec(),
        ];
        let genesis = Hash::new(b"genesis_block");

        let tree_epoch0 = MerkleTree::new_with_golden(data.clone(), &genesis, 0);
        let tree_epoch1 = MerkleTree::new_with_golden(data, &genesis, 1);

        // Different epochs should produce different roots
        assert_ne!(tree_epoch0.root(), tree_epoch1.root());
    }

    #[test]
    fn test_golden_merkle_genesis_sensitivity() {
        let data = vec![
            b"tx1".to_vec(),
            b"tx2".to_vec(),
        ];
        let genesis1 = Hash::new(b"genesis_block_1");
        let genesis2 = Hash::new(b"genesis_block_2");
        let epoch = 1;

        let tree1 = MerkleTree::new_with_golden(data.clone(), &genesis1, epoch);
        let tree2 = MerkleTree::new_with_golden(data, &genesis2, epoch);

        // Different genesis should produce different roots
        assert_ne!(tree1.root(), tree2.root());
    }

    #[test]
    fn test_golden_ordering_deterministic() {
        let data = vec![
            b"tx1".to_vec(),
            b"tx2".to_vec(),
            b"tx3".to_vec(),
            b"tx4".to_vec(),
        ];
        let genesis = Hash::new(b"genesis_block");
        let epoch = 1;

        let tree1 = MerkleTree::new_with_golden_ordering(data.clone(), &genesis, epoch);
        let tree2 = MerkleTree::new_with_golden_ordering(data, &genesis, epoch);

        // Same inputs should produce same root with ordering
        assert_eq!(tree1.root(), tree2.root());
    }

    #[test]
    fn test_golden_ordering_vs_golden_different() {
        let data = vec![
            b"tx1".to_vec(),
            b"tx2".to_vec(),
            b"tx3".to_vec(),
            b"tx4".to_vec(),
        ];
        let genesis = Hash::new(b"genesis_block");
        let epoch = 1;

        let ordered = MerkleTree::new_with_golden_ordering(data.clone(), &genesis, epoch);
        let unordered = MerkleTree::new_with_golden(data, &genesis, epoch);

        // Ordered and unordered should produce different roots
        // (unless by chance the ordering happens to match)
        assert_ne!(ordered.root(), unordered.root());
    }

    #[test]
    fn test_merkle_empty() {
        let empty: Vec<Vec<u8>> = vec![];
        let genesis = Hash::new(b"genesis_block");

        let standard = MerkleTree::new(empty.clone());
        let golden = MerkleTree::new_with_golden(empty, &genesis, 1);

        // Empty trees should return ZERO
        assert_eq!(standard.root(), Hash::ZERO);
        assert_eq!(golden.root(), Hash::ZERO);
    }

    #[test]
    fn test_merkle_single_leaf() {
        let data = vec![b"only_tx".to_vec()];
        let genesis = Hash::new(b"genesis_block");

        let standard = MerkleTree::new(data.clone());
        let golden = MerkleTree::new_with_golden(data, &genesis, 1);

        // Single leaf should return the leaf hash (same for both)
        assert_eq!(standard.root(), golden.root());
    }
}
