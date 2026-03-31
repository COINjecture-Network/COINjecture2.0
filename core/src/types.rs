use serde::{Deserialize, Serialize};
use std::fmt;

/// 256-bit cryptographic hash using Blake3
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Hash([u8; 32]);

impl Hash {
    pub const ZERO: Hash = Hash([0u8; 32]);

    pub fn new(data: &[u8]) -> Self {
        let hash = blake3::hash(data);
        Hash(*hash.as_bytes())
    }

    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Hash(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_vec(&self) -> Vec<u8> {
        self.0.to_vec()
    }
}

impl fmt::Display for Hash {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", hex::encode(self.0))
    }
}

impl fmt::Debug for Hash {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Hash({})", hex::encode(&self.0[..8]))
    }
}

/// Account address (32 bytes derived from public key)
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Address([u8; 32]);

impl Address {
    pub fn from_pubkey(pubkey: &[u8; 32]) -> Self {
        Address(*pubkey)
    }

    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Address(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_base58(&self) -> String {
        bs58::encode(&self.0).into_string()
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", bs58::encode(&self.0).into_string())
    }
}

impl fmt::Debug for Address {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Address({}...)", &self.to_base58()[..8])
    }
}

/// Account balance (u128 for large supply)
pub type Balance = u128;

/// Block height (64-bit unsigned integer)
pub type BlockHeight = u64;

/// Timestamp (Unix epoch in seconds)
pub type Timestamp = i64;

/// Work score (dimensionless ratio)
pub type WorkScore = f64;
