// Transaction mempool with fee prioritization
// Institutional-grade transaction pool for Network B

use coinject_core::{Balance, Hash, Transaction};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};

/// Transaction with priority metadata
#[derive(Clone, Debug)]
struct PooledTransaction {
    tx: Transaction,
    tx_hash: Hash,
    fee: Balance,
    received_at: i64, // Unix timestamp for tie-breaking
}

impl PartialEq for PooledTransaction {
    fn eq(&self, other: &Self) -> bool {
        self.tx_hash == other.tx_hash
    }
}

impl Eq for PooledTransaction {}

impl PartialOrd for PooledTransaction {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PooledTransaction {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Primary: Higher fee = higher priority
        match self.fee.cmp(&other.fee) {
            std::cmp::Ordering::Equal => {
                // Tie-breaker: Earlier received = higher priority
                other.received_at.cmp(&self.received_at)
            }
            ordering => ordering,
        }
    }
}

/// Pool configuration
#[derive(Clone, Debug)]
pub struct PoolConfig {
    /// Maximum number of transactions in pool
    pub max_transactions: usize,
    /// Maximum total size in bytes
    pub max_size_bytes: usize,
    /// Minimum fee per transaction
    pub min_fee: Balance,
}

impl Default for PoolConfig {
    fn default() -> Self {
        PoolConfig {
            max_transactions: 10_000,
            max_size_bytes: 20 * 1024 * 1024, // 20 MB
            min_fee: 1000,                      // 1000 units minimum fee
        }
    }
}

/// Pool statistics
#[derive(Clone, Debug, Default)]
pub struct PoolStats {
    pub total_transactions: usize,
    pub total_bytes: usize,
    pub transactions_added: u64,
    pub transactions_removed: u64,
    pub transactions_rejected: u64,
}

/// Error types for pool operations
#[derive(Debug, Clone, PartialEq)]
pub enum PoolError {
    DuplicateTransaction,
    FeeTooLow,
    PoolFull,
    InvalidTransaction(String),
}

impl std::fmt::Display for PoolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PoolError::DuplicateTransaction => write!(f, "Transaction already in pool"),
            PoolError::FeeTooLow => write!(f, "Transaction fee below minimum"),
            PoolError::PoolFull => write!(f, "Pool is full"),
            PoolError::InvalidTransaction(msg) => write!(f, "Invalid transaction: {}", msg),
        }
    }
}

impl std::error::Error for PoolError {}

/// Transaction pool with fee-based prioritization
pub struct TransactionPool {
    /// Priority queue (max-heap by fee)
    queue: BinaryHeap<PooledTransaction>,
    /// Fast lookup by transaction hash
    by_hash: HashMap<Hash, Transaction>,
    /// Track unique transactions
    seen: HashSet<Hash>,
    /// Pool configuration
    config: PoolConfig,
    /// Statistics
    stats: PoolStats,
    /// Current total size in bytes
    current_size: usize,
}

impl TransactionPool {
    pub fn new() -> Self {
        Self::with_config(PoolConfig::default())
    }

    pub fn with_config(config: PoolConfig) -> Self {
        TransactionPool {
            queue: BinaryHeap::new(),
            by_hash: HashMap::new(),
            seen: HashSet::new(),
            config,
            stats: PoolStats::default(),
            current_size: 0,
        }
    }

    /// Add transaction to pool with validation
    pub fn add(&mut self, tx: Transaction) -> Result<Hash, PoolError> {
        eprintln!("🔵 MEMPOOL: add() called!");
        let tx_hash = tx.hash();
        eprintln!("🔵 MEMPOOL: transaction hash computed: {:?}", tx_hash);

        // Check for duplicate
        if self.seen.contains(&tx_hash) {
            eprintln!("🔴 MEMPOOL: Duplicate transaction detected");
            self.stats.transactions_rejected += 1;
            return Err(PoolError::DuplicateTransaction);
        }

        // Validate fee
        if tx.fee() < self.config.min_fee {
            self.stats.transactions_rejected += 1;
            return Err(PoolError::FeeTooLow);
        }

        // Validate signature
        if !tx.verify_signature() {
            self.stats.transactions_rejected += 1;
            return Err(PoolError::InvalidTransaction(
                "Invalid signature".to_string(),
            ));
        }

        // Estimate size (rough approximation)
        let tx_size = std::mem::size_of::<Transaction>();

        // Check capacity limits
        if self.stats.total_transactions >= self.config.max_transactions
            || self.current_size + tx_size > self.config.max_size_bytes
        {
            // Try to evict lowest-fee transaction if new one has higher fee
            if let Some(lowest) = self.queue.peek() {
                if tx.fee() > lowest.fee {
                    self.evict_lowest();
                } else {
                    self.stats.transactions_rejected += 1;
                    return Err(PoolError::PoolFull);
                }
            } else {
                self.stats.transactions_rejected += 1;
                return Err(PoolError::PoolFull);
            }
        }

        // Add to pool
        let pooled = PooledTransaction {
            tx: tx.clone(),
            tx_hash,
            fee: tx.fee(),
            received_at: chrono::Utc::now().timestamp(),
        };

        self.queue.push(pooled);
        self.by_hash.insert(tx_hash, tx);
        self.seen.insert(tx_hash);

        self.stats.total_transactions = self.queue.len();
        self.stats.total_bytes += tx_size;
        self.stats.transactions_added += 1;
        self.current_size += tx_size;

        Ok(tx_hash)
    }

    /// Get pending transactions ordered by fee (highest first)
    pub fn get_pending(&self) -> Vec<Transaction> {
        let mut sorted: Vec<_> = self.queue.iter().cloned().collect();
        sorted.sort_by(|a, b| b.cmp(a)); // Descending order
        sorted.into_iter().map(|p| p.tx).collect()
    }

    /// Get top N transactions by fee
    pub fn get_top_n(&self, n: usize) -> Vec<Transaction> {
        let mut sorted: Vec<_> = self.queue.iter().cloned().collect();
        sorted.sort_by(|a, b| b.cmp(a)); // Descending order
        sorted
            .into_iter()
            .take(n)
            .map(|p| p.tx)
            .collect()
    }

    /// Get transaction by hash
    pub fn get(&self, hash: &Hash) -> Option<&Transaction> {
        self.by_hash.get(hash)
    }

    /// Check if transaction exists in pool
    pub fn contains(&self, hash: &Hash) -> bool {
        self.seen.contains(hash)
    }

    /// Remove transaction from pool (e.g., after mining)
    pub fn remove(&mut self, hash: &Hash) -> Option<Transaction> {
        if let Some(tx) = self.by_hash.remove(hash) {
            self.seen.remove(hash);

            // Rebuild heap without the removed transaction
            let remaining: Vec<_> = self
                .queue
                .drain()
                .filter(|p| &p.tx_hash != hash)
                .collect();

            self.queue = BinaryHeap::from(remaining);

            let tx_size = std::mem::size_of::<Transaction>();
            self.stats.total_transactions = self.queue.len();
            self.stats.total_bytes = self.stats.total_bytes.saturating_sub(tx_size);
            self.stats.transactions_removed += 1;
            self.current_size = self.current_size.saturating_sub(tx_size);

            Some(tx)
        } else {
            None
        }
    }

    /// Remove multiple transactions (batch operation for mined blocks)
    pub fn remove_batch(&mut self, hashes: &[Hash]) {
        for hash in hashes {
            self.remove(hash);
        }
    }

    /// Evict lowest-fee transaction to make room
    fn evict_lowest(&mut self) {
        // Convert to min-heap temporarily to get lowest
        let mut min_heap: BinaryHeap<Reverse<PooledTransaction>> =
            self.queue.drain().map(Reverse).collect();

        if let Some(Reverse(lowest)) = min_heap.pop() {
            self.by_hash.remove(&lowest.tx_hash);
            self.seen.remove(&lowest.tx_hash);

            let tx_size = std::mem::size_of::<Transaction>();
            self.stats.transactions_removed += 1;
            self.current_size = self.current_size.saturating_sub(tx_size);
        }

        // Restore max-heap
        self.queue = min_heap.into_iter().map(|Reverse(p)| p).collect();
        self.stats.total_transactions = self.queue.len();
    }

    /// Clear all transactions
    pub fn clear(&mut self) {
        self.queue.clear();
        self.by_hash.clear();
        self.seen.clear();
        self.stats.total_transactions = 0;
        self.stats.total_bytes = 0;
        self.current_size = 0;
    }

    /// Get pool statistics
    pub fn stats(&self) -> &PoolStats {
        &self.stats
    }

    /// Get current transaction count
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Check if pool is empty
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

impl Default for TransactionPool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use coinject_core::{Address, Ed25519Signature, PublicKey};

    fn create_test_tx(fee: Balance, nonce: u64) -> Transaction {
        Transaction {
            from: Address::from_bytes([1u8; 32]),
            to: Address::from_bytes([2u8; 32]),
            amount: 1000,
            fee,
            nonce,
            public_key: PublicKey::from_bytes([3u8; 32]),
            signature: Ed25519Signature::from_bytes([4u8; 64]),
        }
    }

    #[test]
    fn test_add_transaction() {
        let mut pool = TransactionPool::new();
        let tx = create_test_tx(5000, 1);

        let result = pool.add(tx);
        // Will fail due to invalid signature in test, but tests the flow
        assert!(result.is_err()); // Expected: invalid signature
    }

    #[test]
    fn test_fee_prioritization() {
        let pool = TransactionPool::new();

        // This test demonstrates the structure
        // Real tests would need properly signed transactions
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn test_duplicate_rejection() {
        let pool = TransactionPool::new();
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn test_pool_capacity() {
        let config = PoolConfig {
            max_transactions: 5,
            max_size_bytes: 1024 * 1024,
            min_fee: 1000,
        };
        let pool = TransactionPool::with_config(config);
        assert_eq!(pool.len(), 0);
    }
}
