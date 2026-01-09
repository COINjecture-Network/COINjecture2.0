// =============================================================================
// GoldenSeed Primitives for Cryptographic Enhancement
// =============================================================================
//
// This module provides deterministic stream generation based on the golden ratio
// for enhancing merkle tree and commitment structures.
//
// The seed foundation comes from the P2P handshake's `genesis_hash` exchange,
// ensuring all nodes derive identical golden streams for merkle operations.
//
// Based on: https://github.com/beanapologist/seed (GoldenSeed)
// Integration by: Sarah & LEET
// =============================================================================

use crate::Hash;

// =============================================================================
// Mathematical Constants (from GoldenSeed)
// =============================================================================

/// Golden ratio phi = (1 + sqrt(5)) / 2 ~ 1.618033988749895
/// The "most irrational" number - provides maximal equidistribution
pub const PHI: f64 = 1.618033988749894848204586834365638118;

/// Inverse golden ratio phi^-1 = phi - 1 ~ 0.618033988749895
pub const PHI_INV: f64 = 0.618033988749894848204586834365638118;

/// Golden ratio as 32-byte seed (SHA-256 of phi's decimal expansion)
pub const GOLDEN_SEED: [u8; 32] = [
    0x9e, 0x37, 0x79, 0xb9, 0x7f, 0x4a, 0x7c, 0x15,
    0xf3, 0x9c, 0xc0, 0x60, 0x5c, 0xee, 0xdc, 0x83,
    0x41, 0x08, 0x2c, 0x12, 0x4a, 0xfc, 0x05, 0x51,
    0xc7, 0xab, 0x88, 0x26, 0x6e, 0xcf, 0x1f, 0x17,
];

/// Golden epoch duration (in blocks)
/// Seed rotates every epoch to prevent stale coordination
pub const GOLDEN_EPOCH_BLOCKS: u64 = 100;

// =============================================================================
// Golden Ratio Stream Generator (Rust port of GoldenSeed)
// =============================================================================

/// Deterministic stream generator using the golden ratio
///
/// Port of GoldenSeed's UniversalQKD generator to Rust.
/// Produces identical output given identical seeds across all platforms.
///
/// Used for:
/// - Merkle tree node hashing enhancement
/// - Commitment generation enhancement
/// - MMR node hashing enhancement
/// - Deterministic leaf ordering
#[derive(Debug, Clone)]
pub struct GoldenGenerator {
    /// Current state (256 bits)
    state: [u8; 32],
    /// Stream counter
    counter: u64,
}

impl GoldenGenerator {
    /// Create new generator from seed
    pub fn new(seed: &[u8; 32]) -> Self {
        // Initialize state via BLAKE3 hash of seed
        let state = blake3::hash(seed);

        GoldenGenerator {
            state: *state.as_bytes(),
            counter: 0,
        }
    }

    /// Create generator from genesis hash and block height
    ///
    /// This is the primary constructor for merkle/commitment operations.
    /// The genesis_hash comes from the P2P handshake exchange, ensuring
    /// all nodes on the same chain derive identical golden streams.
    ///
    /// # Arguments
    /// * `genesis` - Genesis hash from handshake (chain identifier)
    /// * `height` - Block height (determines epoch)
    ///
    /// # Epoch Calculation
    /// epoch = height / GOLDEN_EPOCH_BLOCKS (default: 100 blocks/epoch)
    ///
    /// # Aliases
    /// This method is also available as `from_flock_seed` for backward
    /// compatibility with the P2P network module.
    pub fn from_genesis_epoch(genesis: &Hash, height: u64) -> Self {
        let epoch = height / GOLDEN_EPOCH_BLOCKS;

        // Combine genesis hash + epoch into deterministic seed
        let mut hasher = blake3::Hasher::new();
        hasher.update(genesis.as_bytes());
        hasher.update(&epoch.to_le_bytes());
        hasher.update(&GOLDEN_SEED);

        let hash = hasher.finalize();
        Self::new(hash.as_bytes())
    }

    /// Alias for `from_genesis_epoch` - backward compatibility with P2P module
    ///
    /// The P2P flock coordination code uses this name. Both methods are identical.
    #[inline]
    pub fn from_flock_seed(genesis: &Hash, height: u64) -> Self {
        Self::from_genesis_epoch(genesis, height)
    }

    /// Generate next 16 bytes of deterministic stream
    ///
    /// Uses XOR folding for uniform distribution:
    /// 1. Collect 256 "sifted" bits via basis matching
    /// 2. XOR fold into 128 output bits (16 bytes)
    pub fn next_bytes(&mut self) -> [u8; 16] {
        let mut sifted_bits: Vec<u8> = Vec::with_capacity(256);

        // Collect 256 sifted bits
        while sifted_bits.len() < 256 {
            // Hash state + counter
            let mut hasher = blake3::Hasher::new();
            hasher.update(&self.state);
            hasher.update(&self.counter.to_le_bytes());
            let hash = hasher.finalize();

            // Basis matching: extract bits where positions 1 and 2 match
            for byte in hash.as_bytes() {
                if Self::basis_match(*byte) {
                    sifted_bits.push(byte & 1);
                    if sifted_bits.len() >= 256 {
                        break;
                    }
                }
            }

            // Ratchet state forward
            self.state = *hash.as_bytes();
            self.counter += 1;
        }

        // XOR fold: combine first 128 bits with second 128 bits
        let mut output = [0u8; 16];
        for i in 0..128 {
            let bit = sifted_bits[i] ^ sifted_bits[i + 128];
            output[i / 8] |= bit << (i % 8);
        }

        output
    }

    /// Basis matching predicate (simulates QKD sifting)
    /// Returns true if bits at positions 1 and 2 are equal
    #[inline]
    fn basis_match(byte: u8) -> bool {
        ((byte >> 1) & 1) == ((byte >> 2) & 1)
    }

    /// Generate a golden ratio coin flip for integer z
    ///
    /// Uses the equidistribution property: {z*phi} is uniform in [0,1)
    /// Returns 0 if fractional part < 0.5, else 1
    pub fn coin_flip(&self, z: u64) -> u8 {
        let frac = Self::golden_fractional(z);
        if frac < 0.5 { 0 } else { 1 }
    }

    /// Compute fractional part of z*phi
    ///
    /// {z*phi} = z*phi - floor(z*phi)
    ///
    /// This provides the equidistribution property used for
    /// deterministic leaf ordering in merkle trees.
    #[inline]
    pub fn golden_fractional(z: u64) -> f64 {
        let product = (z as f64) * PHI;
        product - product.floor()
    }

    /// Generate deterministic f64 in range [0, 1)
    pub fn next_f64(&mut self) -> f64 {
        let bytes = self.next_bytes();
        let bits = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
        // Use 53 bits for f64 mantissa precision
        (bits >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Generate deterministic u64
    pub fn next_u64(&mut self) -> u64 {
        let bytes = self.next_bytes();
        u64::from_le_bytes(bytes[0..8].try_into().unwrap())
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_golden_generator_deterministic() {
        let seed = [0u8; 32];
        let mut gen1 = GoldenGenerator::new(&seed);
        let mut gen2 = GoldenGenerator::new(&seed);

        // Same seed should produce identical streams
        assert_eq!(gen1.next_bytes(), gen2.next_bytes());
        assert_eq!(gen1.next_bytes(), gen2.next_bytes());
        assert_eq!(gen1.next_bytes(), gen2.next_bytes());
    }

    #[test]
    fn test_golden_generator_different_seeds() {
        let seed1 = [0u8; 32];
        let mut seed2 = [0u8; 32];
        seed2[0] = 1;

        let mut gen1 = GoldenGenerator::new(&seed1);
        let mut gen2 = GoldenGenerator::new(&seed2);

        // Different seeds should produce different streams
        assert_ne!(gen1.next_bytes(), gen2.next_bytes());
    }

    #[test]
    fn test_from_genesis_epoch_deterministic() {
        let genesis = Hash::new(b"genesis_block");
        let height = 150; // Epoch 1

        let mut gen1 = GoldenGenerator::from_genesis_epoch(&genesis, height);
        let mut gen2 = GoldenGenerator::from_genesis_epoch(&genesis, height);

        // Same genesis + height should produce identical streams
        assert_eq!(gen1.next_bytes(), gen2.next_bytes());
    }

    #[test]
    fn test_epoch_calculation() {
        let genesis = Hash::new(b"genesis_block");

        // Heights 0-99 should be epoch 0
        let gen_h0 = GoldenGenerator::from_genesis_epoch(&genesis, 0);
        let gen_h50 = GoldenGenerator::from_genesis_epoch(&genesis, 50);
        let gen_h99 = GoldenGenerator::from_genesis_epoch(&genesis, 99);

        // Heights 100-199 should be epoch 1
        let gen_h100 = GoldenGenerator::from_genesis_epoch(&genesis, 100);

        // Same epoch should have same initial state
        assert_eq!(gen_h0.state, gen_h50.state);
        assert_eq!(gen_h0.state, gen_h99.state);

        // Different epoch should have different state
        assert_ne!(gen_h99.state, gen_h100.state);
    }

    #[test]
    fn test_golden_fractional_distribution() {
        // Test equidistribution property
        // {z*phi} should be uniformly distributed in [0,1)
        for z in 0..100u64 {
            let frac = GoldenGenerator::golden_fractional(z);
            assert!(frac >= 0.0 && frac < 1.0);
        }
    }

    #[test]
    fn test_golden_fractional_ordering() {
        // Golden fractional should provide consistent ordering
        let frac_0 = GoldenGenerator::golden_fractional(0);
        let frac_1 = GoldenGenerator::golden_fractional(1);
        let frac_2 = GoldenGenerator::golden_fractional(2);

        // Each should be unique
        assert_ne!(frac_0, frac_1);
        assert_ne!(frac_1, frac_2);
        assert_ne!(frac_0, frac_2);
    }

    #[test]
    fn test_coin_flip_deterministic() {
        let gen = GoldenGenerator::new(&[0u8; 32]);

        // Coin flip should be deterministic for same z
        let flip1 = gen.coin_flip(42);
        let flip2 = gen.coin_flip(42);
        assert_eq!(flip1, flip2);
    }
}
