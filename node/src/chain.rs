// Chain State Manager
// Block storage, best chain tracking, and chain reorganization
#![allow(dead_code)]
// ChainError wraps large redb error types; boxing them would change the public API
#![allow(clippy::result_large_err)]

use coinject_core::{Block, BlockHeader, Hash};
use lru::LruCache;
use redb::{Database, TableDefinition};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use thiserror::Error;
use tokio::sync::RwLock;

// Table definitions for redb (using fixed-size arrays for hash keys, strings for metadata keys)
const BLOCKS_TABLE: TableDefinition<&[u8; 32], &[u8]> = TableDefinition::new("blocks");
const METADATA_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("metadata");
const HEIGHT_INDEX_TABLE: TableDefinition<u64, &[u8; 32]> = TableDefinition::new("height_index");

/// Metadata key: sum of `header.work_score` along the canonical best chain (genesis → tip).
const CUM_WORK_META_KEY: &str = "cum_work";

/// Metadata key: sum of `coinbase.reward` on the canonical best chain (genesis → tip).
const TOTAL_MINTED_META_KEY: &str = "total_minted_rewards";

// v4.7.46: Maximum reasonable height to detect database corruption
// If height exceeds this, it's likely corrupted bytes being interpreted as u64
// 10 million blocks at 30s each = ~9.5 years of blocks
const MAX_REASONABLE_HEIGHT: u64 = 10_000_000;

#[derive(Error, Debug)]
pub enum ChainError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] redb::Error),
    #[error("Database creation error: {0}")]
    DatabaseCreationError(#[from] redb::DatabaseError),
    #[error("Storage error: {0}")]
    StorageError(#[from] redb::StorageError),
    #[error("Table error: {0}")]
    TableError(#[from] redb::TableError),
    #[error("Commit error: {0}")]
    CommitError(#[from] redb::CommitError),
    #[error("Transaction error: {0}")]
    TransactionError(#[from] redb::TransactionError),
    #[error("Block not found")]
    BlockNotFound,
    #[error("Invalid block height")]
    InvalidHeight,
    #[error("Serialization error: {0}")]
    SerializationError(#[from] bincode::Error),
    #[error("Genesis block mismatch")]
    GenesisMismatch,
    #[error("Compaction error: {0}")]
    CompactionError(#[from] redb::CompactionError),
}

/// Chain state manager handling block storage and retrieval
pub struct ChainState {
    /// redb database for block storage
    db: Arc<Database>,
    /// Path to the database file (used for backup/restore)
    db_path: PathBuf,
    /// Best block height
    best_height: Arc<RwLock<u64>>,
    /// Lock-free mirror of `best_height` for sync P2P block serving (see `BlockProvider`).
    best_height_atomic: Arc<AtomicU64>,
    /// Best block hash
    best_hash: Arc<RwLock<Hash>>,
    /// Genesis hash for network verification
    genesis_hash: Hash,
    /// In-memory LRU cache for deserialized blocks (avoids repeated bincode decodes)
    block_cache: Arc<Mutex<LruCache<Hash, Block>>>,
}

impl ChainState {
    /// Create or open chain state database.
    ///
    /// `block_cache_size` controls how many recently-accessed deserialized
    /// `Block` values are held in memory (LRU eviction).  Use `512` as a
    /// sensible default for full nodes.
    pub fn new<P: AsRef<Path>>(
        path: P,
        genesis_block: &Block,
        block_cache_size: usize,
    ) -> Result<Self, ChainError> {
        let db_path = path.as_ref().to_path_buf();
        let db = Database::create(&db_path)?;
        let db = Arc::new(db);

        let genesis_hash = genesis_block.header.hash();

        // Initialize tables
        let init_txn = db.begin_write()?;
        {
            let _ = init_txn.open_table(BLOCKS_TABLE)?;
            let _ = init_txn.open_table(METADATA_TABLE)?;
            let _ = init_txn.open_table(HEIGHT_INDEX_TABLE)?;
        }
        init_txn.commit()?;

        // Check if genesis exists
        let read_txn = db.begin_read()?;
        let stored_genesis = {
            let table = read_txn.open_table(METADATA_TABLE)?;
            table.get("genesis_hash")?
        };

        if let Some(stored_hash_ref) = stored_genesis {
            let stored_hash = Hash::from_bytes(
                stored_hash_ref
                    .value()
                    .try_into()
                    .map_err(|_| ChainError::GenesisMismatch)?,
            );

            if stored_hash != genesis_hash {
                return Err(ChainError::GenesisMismatch);
            }
        } else {
            // Store genesis block
            drop(read_txn);

            let write_txn = db.begin_write()?;
            {
                let mut metadata_table = write_txn.open_table(METADATA_TABLE)?;
                metadata_table.insert("genesis_hash", genesis_hash.as_bytes() as &[u8])?;
                metadata_table.insert("best_height", 0u64.to_le_bytes().as_ref())?;
                metadata_table.insert("best_hash", genesis_hash.as_bytes() as &[u8])?;
                let genesis_work = (genesis_block.header.work_score.max(0.0) as u64) as u128;
                metadata_table.insert(CUM_WORK_META_KEY, genesis_work.to_le_bytes().as_ref())?;
                metadata_table.insert(
                    TOTAL_MINTED_META_KEY,
                    genesis_block.coinbase.reward.to_le_bytes().as_ref(),
                )?;
            }
            write_txn.commit()?;

            Self::store_block_raw(&db, genesis_block)?;
            Self::set_height_index(&db, 0, genesis_hash)?;
        }

        // Load best height and hash
        let read_txn = db.begin_read()?;
        let (mut best_height, mut best_hash) = {
            let table = read_txn.open_table(METADATA_TABLE)?;

            let height_bytes = table.get("best_height")?.map(|v| v.value().to_vec());

            let hash_bytes = table.get("best_hash")?.map(|v| v.value().to_vec());

            let height = height_bytes
                .as_ref()
                .and_then(|b| <[u8; 8]>::try_from(b.as_slice()).ok())
                .map(u64::from_le_bytes)
                .unwrap_or(0);

            let hash = hash_bytes
                .as_ref()
                .and_then(|b| <[u8; 32]>::try_from(b.as_slice()).ok())
                .map(Hash::from_bytes)
                .unwrap_or(genesis_hash);

            (height, hash)
        };
        drop(read_txn);

        // v4.7.46: Database corruption detection and auto-fix
        // Corrupted bytes can be interpreted as impossibly high block heights
        if best_height > MAX_REASONABLE_HEIGHT {
            eprintln!(
                "⚠️  DATABASE CORRUPTION DETECTED: Best height {} exceeds maximum reasonable height {}",
                best_height, MAX_REASONABLE_HEIGHT
            );
            eprintln!("   This likely indicates corrupted database bytes being interpreted as u64 values.");
            eprintln!("   Auto-fixing: Resetting to genesis (height 0)...");

            // Reset to genesis
            best_height = 0;
            best_hash = genesis_hash;

            // Update database with corrected values
            let write_txn = db.begin_write()?;
            {
                let mut table = write_txn.open_table(METADATA_TABLE)?;
                table.insert("best_height", best_height.to_le_bytes().as_ref())?;
                table.insert("best_hash", best_hash.as_bytes() as &[u8])?;
                let genesis_work = (genesis_block.header.work_score.max(0.0) as u64) as u128;
                table.insert(CUM_WORK_META_KEY, genesis_work.to_le_bytes().as_ref())?;
                table.insert(
                    TOTAL_MINTED_META_KEY,
                    genesis_block.coinbase.reward.to_le_bytes().as_ref(),
                )?;
            }
            write_txn.commit()?;

            eprintln!("   ✅ Database auto-fixed: Reset to genesis block (height 0)");
            eprintln!("   The node will re-sync from peers.");
        }

        Self::ensure_cumulative_work_meta(&db, best_hash)?;
        Self::ensure_total_minted_meta(&db, best_hash, genesis_block)?;

        let cache_cap = NonZeroUsize::new(block_cache_size.max(1)).unwrap();
        Ok(ChainState {
            db,
            db_path,
            best_height: Arc::new(RwLock::new(best_height)),
            best_height_atomic: Arc::new(AtomicU64::new(best_height)),
            best_hash: Arc::new(RwLock::new(best_hash)),
            genesis_hash,
            block_cache: Arc::new(Mutex::new(LruCache::new(cache_cap))),
        })
    }

    fn read_cumulative_work_meta(db: &Arc<Database>) -> Result<Option<u128>, ChainError> {
        let read_txn = db.begin_read()?;
        let table = read_txn.open_table(METADATA_TABLE)?;
        let Some(v) = table.get(CUM_WORK_META_KEY)? else {
            return Ok(None);
        };
        let bytes = v.value();
        if bytes.len() != 16 {
            return Ok(None);
        }
        let arr: [u8; 16] = bytes[..16]
            .try_into()
            .map_err(|_| ChainError::InvalidHeight)?;
        Ok(Some(u128::from_le_bytes(arr)))
    }

    fn compute_cumulative_work_tip_db(
        db: &Arc<Database>,
        mut tip: Hash,
    ) -> Result<u128, ChainError> {
        let mut sum = 0u128;
        loop {
            let Some(block) = Self::load_block_from_db(db, &tip)? else {
                return Err(ChainError::BlockNotFound);
            };
            sum = sum.saturating_add((block.header.work_score.max(0.0) as u64) as u128);
            if block.header.height == 0 {
                break;
            }
            tip = block.header.prev_hash;
        }
        Ok(sum)
    }

    /// Σ `coinbase.reward` from genesis through `tip` (inclusive), walking `prev_hash`.
    fn compute_total_minted_tip_db(db: &Arc<Database>, mut tip: Hash) -> Result<u128, ChainError> {
        let mut sum = 0u128;
        loop {
            let Some(block) = Self::load_block_from_db(db, &tip)? else {
                return Err(ChainError::BlockNotFound);
            };
            sum = sum.saturating_add(block.coinbase.reward);
            if block.header.height == 0 {
                break;
            }
            tip = block.header.prev_hash;
        }
        Ok(sum)
    }

    fn read_total_minted_meta(db: &Arc<Database>) -> Result<Option<u128>, ChainError> {
        let read_txn = db.begin_read()?;
        let table = read_txn.open_table(METADATA_TABLE)?;
        let Some(v) = table.get(TOTAL_MINTED_META_KEY)? else {
            return Ok(None);
        };
        let bytes = v.value();
        if bytes.len() != 16 {
            return Ok(None);
        }
        let arr: [u8; 16] = bytes[..16]
            .try_into()
            .map_err(|_| ChainError::InvalidHeight)?;
        Ok(Some(u128::from_le_bytes(arr)))
    }

    /// Backfill `total_minted_rewards` for databases created before this metadata existed.
    fn ensure_total_minted_meta(
        db: &Arc<Database>,
        tip: Hash,
        genesis_block: &Block,
    ) -> Result<(), ChainError> {
        if Self::read_total_minted_meta(db)?.is_some() {
            return Ok(());
        }
        let m = Self::compute_total_minted_tip_db(db, tip).unwrap_or(genesis_block.coinbase.reward);
        let write_txn = db.begin_write()?;
        {
            let mut table = write_txn.open_table(METADATA_TABLE)?;
            table.insert(TOTAL_MINTED_META_KEY, m.to_le_bytes().as_ref())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Cumulative work (Σ truncated `work_score`) from genesis through `tip_hash` (inclusive).
    pub fn cumulative_work_at_tip_hash(&self, tip_hash: Hash) -> Result<u128, ChainError> {
        Self::compute_cumulative_work_tip_db(&self.db, tip_hash)
    }

    /// Sum of coinbase rewards on the canonical best chain to the current tip (metadata).
    pub fn best_total_minted_rewards(&self) -> u128 {
        Self::read_total_minted_meta(&self.db)
            .ok()
            .flatten()
            .unwrap_or(0)
    }

    fn load_block_from_db(db: &Arc<Database>, hash: &Hash) -> Result<Option<Block>, ChainError> {
        let read_txn = db.begin_read()?;
        let table = read_txn.open_table(BLOCKS_TABLE)?;
        match table.get(hash.as_bytes())? {
            Some(bytes_ref) => Ok(Some(bincode::deserialize(bytes_ref.value())?)),
            None => Ok(None),
        }
    }

    /// If `cum_work` is missing (pre-upgrade DB), compute from the stored tip and persist.
    fn ensure_cumulative_work_meta(db: &Arc<Database>, tip: Hash) -> Result<(), ChainError> {
        if Self::read_cumulative_work_meta(db)?.is_some() {
            return Ok(());
        }
        let w = Self::compute_cumulative_work_tip_db(db, tip)?;
        let write_txn = db.begin_write()?;
        {
            let mut table = write_txn.open_table(METADATA_TABLE)?;
            table.insert(CUM_WORK_META_KEY, w.to_le_bytes().as_ref())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Store block bytes keyed by hash only (side-chain / buffer blocks must not touch height index).
    fn store_block_raw(db: &Arc<Database>, block: &Block) -> Result<(), ChainError> {
        let block_bytes = bincode::serialize(block)?;
        let hash = block.header.hash();

        let write_txn = db.begin_write()?;
        {
            let mut blocks_table = write_txn.open_table(BLOCKS_TABLE)?;
            blocks_table.insert(hash.as_bytes(), block_bytes.as_slice())?;
        }
        write_txn.commit()?;

        Ok(())
    }

    /// Map height → hash on the canonical best chain only.
    pub(crate) fn set_height_index(
        db: &Arc<Database>,
        height: u64,
        hash: Hash,
    ) -> Result<(), ChainError> {
        let write_txn = db.begin_write()?;
        {
            let mut height_table = write_txn.open_table(HEIGHT_INDEX_TABLE)?;
            height_table.insert(height, hash.as_bytes())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Read canonical tip from metadata (sync-safe; used by P2P block serving).
    fn read_best_tip_from_db(db: &Arc<Database>) -> Result<(u64, Hash), ChainError> {
        let read_txn = db.begin_read()?;
        let table = read_txn.open_table(METADATA_TABLE)?;
        let height = table
            .get("best_height")?
            .map(|v| u64::from_le_bytes(v.value().try_into().unwrap()))
            .unwrap_or(0);
        let hash = table
            .get("best_hash")?
            .map(|v| Hash::from_bytes(v.value().try_into().unwrap()))
            .unwrap_or(Hash::ZERO);
        Ok((height, hash))
    }

    /// Walk the canonical tip backward to the block at `height`.
    pub fn get_canonical_block_by_height(&self, height: u64) -> Result<Option<Block>, ChainError> {
        let (best_height, best_hash) = Self::read_best_tip_from_db(&self.db)?;
        if height > best_height {
            return Ok(None);
        }
        let mut hash = best_hash;
        let mut h = best_height;
        while h > height {
            let Some(block) = self.get_block_by_hash(&hash)? else {
                return Ok(None);
            };
            hash = block.header.prev_hash;
            h -= 1;
        }
        self.get_block_by_hash(&hash)
    }

    /// True if `candidate` is an ancestor of `(tip_hash, tip_height)` on stored blocks.
    #[cfg(not(feature = "adzdb"))]
    pub fn is_hash_on_canonical_chain_from_tip(
        &self,
        tip_hash: Hash,
        tip_height: u64,
        candidate: &Hash,
    ) -> Result<bool, ChainError> {
        crate::sync_canonical::is_hash_on_chain_from_tip(self, tip_hash, tip_height, candidate)
    }

    /// Test / repair hook: set a single height index entry.
    #[cfg(test)]
    pub(crate) fn set_height_index_entry(&self, height: u64, hash: Hash) -> Result<(), ChainError> {
        Self::set_height_index(&self.db, height, hash)
    }

    /// Rewrite height_index entries along the canonical chain from tip to genesis.
    pub fn rebuild_height_index_from_canonical_tip(
        &self,
        tip_hash: Hash,
        tip_height: u64,
    ) -> Result<(), ChainError> {
        let mut hash = tip_hash;
        let mut h = tip_height;
        loop {
            Self::set_height_index(&self.db, h, hash)?;
            if h == 0 {
                break;
            }
            let Some(block) = self.get_block_by_hash(&hash)? else {
                return Err(ChainError::InvalidHeight);
            };
            hash = block.header.prev_hash;
            h -= 1;
        }
        Ok(())
    }

    /// Store a block and update best chain if needed
    pub async fn store_block(&self, block: &Block) -> Result<bool, ChainError> {
        let block_hash = block.header.hash();
        let block_height = block.header.height;

        // v4.7.46: Validate block height to prevent database corruption
        if block_height > MAX_REASONABLE_HEIGHT {
            eprintln!(
                "⚠️  Rejecting block with impossibly high height {}: exceeds MAX_REASONABLE_HEIGHT {}",
                block_height, MAX_REASONABLE_HEIGHT
            );
            return Err(ChainError::InvalidHeight);
        }

        // Store the block
        Self::store_block_raw(&self.db, block)?;

        // Check if this extends the best chain
        let current_best_height = *self.best_height.read().await;
        let current_best_hash = *self.best_hash.read().await;

        if block_height > current_best_height {
            // Verify this block actually extends our current best chain
            if block.header.prev_hash != current_best_hash {
                // Block doesn't extend current chain - don't update best chain
                return Ok(false);
            }

            // New best block
            *self.best_height.write().await = block_height;
            self.best_height_atomic
                .store(block_height, Ordering::Release);
            *self.best_hash.write().await = block_hash;

            let prev_cum = Self::read_cumulative_work_meta(&self.db)?.unwrap_or(0);
            let inc = (block.header.work_score.max(0.0) as u64) as u128;
            let new_cum = prev_cum.saturating_add(inc);

            let prev_minted = Self::read_total_minted_meta(&self.db)?.unwrap_or(0);
            let new_minted = prev_minted.saturating_add(block.coinbase.reward);

            let write_txn = self.db.begin_write()?;
            {
                let mut table = write_txn.open_table(METADATA_TABLE)?;
                table.insert("best_height", block_height.to_le_bytes().as_ref())?;
                table.insert("best_hash", block_hash.as_bytes() as &[u8])?;
                table.insert(CUM_WORK_META_KEY, new_cum.to_le_bytes().as_ref())?;
                table.insert(TOTAL_MINTED_META_KEY, new_minted.to_le_bytes().as_ref())?;
            }
            write_txn.commit()?;

            Self::set_height_index(&self.db, block_height, block_hash)?;

            println!(
                "New best block: height={} hash={:?}",
                block_height, block_hash
            );
            return Ok(true);
        }

        Ok(false)
    }

    /// Get block by hash.
    ///
    /// Results are cached in an LRU cache to avoid repeated bincode deserialisation.
    pub fn get_block_by_hash(&self, hash: &Hash) -> Result<Option<Block>, ChainError> {
        // Check cache first
        if let Ok(mut cache) = self.block_cache.lock() {
            if let Some(block) = cache.get(hash) {
                return Ok(Some(block.clone()));
            }
        }

        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(BLOCKS_TABLE)?;

        match table.get(hash.as_bytes())? {
            Some(bytes_ref) => {
                let block: Block = bincode::deserialize(bytes_ref.value())?;
                // Store in cache
                if let Ok(mut cache) = self.block_cache.lock() {
                    cache.put(*hash, block.clone());
                }
                Ok(Some(block))
            }
            None => Ok(None),
        }
    }

    /// Get block by height on the **canonical** best chain (walks back from best tip).
    pub fn get_block_by_height(&self, height: u64) -> Result<Option<Block>, ChainError> {
        self.get_canonical_block_by_height(height)
    }

    /// Get block header by height
    pub fn get_header_by_height(&self, height: u64) -> Result<Option<BlockHeader>, ChainError> {
        Ok(self.get_block_by_height(height)?.map(|b| b.header))
    }

    /// Get the best block height
    pub async fn best_block_height(&self) -> u64 {
        *self.best_height.read().await
    }

    /// Get the best block hash
    pub async fn best_block_hash(&self) -> Hash {
        *self.best_hash.read().await
    }

    /// Cumulative work (Σ `header.work_score`) on the canonical best chain to the current tip.
    pub async fn best_cumulative_work(&self) -> u128 {
        Self::read_cumulative_work_meta(&self.db)
            .ok()
            .flatten()
            .unwrap_or(0)
    }

    /// Get the best block
    pub async fn best_block(&self) -> Result<Option<Block>, ChainError> {
        let hash = self.best_block_hash().await;
        self.get_block_by_hash(&hash)
    }

    /// Get genesis hash
    pub fn genesis_hash(&self) -> Hash {
        self.genesis_hash
    }

    /// Get shared reference to best height
    pub fn best_height_ref(&self) -> Arc<RwLock<u64>> {
        Arc::clone(&self.best_height)
    }

    /// Lock-free best height for sync block serving (`BlockProvider`).
    pub fn best_height_atomic_ref(&self) -> Arc<AtomicU64> {
        Arc::clone(&self.best_height_atomic)
    }

    /// Get shared reference to best hash
    pub fn best_hash_ref(&self) -> Arc<RwLock<Hash>> {
        Arc::clone(&self.best_hash)
    }

    /// Check if a block exists
    pub fn has_block(&self, hash: &Hash) -> Result<bool, ChainError> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(BLOCKS_TABLE)?;
        Ok(table.get(hash.as_bytes())?.is_some())
    }

    /// Get chain statistics
    pub async fn get_stats(&self) -> ChainStats {
        let db_file_size_bytes = self.db_file_size().unwrap_or(0);
        ChainStats {
            best_height: self.best_block_height().await,
            best_hash: self.best_block_hash().await,
            genesis_hash: self.genesis_hash,
            db_file_size_bytes,
        }
    }

    /// Resolve a header by hash from committed chain storage, or from `alt_chain` (typically the
    /// in-memory P2P sync buffer holding blocks not yet written to disk).
    ///
    /// [`Self::find_common_ancestor_with_alt_chain`] needs this so walking a peer's alternate branch
    /// does not return `None` just because intermediate blocks only exist in the buffer.
    fn block_by_hash_or_alt(
        &self,
        hash: &Hash,
        alt_chain: &[Block],
    ) -> Result<Option<Block>, ChainError> {
        if let Some(b) = self.get_block_by_hash(hash)? {
            return Ok(Some(b));
        }
        for b in alt_chain {
            if b.header.hash() == *hash {
                return Ok(Some(b.clone()));
            }
        }
        Ok(None)
    }

    /// Find common ancestor between our canonical best chain and a target header, optionally
    /// resolving the **competing** branch from `alt_chain` (sync buffer) when hashes are not
    /// stored yet.
    ///
    /// Pass `alt_chain: &[]` for behaviour identical to [`Self::find_common_ancestor`].
    pub async fn find_common_ancestor_with_alt_chain(
        &self,
        target_hash: &Hash,
        target_height: u64,
        alt_chain: &[Block],
    ) -> Result<Option<(Hash, u64)>, ChainError> {
        let current_best_hash = self.best_block_hash().await;
        let current_best_height = self.best_block_height().await;

        // If target is at same or lower height, check if it's on our chain
        if target_height <= current_best_height {
            // Walk back from current best to target height
            let mut current_hash = current_best_hash;
            let mut current_height = current_best_height;

            while current_height > target_height {
                if let Some(block) = self.get_block_by_hash(&current_hash)? {
                    current_hash = block.header.prev_hash;
                    current_height -= 1;
                } else {
                    return Ok(None);
                }
            }

            // Check if we reached the target
            if current_hash == *target_hash {
                return Ok(Some((current_hash, current_height)));
            }
        }

        // Walk back both chains to find common ancestor
        let mut our_hash = current_best_hash;
        let mut our_height = current_best_height;
        let mut their_hash = *target_hash;
        let mut their_height = target_height;

        // Align heights
        while our_height > their_height {
            if let Some(block) = self.get_block_by_hash(&our_hash)? {
                our_hash = block.header.prev_hash;
                our_height -= 1;
            } else {
                return Ok(None);
            }
        }

        while their_height > our_height {
            if let Some(block) = self.block_by_hash_or_alt(&their_hash, alt_chain)? {
                // Check if this block's prev_hash matches our chain at the previous height
                // This allows finding common ancestors even when intermediate blocks are missing
                if their_height > 0 {
                    if let Ok(Some(our_block_at_height)) =
                        self.get_block_by_height(their_height - 1)
                    {
                        if block.header.prev_hash == our_block_at_height.header.hash() {
                            // Found common ancestor!
                            return Ok(Some((our_block_at_height.header.hash(), their_height - 1)));
                        }
                    }
                }
                their_hash = block.header.prev_hash;
                their_height -= 1;
            } else {
                // Block missing in their chain - check if we have a block at this height in our chain
                // If their_hash matches our block hash at this height, that's the common ancestor
                if let Ok(Some(our_block_at_height)) = self.get_block_by_height(their_height) {
                    if their_hash == our_block_at_height.header.hash() {
                        // Found common ancestor!
                        return Ok(Some((their_hash, their_height)));
                    }
                }
                // Can't continue - no common ancestor found at this point
                return Ok(None);
            }
        }

        // Now both at same height, walk back until we find common ancestor
        while our_height > 0 && our_hash != their_hash {
            if let Some(our_block) = self.get_block_by_hash(&our_hash)? {
                our_hash = our_block.header.prev_hash;
            } else {
                // Missing block in our chain - check if their chain has a block that matches our height
                // This handles gaps in our chain
                if let Some(their_block) = self.block_by_hash_or_alt(&their_hash, alt_chain)? {
                    // Check if their prev_hash matches any block we have at this height
                    if let Ok(Some(our_block_at_height)) = self.get_block_by_height(our_height) {
                        if their_block.header.prev_hash == our_block_at_height.header.hash() {
                            // Found common ancestor!
                            return Ok(Some((our_block_at_height.header.hash(), our_height)));
                        }
                    }
                }
                return Ok(None);
            }

            if let Some(their_block) = self.block_by_hash_or_alt(&their_hash, alt_chain)? {
                // Check if their block's prev_hash matches our block at the previous height
                if their_height > 0 {
                    if let Ok(Some(our_block_at_height)) =
                        self.get_block_by_height(their_height - 1)
                    {
                        if their_block.header.prev_hash == our_block_at_height.header.hash() {
                            // Found common ancestor!
                            return Ok(Some((our_block_at_height.header.hash(), their_height - 1)));
                        }
                    }
                }
                their_hash = their_block.header.prev_hash;
            } else {
                // Missing block in their chain - check if we have a block at this height
                // If their_hash matches our block hash at this height, that's the common ancestor
                if let Ok(Some(our_block_at_height)) = self.get_block_by_height(their_height) {
                    if their_hash == our_block_at_height.header.hash() {
                        // Found common ancestor!
                        return Ok(Some((their_hash, their_height)));
                    }
                }
                // Can't continue - no common ancestor found
                return Ok(None);
            }

            our_height -= 1;
        }

        if our_hash == their_hash {
            Ok(Some((our_hash, our_height)))
        } else {
            Ok(None)
        }
    }

    /// Find common ancestor between current best chain and a target block
    /// Returns (common_ancestor_hash, common_ancestor_height)
    pub async fn find_common_ancestor(
        &self,
        target_hash: &Hash,
        target_height: u64,
    ) -> Result<Option<(Hash, u64)>, ChainError> {
        self.find_common_ancestor_with_alt_chain(target_hash, target_height, &[])
            .await
    }

    /// Get chain path from start_hash to end_hash (inclusive)
    /// Returns blocks in order from start to end
    pub fn get_chain_path(
        &self,
        start_hash: &Hash,
        start_height: u64,
        end_hash: &Hash,
        end_height: u64,
    ) -> Result<Vec<Block>, ChainError> {
        if start_height > end_height {
            return Ok(Vec::new());
        }

        let mut path = Vec::new();
        let mut current_hash = *start_hash;
        let mut current_height = start_height;

        // If start == end, return single block
        if start_hash == end_hash {
            if let Some(block) = self.get_block_by_hash(start_hash)? {
                path.push(block);
            }
            return Ok(path);
        }

        // Walk forward from start to end
        while current_height <= end_height {
            if let Some(block) = self.get_block_by_hash(&current_hash)? {
                path.push(block.clone());

                if current_hash == *end_hash {
                    break;
                }

                // Move to next block
                current_hash = block.header.hash();
                current_height += 1;

                // Find next block by height (since we don't have next_hash)
                if current_height <= end_height {
                    if let Some(next_block) = self.get_block_by_height(current_height)? {
                        // Verify it connects
                        if next_block.header.prev_hash == current_hash {
                            current_hash = next_block.header.hash();
                        } else {
                            // Chain broken, return what we have
                            break;
                        }
                    } else {
                        break;
                    }
                }
            } else {
                break;
            }
        }

        Ok(path)
    }

    /// Reorganize chain to a new best block
    /// Returns (old_chain_blocks, new_chain_blocks) for state unwinding/reapplying
    pub async fn prepare_reorganization(
        &self,
        new_best_hash: &Hash,
        new_best_height: u64,
    ) -> Result<(Vec<Block>, Vec<Block>), ChainError> {
        let _current_best_hash = self.best_block_hash().await;
        let current_best_height = self.best_block_height().await;

        // Find common ancestor
        let (_common_hash, common_height) = match self
            .find_common_ancestor(new_best_hash, new_best_height)
            .await?
        {
            Some((hash, height)) => (hash, height),
            None => {
                // No common ancestor found, can't reorganize
                return Err(ChainError::GenesisMismatch);
            }
        };

        // Get old chain blocks (from common ancestor to current best, excluding common ancestor)
        let old_chain = if common_height < current_best_height {
            // Get blocks from common+1 to current best
            let mut old_blocks = Vec::new();
            for height in (common_height + 1)..=current_best_height {
                if let Some(block) = self.get_block_by_height(height)? {
                    old_blocks.push(block);
                }
            }
            old_blocks.reverse(); // Reverse so we unwind from newest to oldest
            old_blocks
        } else {
            Vec::new()
        };

        // Get new chain blocks (from common ancestor to new best, excluding common ancestor)
        // Note: We need to get these from the network, so this will be called after blocks are received
        // For now, return empty new_chain - caller will populate it
        let new_chain = Vec::new();

        Ok((old_chain, new_chain))
    }

    /// Update best chain to new block (after reorganization validation)
    pub async fn update_best_chain(
        &self,
        new_best_hash: Hash,
        new_best_height: u64,
    ) -> Result<(), ChainError> {
        *self.best_height.write().await = new_best_height;
        self.best_height_atomic
            .store(new_best_height, Ordering::Release);
        *self.best_hash.write().await = new_best_hash;

        let new_cum = Self::compute_cumulative_work_tip_db(&self.db, new_best_hash)?;
        let new_minted = Self::compute_total_minted_tip_db(&self.db, new_best_hash)?;

        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(METADATA_TABLE)?;
            table.insert("best_height", new_best_height.to_le_bytes().as_ref())?;
            table.insert("best_hash", new_best_hash.as_bytes() as &[u8])?;
            table.insert(CUM_WORK_META_KEY, new_cum.to_le_bytes().as_ref())?;
            table.insert(TOTAL_MINTED_META_KEY, new_minted.to_le_bytes().as_ref())?;
        }
        write_txn.commit()?;

        self.rebuild_height_index_from_canonical_tip(new_best_hash, new_best_height)?;

        println!(
            "🔄 Chain reorganized: new best block height={} hash={:?}",
            new_best_height, new_best_hash
        );
        Ok(())
    }

    // =========================================================================
    // Phase 13: Database Management — Pruning, Backup, Compaction
    // =========================================================================

    /// Prune blocks older than `keep_height` from the database.
    ///
    /// Removes both the block data and the height-index entries for all heights
    /// strictly less than `keep_height`.  The genesis block (height 0) is always
    /// preserved.  Returns the number of blocks pruned.
    ///
    /// NOTE: The best block and any block at or above `keep_height` are never touched.
    pub async fn prune_blocks_before(&self, keep_height: u64) -> Result<u64, ChainError> {
        if keep_height == 0 {
            return Ok(0);
        }

        let current_best = self.best_block_height().await;
        // Never prune up to or above the best block
        let prune_below = keep_height.min(current_best);

        // Collect the hashes we need to remove (heights 1..prune_below)
        let to_prune: Vec<(u64, [u8; 32])> = {
            let read_txn = self.db.begin_read()?;
            let height_table = read_txn.open_table(HEIGHT_INDEX_TABLE)?;
            height_table
                .range(1..prune_below)?
                .filter_map(|r| r.ok())
                .map(|(h, hash_ref)| (h.value(), *hash_ref.value()))
                .collect()
        };

        if to_prune.is_empty() {
            return Ok(0);
        }

        let pruned_count = to_prune.len() as u64;
        let write_txn = self.db.begin_write()?;
        {
            let mut height_table = write_txn.open_table(HEIGHT_INDEX_TABLE)?;
            let mut blocks_table = write_txn.open_table(BLOCKS_TABLE)?;
            for (_height, hash) in &to_prune {
                height_table.remove(_height)?;
                blocks_table.remove(hash)?;
            }
        }
        write_txn.commit()?;

        // Evict pruned blocks from the LRU cache
        if let Ok(mut cache) = self.block_cache.lock() {
            for (_height, hash) in &to_prune {
                let h = Hash::from_bytes(*hash);
                cache.pop(&h);
            }
        }

        tracing::info!(
            "Pruned {} blocks (heights 1..{})",
            pruned_count,
            prune_below
        );
        Ok(pruned_count)
    }

    /// Back up the chain database to `dest_dir`.
    ///
    /// Copies the redb file to `{dest_dir}/chain.db.bak`.  The database is
    /// flushed before the copy so the backup is consistent.  For safest results
    /// run this when no writes are in flight (e.g. node is idle).
    pub fn backup(&self, dest_dir: &Path) -> Result<(), ChainError> {
        std::fs::create_dir_all(dest_dir).map_err(|e| {
            ChainError::DatabaseCreationError(redb::DatabaseError::Storage(redb::StorageError::Io(
                e,
            )))
        })?;
        let backup_path = dest_dir.join("chain.db.bak");
        std::fs::copy(&self.db_path, &backup_path).map_err(|e| {
            ChainError::DatabaseCreationError(redb::DatabaseError::Storage(redb::StorageError::Io(
                e,
            )))
        })?;
        tracing::info!("Chain database backed up to {:?}", backup_path);
        Ok(())
    }

    /// Compact the chain database, reclaiming space freed by pruned entries.
    ///
    /// **The node must be stopped before calling this.**  Compaction opens a
    /// fresh database handle and calls redb's `compact()`, which requires
    /// exclusive `&mut` access.  Pass `self.db_path()` as the argument.
    ///
    /// Returns `true` if the database was actually compacted (i.e. wasted
    /// space existed), `false` if already fully compact.
    pub fn compact_database(db_path: &Path) -> Result<bool, ChainError> {
        let mut db = Database::create(db_path)?;
        let compacted = db.compact()?;
        if compacted {
            tracing::info!("Chain database compacted at {:?}", db_path);
        }
        Ok(compacted)
    }

    /// Return the path of the underlying database file.
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    /// Return the on-disk size of the chain database file in bytes.
    pub fn db_file_size(&self) -> Result<u64, ChainError> {
        let meta = std::fs::metadata(&self.db_path).map_err(|e| {
            ChainError::DatabaseCreationError(redb::DatabaseError::Storage(redb::StorageError::Io(
                e,
            )))
        })?;
        Ok(meta.len())
    }

    /// Export a portable state snapshot.
    ///
    /// Copies the chain database to `{dest_dir}/chain-snapshot-{height}.db` for
    /// use by nodes performing fast-sync.  The snapshot filename embeds the best
    /// block height so receivers can validate integrity.
    pub async fn export_snapshot(&self, dest_dir: &Path) -> Result<PathBuf, ChainError> {
        std::fs::create_dir_all(dest_dir).map_err(|e| {
            ChainError::DatabaseCreationError(redb::DatabaseError::Storage(redb::StorageError::Io(
                e,
            )))
        })?;
        let height = self.best_block_height().await;
        let snap_name = format!("chain-snapshot-{}.db", height);
        let snap_path = dest_dir.join(&snap_name);
        std::fs::copy(&self.db_path, &snap_path).map_err(|e| {
            ChainError::DatabaseCreationError(redb::DatabaseError::Storage(redb::StorageError::Io(
                e,
            )))
        })?;
        tracing::info!(
            "Chain snapshot exported to {:?} (height {})",
            snap_path,
            height
        );
        Ok(snap_path)
    }
}

/// Chain statistics
#[derive(Debug, Clone)]
pub struct ChainStats {
    pub best_height: u64,
    pub best_hash: Hash,
    pub genesis_hash: Hash,
    /// On-disk size of the chain database in bytes (0 if unavailable)
    pub db_file_size_bytes: u64,
}

// Implement BlockchainReader trait for RPC access
impl coinject_rpc::BlockchainReader for ChainState {
    fn get_block_by_height(&self, height: u64) -> Result<Option<Block>, String> {
        self.get_block_by_height(height).map_err(|e| e.to_string())
    }

    fn get_block_by_hash(&self, hash: &Hash) -> Result<Option<Block>, String> {
        self.get_block_by_hash(hash).map_err(|e| e.to_string())
    }

    fn get_header_by_height(&self, height: u64) -> Result<Option<BlockHeader>, String> {
        self.get_header_by_height(height).map_err(|e| e.to_string())
    }

    /// O(1) read from chain metadata — default scans every header and blocks RPC under load.
    fn calculate_chain_work(&self, up_to_height: u64) -> Result<u64, String> {
        if let Ok(Some(cum)) = Self::read_cumulative_work_meta(&self.db) {
            return Ok(cum.min(u64::MAX as u128) as u64);
        }
        let mut total: u64 = 0;
        for h in 0..=up_to_height {
            if let Ok(Some(header)) = self.get_header_by_height(h) {
                total = total.saturating_add(header.work_score as u64);
            }
        }
        Ok(total)
    }

    fn best_cumulative_work_decimal(&self) -> Option<String> {
        Some(
            Self::read_cumulative_work_meta(&self.db)
                .ok()
                .flatten()
                .unwrap_or(0)
                .to_string(),
        )
    }

    fn total_minted_rewards_decimal(&self) -> Option<String> {
        Some(self.best_total_minted_rewards().to_string())
    }
}

// =============================================================================
// BlockProvider Adapter for Custom P2P
// =============================================================================
// This adapter allows the CPP network to query blocks from the canonical chain
// for serving sync requests to peers.

use coinject_network::cpp::BlockProvider;

/// Wrapper that implements BlockProvider for ChainState
///
/// This adapter bridges the node's chain storage (ChainState) with the
/// CPP network's block provider interface, enabling the network to serve
/// blocks to peers during sync.
///
/// CRITICAL: All block queries go through the canonical height index,
/// ensuring only best-chain blocks are served.
pub struct ChainBlockProvider {
    chain: std::sync::Arc<ChainState>,
    /// Lock-free tip height for hot-path sync serving.
    best_height_atomic: std::sync::Arc<std::sync::atomic::AtomicU64>,
}

impl ChainBlockProvider {
    /// Create new block provider wrapping a ChainState
    pub fn new(chain: std::sync::Arc<ChainState>) -> Self {
        let best_height_atomic = chain.best_height_atomic_ref();
        ChainBlockProvider {
            chain,
            best_height_atomic,
        }
    }
}

impl BlockProvider for ChainBlockProvider {
    fn get_block_by_height(&self, height: u64) -> Option<Block> {
        match self.chain.get_block_by_height(height) {
            Ok(block) => block,
            Err(e) => {
                eprintln!(
                    "[BlockProvider] Error fetching block at height {}: {}",
                    height, e
                );
                None
            }
        }
    }

    fn get_best_height(&self) -> u64 {
        self.best_height_atomic.load(Ordering::Acquire)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genesis::{create_genesis_block, GenesisConfig};

    #[tokio::test]
    async fn test_chain_initialization() {
        let temp_dir = std::env::temp_dir().join("coinject-chain-test-init");
        let _ = std::fs::remove_dir_all(&temp_dir);

        let genesis = create_genesis_block(GenesisConfig::default());
        let chain = ChainState::new(&temp_dir, &genesis, 512).unwrap();

        assert_eq!(chain.best_block_height().await, 0);
        assert_eq!(chain.genesis_hash(), genesis.header.hash());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_block_storage() {
        let temp_dir = std::env::temp_dir().join("coinject-chain-test-storage");
        let _ = std::fs::remove_dir_all(&temp_dir);

        let genesis = create_genesis_block(GenesisConfig::default());
        let chain = ChainState::new(&temp_dir, &genesis, 512).unwrap();

        // Retrieve genesis
        let retrieved = chain.get_block_by_height(0).unwrap().unwrap();
        assert_eq!(retrieved.header.height, 0);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    /// After a fork, `height_index` must not make `get_block_by_height` return a stale orphan.
    #[tokio::test]
    async fn height_index_serves_canonical_tip_after_index_poison() {
        use coinject_core::Hash;

        let temp_dir = std::env::temp_dir().join("coinject-chain-height-index-canonical");
        let _ = std::fs::remove_dir_all(&temp_dir);

        let genesis = create_genesis_block(GenesisConfig::default());
        let chain = ChainState::new(&temp_dir, &genesis, 64).unwrap();

        let g_hash = genesis.header.hash();
        let b1 = {
            let mut h = genesis.header.clone();
            h.height = 1;
            h.prev_hash = g_hash;
            h.nonce = 1;
            let block = Block {
                header: h,
                coinbase: genesis.coinbase.clone(),
                transactions: vec![],
                solution_reveal: genesis.solution_reveal.clone(),
            };
            chain.store_block(&block).await.unwrap();
            block
        };
        let b1_hash = b1.header.hash();

        let b2 = {
            let mut h = b1.header.clone();
            h.height = 2;
            h.prev_hash = b1_hash;
            h.nonce = 2;
            let block = Block {
                header: h,
                coinbase: genesis.coinbase.clone(),
                transactions: vec![],
                solution_reveal: genesis.solution_reveal.clone(),
            };
            chain.store_block(&block).await.unwrap();
            block
        };
        let b2_hash = b2.header.hash();

        // Poison height index at 1 with a fake hash (simulates pre-fix orphan mapping).
        let orphan_hash = Hash::from_bytes([0xEEu8; 32]);
        chain.set_height_index_entry(1, orphan_hash).unwrap();

        let at_one = chain.get_block_by_height(1).unwrap().unwrap();
        assert_eq!(
            at_one.header.hash(),
            b1_hash,
            "canonical walk must ignore stale height_index"
        );

        chain
            .rebuild_height_index_from_canonical_tip(b2_hash, 2)
            .unwrap();
        assert_eq!(
            chain.get_block_by_height(1).unwrap().unwrap().header.hash(),
            b1_hash
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
