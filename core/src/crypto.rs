use crate::{Address, Hash};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};

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
}
