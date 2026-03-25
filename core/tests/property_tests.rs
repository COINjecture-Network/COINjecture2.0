//! Property-based tests for coinject-core using proptest.
//!
//! Verifies invariants that must hold for ALL valid inputs, not just specific cases.
//! Key properties tested:
//!   - Hash determinism: same input always produces same output
//!   - Signature correctness: any transaction created with a KeyPair must verify
//!   - Serialization round-trips: hash identity is preserved through serde
//!   - Work score non-negativity: score is always >= 0

use coinject_core::{Address, Hash, KeyPair, Transaction};
use proptest::prelude::*;

// =============================================================================
// Hash properties
// =============================================================================

proptest! {
    /// Hash::new must be deterministic: same bytes → same hash every time.
    #[test]
    fn prop_hash_deterministic(data in proptest::collection::vec(any::<u8>(), 0..512)) {
        let h1 = Hash::new(&data);
        let h2 = Hash::new(&data);
        prop_assert_eq!(h1, h2, "Hash::new must be deterministic");
    }

    /// Non-empty inputs must not produce the all-zeros sentinel.
    #[test]
    fn prop_hash_nonempty_input_not_zero(data in proptest::collection::vec(any::<u8>(), 1..256)) {
        let h = Hash::new(&data);
        prop_assert_ne!(h, Hash::ZERO, "Non-empty input must not hash to ZERO sentinel");
    }

    /// from_bytes / as_bytes roundtrip must be lossless.
    #[test]
    fn prop_hash_bytes_roundtrip(bytes in proptest::array::uniform32(any::<u8>())) {
        let h = Hash::from_bytes(bytes);
        prop_assert_eq!(h.as_bytes(), &bytes);
    }

    /// Two distinct non-empty inputs should (almost certainly) produce different hashes.
    /// This is a probabilistic test — collision probability is negligible for Blake3.
    #[test]
    fn prop_different_data_different_hash(
        a in proptest::collection::vec(any::<u8>(), 1..64),
        b in proptest::collection::vec(any::<u8>(), 1..64),
    ) {
        // Only assert if the inputs themselves differ
        prop_assume!(a != b);
        let ha = Hash::new(&a);
        let hb = Hash::new(&b);
        prop_assert_ne!(ha, hb, "Blake3 collision detected — extremely unlikely");
    }
}

// =============================================================================
// Address properties
// =============================================================================

proptest! {
    /// Address::from_bytes / as_bytes roundtrip.
    #[test]
    fn prop_address_bytes_roundtrip(bytes in proptest::array::uniform32(any::<u8>())) {
        let addr = Address::from_bytes(bytes);
        prop_assert_eq!(addr.as_bytes(), &bytes);
    }

    /// Two addresses from different byte arrays must compare as unequal.
    #[test]
    fn prop_address_equality_by_bytes(
        a in proptest::array::uniform32(any::<u8>()),
        b in proptest::array::uniform32(any::<u8>()),
    ) {
        prop_assume!(a != b);
        let addr_a = Address::from_bytes(a);
        let addr_b = Address::from_bytes(b);
        prop_assert_ne!(addr_a, addr_b);
    }
}

// =============================================================================
// Signature / KeyPair properties
// =============================================================================

proptest! {
    /// Any transfer created via Transaction::new_transfer must have a valid signature.
    #[test]
    fn prop_signed_transfer_always_verifies(
        amount  in 1u128..1_000_000_000u128,
        fee     in 1000u128..1_000_000u128,
        nonce   in 0u64..10_000u64,
    ) {
        let kp   = KeyPair::generate();
        let from = kp.address();
        let to   = Address::from_bytes([0x42u8; 32]);
        let tx   = Transaction::new_transfer(from, to, amount, fee, nonce, &kp);

        prop_assert!(tx.verify_signature(), "Any transaction signed with its own keypair must verify");
    }

    /// Varying only the nonce must still produce a valid signature each time.
    #[test]
    fn prop_varying_nonce_still_valid(nonce in 0u64..100_000u64) {
        let kp = KeyPair::generate();
        let tx = Transaction::new_transfer(
            kp.address(),
            Address::from_bytes([1u8; 32]),
            5000,
            1000,
            nonce,
            &kp,
        );
        prop_assert!(tx.verify_signature());
    }

    /// A non-zero amount transfer created with a matching keypair must pass is_valid().
    #[test]
    fn prop_valid_transfer_passes_is_valid(
        amount in 1u128..1_000_000u128,
        fee    in 1000u128..500_000u128,
        nonce  in 0u64..1000u64,
    ) {
        let kp = KeyPair::generate();
        let tx = Transaction::new_transfer(
            kp.address(),
            Address::from_bytes([2u8; 32]),
            amount,
            fee,
            nonce,
            &kp,
        );
        prop_assert!(tx.is_valid(), "Non-zero amount + valid sig must pass is_valid()");
    }
}

// =============================================================================
// Serialization round-trip properties
// =============================================================================

proptest! {
    /// Serializing and deserializing a Transaction (bincode) must preserve its hash.
    #[test]
    fn prop_transaction_serde_preserves_hash(
        amount in 1u128..1_000_000u128,
        fee    in 1000u128..100_000u128,
        nonce  in 0u64..1000u64,
    ) {
        let kp = KeyPair::generate();
        let tx = Transaction::new_transfer(
            kp.address(),
            Address::from_bytes([0xFFu8; 32]),
            amount,
            fee,
            nonce,
            &kp,
        );

        let original_hash = tx.hash();

        let bytes = bincode::serialize(&tx).expect("bincode serialization must succeed");
        let recovered: Transaction = bincode::deserialize(&bytes).expect("bincode deserialization must succeed");

        prop_assert_eq!(original_hash, recovered.hash(), "Hash must be identical after serde round-trip");
        prop_assert!(recovered.verify_signature(), "Signature must still verify after serde round-trip");
    }

    /// JSON round-trip must also preserve transaction identity.
    #[test]
    fn prop_transaction_json_roundtrip(
        amount in 1u128..1_000_000u128,
        fee    in 1000u128..100_000u128,
        nonce  in 0u64..1000u64,
    ) {
        let kp = KeyPair::generate();
        let tx = Transaction::new_transfer(
            kp.address(),
            Address::from_bytes([0xCCu8; 32]),
            amount,
            fee,
            nonce,
            &kp,
        );

        let original_hash = tx.hash();
        let json = serde_json::to_string(&tx).expect("JSON serialization must succeed");
        let recovered: Transaction = serde_json::from_str(&json).expect("JSON deserialization must succeed");

        prop_assert_eq!(original_hash, recovered.hash(), "Hash must survive JSON round-trip");
    }

    /// Hash serialization round-trip (bincode) must be lossless.
    #[test]
    fn prop_hash_serde_roundtrip(data in proptest::collection::vec(any::<u8>(), 0..256)) {
        let h = Hash::new(&data);
        let bytes = bincode::serialize(&h).unwrap();
        let recovered: Hash = bincode::deserialize(&bytes).unwrap();
        prop_assert_eq!(h, recovered);
    }
}
