// Chain State Manager
// Block storage, best chain tracking, and chain reorganization

use coinject_core::{Block, BlockHeader, Hash};
use redb::{Database, TableDefinition};
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;

// Table definitions for redb (using fixed-size arrays for hash keys, strings for metadata keys)
const BLOCKS_TABLE: TableDefinition<&[u8; 32], &[u8]> = TableDefinition::new("blocks");
const METADATA_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("metadata");
const HEIGHT_INDEX_TABLE: TableDefinition<u64, &[u8; 32]> = TableDefinition::new("height_index");

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
}

/// Chain state manager handling block storage and retrieval
pub struct ChainState {
    /// redb database for block storage
    db: Arc<Database>,
    /// Best block height
    best_height: Arc<RwLock<u64>>,
    /// Best block hash
    best_hash: Arc<RwLock<Hash>>,
    /// Genesis hash for network verification
    genesis_hash: Hash,
}

impl ChainState {
    /// Create or open chain state database
    pub fn new<P: AsRef<Path>>(path: P, genesis_block: &Block) -> Result<Self, ChainError> {
        let db = Database::create(path)?;
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
            }
            write_txn.commit()?;

            Self::store_block_raw(&db, genesis_block)?;
        }

        // Load best height and hash
        let read_txn = db.begin_read()?;
        let (best_height, best_hash) = {
            let table = read_txn.open_table(METADATA_TABLE)?;

            let height_bytes = table
                .get("best_height")?
                .map(|v| v.value().to_vec());

            let hash_bytes = table
                .get("best_hash")?
                .map(|v| v.value().to_vec());

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

        Ok(ChainState {
            db,
            best_height: Arc::new(RwLock::new(best_height)),
            best_hash: Arc::new(RwLock::new(best_hash)),
            genesis_hash,
        })
    }

    /// Store a block in the database
    fn store_block_raw(db: &Arc<Database>, block: &Block) -> Result<(), ChainError> {
        let block_bytes = bincode::serialize(block)?;
        let hash = block.header.hash();
        let height = block.header.height;

        let write_txn = db.begin_write()?;
        {
            // Store by hash
            let mut blocks_table = write_txn.open_table(BLOCKS_TABLE)?;
            blocks_table.insert(hash.as_bytes(), block_bytes.as_slice())?;

            // Store hash by height (for quick height lookups)
            let mut height_table = write_txn.open_table(HEIGHT_INDEX_TABLE)?;
            height_table.insert(height, hash.as_bytes())?;
        }
        write_txn.commit()?;

        Ok(())
    }

    /// Store a block and update best chain if needed
    pub async fn store_block(&self, block: &Block) -> Result<bool, ChainError> {
        let block_hash = block.header.hash();
        let block_height = block.header.height;

        // Store the block
        Self::store_block_raw(&self.db, block)?;

        // Check if this extends the best chain
        let current_best_height = *self.best_height.read().await;

        if block_height > current_best_height {
            // New best block
            *self.best_height.write().await = block_height;
            *self.best_hash.write().await = block_hash;

            let write_txn = self.db.begin_write()?;
            {
                let mut table = write_txn.open_table(METADATA_TABLE)?;
                table.insert("best_height", block_height.to_le_bytes().as_ref())?;
                table.insert("best_hash", block_hash.as_bytes() as &[u8])?;
            }
            write_txn.commit()?;

            println!("New best block: height={} hash={:?}", block_height, block_hash);
            return Ok(true);
        }

        Ok(false)
    }

    /// Get block by hash
    pub fn get_block_by_hash(&self, hash: &Hash) -> Result<Option<Block>, ChainError> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(BLOCKS_TABLE)?;

        match table.get(hash.as_bytes())? {
            Some(bytes_ref) => {
                let block: Block = bincode::deserialize(bytes_ref.value())?;
                Ok(Some(block))
            }
            None => Ok(None),
        }
    }

    /// Get block by height
    pub fn get_block_by_height(&self, height: u64) -> Result<Option<Block>, ChainError> {
        let read_txn = self.db.begin_read()?;
        let height_table = read_txn.open_table(HEIGHT_INDEX_TABLE)?;

        match height_table.get(height)? {
            Some(hash_bytes_ref) => {
                let hash = Hash::from_bytes(*hash_bytes_ref.value());
                drop(read_txn);
                self.get_block_by_hash(&hash)
            }
            None => Ok(None),
        }
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
        ChainStats {
            best_height: self.best_block_height().await,
            best_hash: self.best_block_hash().await,
            genesis_hash: self.genesis_hash,
        }
    }

    /// Find common ancestor between current best chain and a target block
    /// Returns (common_ancestor_hash, common_ancestor_height)
    pub async fn find_common_ancestor(&self, target_hash: &Hash, target_height: u64) -> Result<Option<(Hash, u64)>, ChainError> {
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
            if let Some(block) = self.get_block_by_hash(&their_hash)? {
                their_hash = block.header.prev_hash;
                their_height -= 1;
            } else {
                return Ok(None);
            }
        }

        // Now both at same height, walk back until we find common ancestor
        while our_height > 0 && our_hash != their_hash {
            if let Some(our_block) = self.get_block_by_hash(&our_hash)? {
                our_hash = our_block.header.prev_hash;
            } else {
                return Ok(None);
            }

            if let Some(their_block) = self.get_block_by_hash(&their_hash)? {
                their_hash = their_block.header.prev_hash;
            } else {
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

    /// Get chain path from start_hash to end_hash (inclusive)
    /// Returns blocks in order from start to end
    pub fn get_chain_path(&self, start_hash: &Hash, start_height: u64, end_hash: &Hash, end_height: u64) -> Result<Vec<Block>, ChainError> {
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
    pub async fn prepare_reorganization(&self, new_best_hash: &Hash, new_best_height: u64) -> Result<(Vec<Block>, Vec<Block>), ChainError> {
        let current_best_hash = self.best_block_hash().await;
        let current_best_height = self.best_block_height().await;

        // Find common ancestor
        let (common_hash, common_height) = match self.find_common_ancestor(new_best_hash, new_best_height).await? {
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
    pub async fn update_best_chain(&self, new_best_hash: Hash, new_best_height: u64) -> Result<(), ChainError> {
        *self.best_height.write().await = new_best_height;
        *self.best_hash.write().await = new_best_hash;

        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(METADATA_TABLE)?;
            table.insert("best_height", new_best_height.to_le_bytes().as_ref())?;
            table.insert("best_hash", new_best_hash.as_bytes() as &[u8])?;
        }
        write_txn.commit()?;

        println!("🔄 Chain reorganized: new best block height={} hash={:?}", new_best_height, new_best_hash);
        Ok(())
    }
}

/// Chain statistics
#[derive(Debug, Clone)]
pub struct ChainStats {
    pub best_height: u64,
    pub best_hash: Hash,
    pub genesis_hash: Hash,
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
        let chain = ChainState::new(&temp_dir, &genesis).unwrap();

        assert_eq!(chain.best_block_height().await, 0);
        assert_eq!(chain.genesis_hash(), genesis.header.hash());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_block_storage() {
        let temp_dir = std::env::temp_dir().join("coinject-chain-test-storage");
        let _ = std::fs::remove_dir_all(&temp_dir);

        let genesis = create_genesis_block(GenesisConfig::default());
        let chain = ChainState::new(&temp_dir, &genesis).unwrap();

        // Retrieve genesis
        let retrieved = chain.get_block_by_height(0).unwrap().unwrap();
        assert_eq!(retrieved.header.height, 0);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
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
}
