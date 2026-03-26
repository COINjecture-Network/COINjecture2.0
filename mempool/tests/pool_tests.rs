//! Comprehensive mempool tests.
//!
//! All transactions here are properly signed with a matching KeyPair so
//! signature validation inside the pool is exercised correctly.

use coinject_core::{Address, KeyPair, Transaction};
use coinject_mempool::{PoolConfig, PoolError, TransactionPool};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create a valid, signed Transfer transaction.
fn make_tx(amount: u128, fee: u128, nonce: u64) -> Transaction {
    let kp = KeyPair::generate();
    Transaction::new_transfer(
        kp.address(),
        Address::from_bytes([0xAB; 32]),
        amount,
        fee,
        nonce,
        &kp,
    )
}

/// Create a pool with a small min_fee for most tests.
fn small_pool() -> TransactionPool {
    TransactionPool::with_config(PoolConfig {
        min_fee: 1_000,
        max_transactions: 100,
        max_size_bytes: 10 * 1024 * 1024,
    })
}

// ---------------------------------------------------------------------------
// Basic add / len
// ---------------------------------------------------------------------------

#[test]
fn test_empty_pool_has_zero_len() {
    let pool = small_pool();
    assert_eq!(pool.len(), 0);
    assert!(pool.is_empty());
}

#[test]
fn test_add_valid_transaction_succeeds() {
    let mut pool = small_pool();
    let tx = make_tx(5_000, 2_000, 1);
    let result = pool.add(tx);
    assert!(result.is_ok(), "Valid signed transaction must be accepted");
    assert_eq!(pool.len(), 1);
    assert!(!pool.is_empty());
}

#[test]
fn test_add_returns_transaction_hash() {
    let mut pool = small_pool();
    let kp = KeyPair::generate();
    let tx = Transaction::new_transfer(
        kp.address(),
        Address::from_bytes([1u8; 32]),
        1_000,
        2_000,
        1,
        &kp,
    );
    let expected_hash = tx.hash();
    let returned_hash = pool.add(tx).unwrap();
    assert_eq!(expected_hash, returned_hash);
}

#[test]
fn test_multiple_transactions_all_added() {
    let mut pool = small_pool();
    for i in 1..=5 {
        pool.add(make_tx(1_000, 1_000 * i as u128, i as u64))
            .unwrap();
    }
    assert_eq!(pool.len(), 5);
}

// ---------------------------------------------------------------------------
// Duplicate rejection
// ---------------------------------------------------------------------------

#[test]
fn test_duplicate_transaction_rejected() {
    let mut pool = small_pool();
    let kp = KeyPair::generate();
    let tx = Transaction::new_transfer(
        kp.address(),
        Address::from_bytes([1u8; 32]),
        1_000,
        2_000,
        1,
        &kp,
    );

    pool.add(tx.clone()).unwrap();
    let err = pool.add(tx).unwrap_err();
    assert_eq!(
        err,
        PoolError::DuplicateTransaction,
        "Second identical tx must be rejected"
    );
    assert_eq!(pool.len(), 1, "Pool size must not grow on duplicate");
}

#[test]
fn test_same_sender_different_nonce_both_accepted() {
    let mut pool = small_pool();
    let kp = KeyPair::generate();
    let to = Address::from_bytes([1u8; 32]);
    let tx1 = Transaction::new_transfer(kp.address(), to, 1_000, 2_000, 1, &kp);
    let tx2 = Transaction::new_transfer(kp.address(), to, 1_000, 2_000, 2, &kp);

    pool.add(tx1).unwrap();
    pool.add(tx2).unwrap();
    assert_eq!(pool.len(), 2);
}

// ---------------------------------------------------------------------------
// Fee validation
// ---------------------------------------------------------------------------

#[test]
fn test_fee_below_minimum_rejected() {
    let pool_config = PoolConfig {
        min_fee: 10_000,
        max_transactions: 100,
        max_size_bytes: 10 * 1024 * 1024,
    };
    let mut pool = TransactionPool::with_config(pool_config);
    let tx = make_tx(5_000, 999, 1); // fee 999 < min_fee 10_000
    let err = pool.add(tx).unwrap_err();
    assert_eq!(err, PoolError::FeeTooLow);
}

#[test]
fn test_fee_exactly_at_minimum_accepted() {
    let min = 5_000u128;
    let pool_config = PoolConfig {
        min_fee: min,
        max_transactions: 100,
        max_size_bytes: 10 * 1024 * 1024,
    };
    let mut pool = TransactionPool::with_config(pool_config);
    let tx = make_tx(1_000, min, 1);
    assert!(pool.add(tx).is_ok());
}

// ---------------------------------------------------------------------------
// Fee-based prioritization
// ---------------------------------------------------------------------------

#[test]
fn test_get_pending_ordered_by_fee_descending() {
    let mut pool = small_pool();

    pool.add(make_tx(1_000, 1_000, 1)).unwrap();
    pool.add(make_tx(1_000, 50_000, 2)).unwrap();
    pool.add(make_tx(1_000, 10_000, 3)).unwrap();

    let pending = pool.get_pending();
    assert_eq!(pending.len(), 3);
    assert_eq!(pending[0].fee(), 50_000, "Highest fee must be first");
    assert_eq!(pending[1].fee(), 10_000);
    assert_eq!(pending[2].fee(), 1_000, "Lowest fee must be last");
}

#[test]
fn test_get_top_n_returns_n_highest_fee_transactions() {
    let mut pool = small_pool();
    for i in 1..=10u128 {
        pool.add(make_tx(1_000, i * 1_000, i as u64)).unwrap();
    }

    let top3 = pool.get_top_n(3);
    assert_eq!(top3.len(), 3);
    // Fees: 10_000, 9_000, 8_000
    assert_eq!(top3[0].fee(), 10_000);
    assert_eq!(top3[1].fee(), 9_000);
    assert_eq!(top3[2].fee(), 8_000);
}

#[test]
fn test_get_top_n_larger_than_pool_returns_all() {
    let mut pool = small_pool();
    pool.add(make_tx(1_000, 5_000, 1)).unwrap();
    pool.add(make_tx(1_000, 3_000, 2)).unwrap();

    let top10 = pool.get_top_n(10);
    assert_eq!(top10.len(), 2, "top_n capped at pool size");
}

// ---------------------------------------------------------------------------
// Lookup / remove
// ---------------------------------------------------------------------------

#[test]
fn test_contains_and_get_after_add() {
    let mut pool = small_pool();
    let kp = KeyPair::generate();
    let tx = Transaction::new_transfer(
        kp.address(),
        Address::from_bytes([1u8; 32]),
        1_000,
        5_000,
        1,
        &kp,
    );
    let h = pool.add(tx.clone()).unwrap();

    assert!(pool.contains(&h));
    assert!(pool.get(&h).is_some());
}

#[test]
fn test_remove_decrements_len() {
    let mut pool = small_pool();
    let h = pool.add(make_tx(1_000, 5_000, 1)).unwrap();

    assert_eq!(pool.len(), 1);
    let removed = pool.remove(&h);
    assert!(removed.is_some());
    assert_eq!(pool.len(), 0);
    assert!(!pool.contains(&h));
}

#[test]
fn test_remove_nonexistent_returns_none() {
    let mut pool = small_pool();
    let fake_hash = make_tx(1_000, 5_000, 99).hash(); // not in pool
    assert!(pool.remove(&fake_hash).is_none());
}

#[test]
fn test_remove_batch_removes_all() {
    let mut pool = small_pool();
    let h1 = pool.add(make_tx(1_000, 5_000, 1)).unwrap();
    let h2 = pool.add(make_tx(1_000, 6_000, 2)).unwrap();
    let h3 = pool.add(make_tx(1_000, 7_000, 3)).unwrap();

    pool.remove_batch(&[h1, h2, h3]);
    assert!(pool.is_empty());
}

// ---------------------------------------------------------------------------
// Pool capacity and eviction
// ---------------------------------------------------------------------------

#[test]
fn test_pool_full_rejects_low_fee_tx() {
    let cfg = PoolConfig {
        max_transactions: 2,
        max_size_bytes: 10 * 1024 * 1024,
        min_fee: 1_000,
    };
    let mut pool = TransactionPool::with_config(cfg);

    pool.add(make_tx(1_000, 5_000, 1)).unwrap();
    pool.add(make_tx(1_000, 6_000, 2)).unwrap();

    // Pool is full; adding a lower-fee tx must fail
    let low = make_tx(1_000, 1_000, 3);
    let err = pool.add(low).unwrap_err();
    assert_eq!(err, PoolError::PoolFull);
    assert_eq!(pool.len(), 2);
}

#[test]
fn test_high_fee_evicts_lowest_when_pool_full() {
    let cfg = PoolConfig {
        max_transactions: 2,
        max_size_bytes: 10 * 1024 * 1024,
        min_fee: 1_000,
    };
    let mut pool = TransactionPool::with_config(cfg);

    pool.add(make_tx(1_000, 2_000, 1)).unwrap();
    pool.add(make_tx(1_000, 3_000, 2)).unwrap();

    // A higher-fee transaction should evict the lowest (fee=2_000)
    let high = make_tx(1_000, 100_000, 3);
    assert!(
        pool.add(high).is_ok(),
        "High-fee tx must evict the lowest and succeed"
    );
    assert_eq!(pool.len(), 2);

    // Remaining fees should be 3_000 and 100_000
    let pending = pool.get_pending();
    let fees: Vec<u128> = pending.iter().map(|t| t.fee()).collect();
    assert!(fees.contains(&100_000));
    assert!(fees.contains(&3_000));
    assert!(
        !fees.contains(&2_000),
        "Lowest-fee tx must have been evicted"
    );
}

// ---------------------------------------------------------------------------
// Clear
// ---------------------------------------------------------------------------

#[test]
fn test_clear_empties_pool() {
    let mut pool = small_pool();
    for i in 1..=5 {
        pool.add(make_tx(1_000, i * 1_000, i as u64)).unwrap();
    }
    assert_eq!(pool.len(), 5);
    pool.clear();
    assert_eq!(pool.len(), 0);
    assert!(pool.is_empty());
}

// ---------------------------------------------------------------------------
// Statistics
// ---------------------------------------------------------------------------

#[test]
fn test_stats_transactions_added_increments() {
    let mut pool = small_pool();
    assert_eq!(pool.stats().transactions_added, 0);
    pool.add(make_tx(1_000, 5_000, 1)).unwrap();
    assert_eq!(pool.stats().transactions_added, 1);
    pool.add(make_tx(1_000, 6_000, 2)).unwrap();
    assert_eq!(pool.stats().transactions_added, 2);
}

#[test]
fn test_stats_transactions_removed_increments_on_remove() {
    let mut pool = small_pool();
    let h = pool.add(make_tx(1_000, 5_000, 1)).unwrap();
    assert_eq!(pool.stats().transactions_removed, 0);
    pool.remove(&h);
    assert_eq!(pool.stats().transactions_removed, 1);
}

#[test]
fn test_stats_rejected_increments_on_duplicate() {
    let mut pool = small_pool();
    let kp = KeyPair::generate();
    let tx = Transaction::new_transfer(
        kp.address(),
        Address::from_bytes([1u8; 32]),
        1_000,
        5_000,
        1,
        &kp,
    );
    pool.add(tx.clone()).unwrap();
    let _ = pool.add(tx);
    assert_eq!(pool.stats().transactions_rejected, 1);
}

// ---------------------------------------------------------------------------
// Property-based tests (proptest)
// ---------------------------------------------------------------------------

use proptest::prelude::*;

proptest! {
    /// Any valid signed transaction (non-zero amount, fee >= min) must be accepted.
    #[test]
    fn prop_valid_tx_always_accepted(
        amount in 1u128..1_000_000u128,
        fee    in 1_000u128..100_000u128,
        nonce  in 0u64..10_000u64,
    ) {
        let mut pool = small_pool();
        let kp  = KeyPair::generate();
        let tx  = Transaction::new_transfer(kp.address(), Address::from_bytes([0xFFu8; 32]), amount, fee, nonce, &kp);
        prop_assert!(pool.add(tx).is_ok(), "Valid signed tx must be accepted by pool");
    }

    /// After adding N distinct transactions, pool.len() must equal N.
    #[test]
    fn prop_pool_len_matches_additions(n in 1usize..=20) {
        let mut pool = small_pool();
        for i in 0..n {
            pool.add(make_tx(1_000, 1_000 + i as u128, i as u64)).unwrap();
        }
        prop_assert_eq!(pool.len(), n);
    }
}
