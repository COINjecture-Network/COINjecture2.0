//! Comprehensive unit tests for coinject-core
//!
//! Covers: Hash, Address, KeyPair (sign/verify), MerkleTree,
//! Block/Blockchain construction, and Transaction validation.

use coinject_core::{
    Address, Block, Blockchain, Hash, KeyPair, MerkleTree, PublicKey, Transaction,
};

// =============================================================================
// Hash
// =============================================================================

#[test]
fn test_hash_zero_constant_is_all_zeros() {
    assert_eq!(Hash::ZERO.as_bytes(), &[0u8; 32]);
}

#[test]
fn test_hash_new_is_deterministic() {
    let h1 = Hash::new(b"deterministic input");
    let h2 = Hash::new(b"deterministic input");
    assert_eq!(h1, h2);
}

#[test]
fn test_hash_different_inputs_produce_different_hashes() {
    let h1 = Hash::new(b"input A");
    let h2 = Hash::new(b"input B");
    assert_ne!(h1, h2);
}

#[test]
fn test_hash_empty_input_is_not_zero() {
    let h = Hash::new(b"");
    assert_ne!(h, Hash::ZERO);
}

#[test]
fn test_hash_from_bytes_roundtrip() {
    let bytes = [0xDE_u8; 32];
    let hash = Hash::from_bytes(bytes);
    assert_eq!(hash.as_bytes(), &bytes);
}

#[test]
fn test_hash_to_vec_length() {
    let hash = Hash::new(b"test");
    let v = hash.to_vec();
    assert_eq!(v.len(), 32);
}

#[test]
fn test_hash_display_is_64_hex_chars() {
    let hash = Hash::new(b"hello world");
    let s = format!("{}", hash);
    assert_eq!(s.len(), 64);
    assert!(s.chars().all(|c| c.is_ascii_hexdigit()));
}

// =============================================================================
// Address
// =============================================================================

#[test]
fn test_address_from_bytes_roundtrip() {
    let bytes = [0x42_u8; 32];
    let addr = Address::from_bytes(bytes);
    assert_eq!(addr.as_bytes(), &bytes);
}

#[test]
fn test_address_to_base58_is_nonempty() {
    let addr = Address::from_bytes([1u8; 32]);
    let b58 = addr.to_base58();
    assert!(!b58.is_empty());
}

#[test]
fn test_address_equality() {
    let a = Address::from_bytes([7u8; 32]);
    let b = Address::from_bytes([7u8; 32]);
    let c = Address::from_bytes([8u8; 32]);
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn test_address_from_keypair_is_pubkey_bytes() {
    let kp = KeyPair::generate();
    let addr_via_keypair = kp.address();
    let addr_via_pubkey = kp.public_key().to_address();
    assert_eq!(addr_via_keypair, addr_via_pubkey);
}

// =============================================================================
// KeyPair — sign / verify
// =============================================================================

#[test]
fn test_sign_then_verify_succeeds() {
    let kp = KeyPair::generate();
    let msg = b"this is the signing payload";
    let sig = kp.sign(msg);
    assert!(
        kp.public_key().verify(msg, &sig),
        "Fresh signature must verify"
    );
}

#[test]
fn test_verify_fails_on_tampered_message() {
    let kp = KeyPair::generate();
    let sig = kp.sign(b"original");
    assert!(
        !kp.public_key().verify(b"tampered", &sig),
        "Signature must not verify on a different message"
    );
}

#[test]
fn test_verify_fails_with_wrong_keypair() {
    let kp1 = KeyPair::generate();
    let kp2 = KeyPair::generate();
    let sig = kp1.sign(b"message");
    assert!(
        !kp2.public_key().verify(b"message", &sig),
        "kp1's signature must not verify under kp2's public key"
    );
}

#[test]
fn test_same_message_same_key_gives_same_signature() {
    // ed25519-dalek uses deterministic signing (RFC 8032)
    let kp = KeyPair::generate();
    let msg = b"deterministic signing";
    let sig1 = kp.sign(msg);
    let sig2 = kp.sign(msg);
    assert_eq!(sig1.as_bytes(), sig2.as_bytes());
}

#[test]
fn test_public_key_from_bytes_roundtrip() {
    let kp = KeyPair::generate();
    let pk = kp.public_key();
    let pk2 = PublicKey::from_bytes(*pk.as_bytes());
    assert_eq!(pk.as_bytes(), pk2.as_bytes());
}

// =============================================================================
// MerkleTree
// =============================================================================

#[test]
fn test_merkle_empty_returns_zero_hash() {
    let tree = MerkleTree::new(vec![]);
    assert_eq!(tree.root(), Hash::ZERO);
}

#[test]
fn test_merkle_single_leaf_equals_leaf_hash() {
    let data = b"only leaf";
    let tree = MerkleTree::new(vec![data.to_vec()]);
    assert_eq!(tree.root(), Hash::new(data));
}

#[test]
fn test_merkle_two_leaves_is_deterministic() {
    let data = vec![b"leaf1".to_vec(), b"leaf2".to_vec()];
    let r1 = MerkleTree::new(data.clone()).root();
    let r2 = MerkleTree::new(data).root();
    assert_eq!(r1, r2);
}

#[test]
fn test_merkle_order_sensitivity() {
    let tree_ab = MerkleTree::new(vec![b"A".to_vec(), b"B".to_vec()]);
    let tree_ba = MerkleTree::new(vec![b"B".to_vec(), b"A".to_vec()]);
    assert_ne!(
        tree_ab.root(),
        tree_ba.root(),
        "Merkle root must be order-sensitive"
    );
}

#[test]
fn test_merkle_four_leaves_not_zero() {
    let data: Vec<Vec<u8>> = (0u8..4).map(|i| vec![i; 32]).collect();
    let tree = MerkleTree::new(data);
    assert_ne!(tree.root(), Hash::ZERO);
}

#[test]
fn test_merkle_adding_leaf_changes_root() {
    let data3 = vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()];
    let data4 = vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec(), b"d".to_vec()];
    let r3 = MerkleTree::new(data3).root();
    let r4 = MerkleTree::new(data4).root();
    assert_ne!(r3, r4);
}

// =============================================================================
// Block / Blockchain
// =============================================================================

#[test]
fn test_genesis_block_height_is_zero() {
    let addr = Address::from_bytes([0u8; 32]);
    let block = Block::genesis(addr);
    assert_eq!(block.header.height, 0);
}

#[test]
fn test_genesis_block_prev_hash_is_zero() {
    let block = Block::genesis(Address::from_bytes([0u8; 32]));
    assert_eq!(block.header.prev_hash, Hash::ZERO);
}

#[test]
fn test_genesis_block_has_no_transactions() {
    let block = Block::genesis(Address::from_bytes([0u8; 32]));
    assert!(block.transactions.is_empty());
}

#[test]
fn test_genesis_block_coinbase_height_matches_header() {
    let block = Block::genesis(Address::from_bytes([0u8; 32]));
    assert_eq!(block.coinbase.height, block.header.height);
}

#[test]
fn test_genesis_block_total_fees_are_zero() {
    let block = Block::genesis(Address::from_bytes([0u8; 32]));
    assert_eq!(block.total_fees(), 0);
}

#[test]
fn test_genesis_block_hash_is_deterministic() {
    let addr = Address::from_bytes([1u8; 32]);
    let b1 = Block::genesis(addr);
    let b2 = Block::genesis(addr);
    assert_eq!(b1.hash(), b2.hash());
}

#[test]
fn test_genesis_block_hash_changes_with_different_miner() {
    let b1 = Block::genesis(Address::from_bytes([0u8; 32]));
    let b2 = Block::genesis(Address::from_bytes([1u8; 32]));
    assert_ne!(b1.hash(), b2.hash());
}

#[test]
fn test_blockchain_initial_height_is_zero() {
    let chain = Blockchain::new(Address::from_bytes([0u8; 32]));
    assert_eq!(chain.height(), 0);
}

#[test]
fn test_blockchain_tip_is_genesis_at_start() {
    let addr = Address::from_bytes([0u8; 32]);
    let chain = Blockchain::new(addr);
    assert_eq!(chain.tip().header.height, 0);
}

#[test]
fn test_blockchain_get_block_at_zero() {
    let chain = Blockchain::new(Address::from_bytes([0u8; 32]));
    assert!(chain.get_block(0).is_some());
}

#[test]
fn test_blockchain_get_block_beyond_tip_is_none() {
    let chain = Blockchain::new(Address::from_bytes([0u8; 32]));
    assert!(chain.get_block(1).is_none());
    assert!(chain.get_block(999).is_none());
}

// =============================================================================
// Transaction
// =============================================================================

#[test]
fn test_signed_transfer_is_valid() {
    let kp = KeyPair::generate();
    let from = kp.address();
    let to = Address::from_bytes([0xAB; 32]);
    let tx = Transaction::new_transfer(from, to, 1000, 1000, 1, &kp);
    assert!(tx.verify_signature(), "Freshly signed tx must verify");
    assert!(tx.is_valid(), "Freshly signed tx must be valid");
}

#[test]
fn test_zero_amount_transfer_is_invalid() {
    let kp = KeyPair::generate();
    let from = kp.address();
    let to = Address::from_bytes([0xAB; 32]);
    let tx = Transaction::new_transfer(from, to, 0, 1000, 1, &kp);
    // is_valid() fails because amount == 0
    assert!(!tx.is_valid(), "Zero-amount transfer must be invalid");
}

#[test]
fn test_transaction_fee_accessor() {
    let kp = KeyPair::generate();
    let tx = Transaction::new_transfer(
        kp.address(),
        Address::from_bytes([1u8; 32]),
        500,
        1234,
        1,
        &kp,
    );
    assert_eq!(tx.fee(), 1234);
}

#[test]
fn test_transaction_nonce_accessor() {
    let kp = KeyPair::generate();
    let tx = Transaction::new_transfer(
        kp.address(),
        Address::from_bytes([1u8; 32]),
        500,
        1000,
        42,
        &kp,
    );
    assert_eq!(tx.nonce(), 42);
}

#[test]
fn test_transaction_from_accessor() {
    let kp = KeyPair::generate();
    let from = kp.address();
    let tx = Transaction::new_transfer(from, Address::from_bytes([1u8; 32]), 500, 1000, 1, &kp);
    assert_eq!(*tx.from(), from);
}

#[test]
fn test_transfer_to_and_amount_accessors() {
    let kp = KeyPair::generate();
    let from = kp.address();
    let to = Address::from_bytes([0xBE; 32]);
    let tx = Transaction::new_transfer(from, to, 7777, 1000, 1, &kp);
    assert_eq!(tx.to(), Some(&to));
    assert_eq!(tx.amount(), Some(7777));
}

#[test]
fn test_different_nonces_produce_different_hashes() {
    let kp = KeyPair::generate();
    let from = kp.address();
    let to = Address::from_bytes([1u8; 32]);
    let tx1 = Transaction::new_transfer(from, to, 1000, 1000, 1, &kp);
    let tx2 = Transaction::new_transfer(from, to, 1000, 1000, 2, &kp);
    assert_ne!(tx1.hash(), tx2.hash());
}

#[test]
fn test_different_keypairs_produce_different_hashes() {
    let kp1 = KeyPair::generate();
    let kp2 = KeyPair::generate();
    let to = Address::from_bytes([1u8; 32]);
    let tx1 = Transaction::new_transfer(kp1.address(), to, 1000, 1000, 1, &kp1);
    let tx2 = Transaction::new_transfer(kp2.address(), to, 1000, 1000, 1, &kp2);
    assert_ne!(tx1.hash(), tx2.hash());
}

#[test]
fn test_timelock_transaction_is_valid_for_future_unlock() {
    let kp = KeyPair::generate();
    let from = kp.address();
    let recipient = Address::from_bytes([1u8; 32]);
    let future_time = chrono::Utc::now().timestamp() + 3600;
    let tx = Transaction::new_timelock(from, recipient, 1000, future_time, 1000, 1, &kp);
    assert!(tx.verify_signature());
    assert!(tx.is_valid());
}
