//! ADZDB-backed Chain State Manager
//!
//! Alternative to redb-based chain.rs, using the custom ADZDB database
//! designed specifically for blockchain data.
//!
//! Enable with: --features adzdb

#![allow(clippy::duplicated_attributes, dead_code)]
#![cfg(feature = "adzdb")]

use adzdb::{Config as AdzConfig, Database as AdzDatabase, Error as AdzError};
use coinject_core::{Block, BlockHeader, Hash};
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;

#[derive(Error, Debug)]
pub enum ChainError {
    #[error("ADZDB error: {0}")]
    AdzdbError(String),
    #[error("Block not found")]
    BlockNotFound,
    #[error("Invalid block height")]
    InvalidHeight,
    #[error("Serialization error: {0}")]
    SerializationError(#[from] bincode::Error),
    #[error("Genesis block mismatch")]
    GenesisMismatch,
}

impl From<AdzError> for ChainError {
    fn from(e: AdzError) -> Self {
        ChainError::AdzdbError(e.to_string())
    }
}

/// ADZDB-backed chain state manager
pub struct AdzdbChainState {
    /// ADZDB database instance
    db: Arc<RwLock<AdzDatabase>>,
    /// Best block height (cached)
    best_height: Arc<RwLock<u64>>,
    /// Best block hash (cached)
    best_hash: Arc<RwLock<Hash>>,
    /// Genesis hash
    genesis_hash: Hash,
}

impl AdzdbChainState {
    /// Create or open chain state database using ADZDB
    pub fn new<P: AsRef<Path>>(path: P, genesis_block: &Block) -> Result<Self, ChainError> {
        // If path is a file (like chain.db), use its parent directory
        // Otherwise use the path directly
        let base_path = if path.as_ref().is_file() {
            path.as_ref()
                .parent()
                .ok_or_else(|| ChainError::AdzdbError("Cannot get parent directory".to_string()))?
        } else {
            path.as_ref()
        };

        let adzdb_path = base_path.join("adzdb");
        let config = AdzConfig::new(&adzdb_path);

        let genesis_hash = genesis_block.header.hash();

        // Create or open database
        let mut db = AdzDatabase::open_or_create(config)?;

        // Check if genesis exists
        if db.contains_height(0) {
            // Verify genesis hash matches
            let stored_genesis_hash = db.get_hash_by_height(0)?;
            if stored_genesis_hash != *genesis_hash.as_bytes() {
                return Err(ChainError::GenesisMismatch);
            }
        } else {
            // Store genesis block
            let block_bytes = bincode::serialize(genesis_block)?;
            db.put(genesis_hash.as_bytes(), 0, &block_bytes)?;
            println!("🗄️  ADZDB: Stored genesis block");
        }

        // Load best height and hash
        let best_height = db.latest_height();
        let best_hash_bytes = if best_height > 0 {
            db.get_hash_by_height(best_height)?
        } else {
            *genesis_hash.as_bytes()
        };
        let best_hash = Hash::from_bytes(best_hash_bytes);

        println!(
            "🗄️  ADZDB ChainState: height={}, entries={}",
            best_height,
            db.entry_count()
        );

        Ok(AdzdbChainState {
            db: Arc::new(RwLock::new(db)),
            best_height: Arc::new(RwLock::new(best_height)),
            best_hash: Arc::new(RwLock::new(best_hash)),
            genesis_hash,
        })
    }

    /// Store a block and update best chain if needed
    pub async fn store_block(&self, block: &Block) -> Result<bool, ChainError> {
        let block_hash = block.header.hash();
        let block_height = block.header.height;

        // Serialize block
        let block_bytes = bincode::serialize(block)?;

        // Store in ADZDB
        {
            let mut db = self.db.write().await;
            db.put(block_hash.as_bytes(), block_height, &block_bytes)?;
        }

        // Check if this extends the best chain
        let current_best_height = *self.best_height.read().await;

        if block_height > current_best_height {
            // New best block
            *self.best_height.write().await = block_height;
            *self.best_hash.write().await = block_hash;

            println!(
                "🗄️  ADZDB: New best block height={} hash={:?}",
                block_height, block_hash
            );
            return Ok(true);
        }

        Ok(false)
    }

    /// Get block by hash (sync for compatibility with ChainState)
    pub fn get_block_by_hash(&self, hash: &Hash) -> Result<Option<Block>, ChainError> {
        let db = futures::executor::block_on(self.db.read());

        match db.get(hash.as_bytes()) {
            Ok(bytes) => {
                let block: Block = bincode::deserialize(&bytes)?;
                Ok(Some(block))
            }
            Err(AdzError::NotFound) => Ok(None),
            Err(e) => Err(ChainError::from(e)),
        }
    }

    /// Get block by height
    pub fn get_block_by_height(&self, height: u64) -> Result<Option<Block>, ChainError> {
        // Note: This is sync because it's called from sync contexts
        // In production, would use tokio::runtime::Handle
        let db = futures::executor::block_on(self.db.read());

        match db.get_by_height(height) {
            Ok(bytes) => {
                let block: Block = bincode::deserialize(&bytes)?;
                Ok(Some(block))
            }
            Err(AdzError::NotFound) => Ok(None),
            Err(e) => Err(ChainError::from(e)),
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

    /// Check if a block exists (sync for compatibility with ChainState)
    pub fn has_block(&self, hash: &Hash) -> Result<bool, ChainError> {
        let db = futures::executor::block_on(self.db.read());
        Ok(db.contains(hash.as_bytes()))
    }

    /// Find common ancestor between current best chain and a target block
    pub async fn find_common_ancestor(
        &self,
        target_hash: &Hash,
        target_height: u64,
    ) -> Result<Option<(Hash, u64)>, ChainError> {
        let current_best_hash = self.best_block_hash().await;
        let current_best_height = self.best_block_height().await;

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

    /// Get chain path from start to end (sync for compatibility)
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

    /// Update best chain to new block (after reorganization)
    pub async fn update_best_chain(
        &self,
        new_best_hash: Hash,
        new_best_height: u64,
    ) -> Result<(), ChainError> {
        *self.best_height.write().await = new_best_height;
        *self.best_hash.write().await = new_best_hash;

        // Sync database
        {
            let mut db = self.db.write().await;
            db.sync()?;
        }

        println!(
            "🗄️  ADZDB: Chain reorganized to height={} hash={:?}",
            new_best_height, new_best_hash
        );
        Ok(())
    }

    /// Get chain statistics
    pub async fn get_stats(&self) -> ChainStats {
        let db = self.db.read().await;
        ChainStats {
            best_height: self.best_block_height().await,
            best_hash: self.best_block_hash().await,
            genesis_hash: self.genesis_hash,
            entry_count: db.entry_count(),
            data_size: db.stats().data_size,
        }
    }
}

/// Chain statistics (extended for ADZDB)
#[derive(Debug, Clone)]
pub struct ChainStats {
    pub best_height: u64,
    pub best_hash: Hash,
    pub genesis_hash: Hash,
    pub entry_count: u64,
    pub data_size: u64,
}

// BlockProvider implementation for CPP network sync
use coinject_network::cpp::BlockProvider;

/// Wrapper that implements BlockProvider for AdzdbChainState
///
/// This adapter bridges the node's chain storage (AdzdbChainState) with the
/// CPP network's block provider interface, enabling the network to serve
/// blocks to peers during sync.
pub struct ChainBlockProvider {
    chain: std::sync::Arc<AdzdbChainState>,
    /// Cached best height (updated on block storage)
    best_height: std::sync::Arc<tokio::sync::RwLock<u64>>,
}

impl ChainBlockProvider {
    /// Create new block provider wrapping a ChainState
    pub fn new(chain: std::sync::Arc<AdzdbChainState>) -> Self {
        let best_height = chain.best_height_ref();
        ChainBlockProvider { chain, best_height }
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
        // Use blocking read since BlockProvider is sync
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async { *self.best_height.read().await })
        })
    }
}

// Implement BlockchainReader trait for RPC access
impl coinject_rpc::BlockchainReader for AdzdbChainState {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genesis::{create_genesis_block, GenesisConfig};

    #[tokio::test]
    async fn test_adzdb_chain_initialization() {
        let temp_dir = std::env::temp_dir().join("coinject-adzdb-chain-test");
        let _ = std::fs::remove_dir_all(&temp_dir);

        let genesis = create_genesis_block(GenesisConfig::default());
        let chain = AdzdbChainState::new(&temp_dir, &genesis).unwrap();

        assert_eq!(chain.best_block_height().await, 0);
        assert_eq!(chain.genesis_hash(), genesis.header.hash());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
