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
