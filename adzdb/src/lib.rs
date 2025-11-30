//! # ADZDB: Append-only Deterministic Zero-copy Database
//!
//! A specialized storage engine for blockchain data, inspired by:
//! - **NuDB** (XRPL): Append-only data file, linear hashing, O(1) reads
//! - **TigerBeetle**: Deterministic operations, zero-copy structs, protocol-aware recovery
//!
//! ## Design Principles
//!
//! 1. **Append-only**: Data is never overwritten, only appended
//!    - Perfect for blockchain: blocks are immutable
//!    - Simplifies concurrency: readers never block writers
//!    - Enables crash recovery without complex journaling
//!
//! 2. **Deterministic**: All operations produce identical results
//!    - Same input → Same output (consensus-safe)
//!    - No background threads with unpredictable timing
//!    - No "compaction jitter" like RocksDB
//!
//! 3. **Zero-copy**: Minimize serialization overhead
//!    - Fixed-size headers mapped directly to structs
//!    - Memory-mapped I/O where possible
//!    - No JSON/protobuf parsing in hot path
//!
//! ## File Structure
//!
//! ```text
//! adzdb/
//! ├── adzdb.idx     # Index file (hash → offset mapping)
//! ├── adzdb.dat     # Data file (append-only block storage)
//! └── adzdb.meta    # Metadata (chain state, latest height)
//! ```
//!
//! ## Architecture
//!
//! ```text
//!                    ┌─────────────────────────────────────┐
//!                    │            ADZDB Engine             │
//!                    ├─────────────────────────────────────┤
//!                    │                                     │
//!   Write Path:      │  ┌─────────┐    ┌──────────────┐   │
//!   append_block()───┼─▶│ Compute │───▶│ Append to    │   │
//!                    │  │ Hash    │    │ Data File    │   │
//!                    │  └─────────┘    └──────┬───────┘   │
//!                    │                        │           │
//!                    │                        ▼           │
//!                    │               ┌──────────────┐     │
//!                    │               │ Update Index │     │
//!                    │               │ (Linear Hash)│     │
//!                    │               └──────────────┘     │
//!                    │                                     │
//!   Read Path:       │  ┌─────────┐    ┌──────────────┐   │
//!   get_block()──────┼─▶│ Hash    │───▶│ Index Lookup │   │
//!                    │  │ Key     │    │ O(1)         │   │
//!                    │  └─────────┘    └──────┬───────┘   │
//!                    │                        │           │
//!                    │                        ▼           │
//!                    │               ┌──────────────┐     │
//!                    │               │ Data File    │     │
//!                    │               │ Direct Read  │     │
//!                    │               └──────────────┘     │
//!                    │                                     │
//!                    └─────────────────────────────────────┘
//! ```

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::collections::HashMap;

pub mod index;
pub mod data_file;
pub mod recovery;

/// Magic bytes for ADZDB files
pub const MAGIC: &[u8; 4] = b"ADZB";

/// Current file format version
pub const VERSION: u32 = 1;

/// Default bucket count for index (must be power of 2)
pub const DEFAULT_BUCKETS: u32 = 1 << 16; // 65,536 buckets

/// Maximum value size (1 GB)
pub const MAX_VALUE_SIZE: u64 = 1 << 30;

/// Configuration for ADZDB
#[derive(Debug, Clone)]
pub struct Config {
    /// Base path for database files
    pub path: PathBuf,
    /// Number of index buckets (power of 2)
    pub bucket_count: u32,
    /// Target load factor before bucket split (0.0-1.0)
    pub load_factor: f64,
    /// Enable memory-mapped I/O for reads
    pub use_mmap: bool,
    /// Sync data to disk after each write
    pub sync_on_write: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            path: PathBuf::from("./adzdb"),
            bucket_count: DEFAULT_BUCKETS,
            load_factor: 0.5,
            use_mmap: true,
            sync_on_write: true,
        }
    }
}

impl Config {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            ..Default::default()
        }
    }
}

/// Error types for ADZDB operations
#[derive(Debug)]
pub enum Error {
    /// I/O error
    Io(io::Error),
    /// Key not found
    NotFound,
    /// Corrupt data detected
    Corruption(String),
    /// Value too large
    ValueTooLarge(u64),
    /// Database already exists
    AlreadyExists,
    /// Invalid configuration
    InvalidConfig(String),
    /// Hash mismatch (content-addressable violation)
    HashMismatch { expected: Hash, actual: Hash },
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(e) => write!(f, "I/O error: {}", e),
            Error::NotFound => write!(f, "Key not found"),
            Error::Corruption(msg) => write!(f, "Data corruption: {}", msg),
            Error::ValueTooLarge(size) => write!(f, "Value too large: {} bytes", size),
            Error::AlreadyExists => write!(f, "Database already exists"),
            Error::InvalidConfig(msg) => write!(f, "Invalid config: {}", msg),
            Error::HashMismatch { expected, actual } => {
                write!(f, "Hash mismatch: expected {:?}, got {:?}", expected, actual)
            }
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

/// 256-bit hash (SHA-256 or BLAKE3)
pub type Hash = [u8; 32];

/// Zero hash constant
pub const ZERO_HASH: Hash = [0u8; 32];

/// Block header - fixed 128 bytes for zero-copy access
/// Aligned to cache line for optimal memory access
#[repr(C, align(64))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockHeader {
    /// Block height (0 = genesis)
    pub height: u64,
    /// Block hash (content-addressable key)
    pub hash: Hash,
    /// Previous block hash
    pub prev_hash: Hash,
    /// Merkle root of transactions
    pub merkle_root: Hash,
    /// Unix timestamp (seconds since epoch)
    pub timestamp: u64,
    /// Difficulty target
    pub difficulty: u64,
    /// Nonce used for mining
    pub nonce: u64,
    /// Problem type: 0=SAT, 1=SubsetSum, 2=TSP
    pub problem_type: u8,
    /// Reserved for future use
    pub _reserved: [u8; 7],
}

impl BlockHeader {
    pub const SIZE: usize = 128;
    
    /// Serialize header to bytes (zero-copy - just transmute)
    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        // SAFETY: BlockHeader is repr(C) with known layout
        unsafe { std::mem::transmute_copy(self) }
    }
    
    /// Deserialize header from bytes (zero-copy)
    pub fn from_bytes(bytes: &[u8; Self::SIZE]) -> Self {
        // SAFETY: BlockHeader is repr(C) with known layout
        unsafe { std::mem::transmute_copy(bytes) }
    }
}

/// Index entry - maps hash to data file offset
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct IndexEntry {
    /// Key hash (first 8 bytes for quick comparison)
    pub key_prefix: u64,
    /// Full key hash
    pub key: Hash,
    /// Offset in data file
    pub offset: u64,
    /// Size of value in data file
    pub size: u32,
    /// Flags (deleted, etc.)
    pub flags: u32,
}

impl IndexEntry {
    pub const SIZE: usize = 56;
    pub const FLAG_DELETED: u32 = 1 << 0;
}

/// The main ADZDB database handle
pub struct Database {
    config: Config,
    /// Index: hash → offset
    index: index::LinearHashIndex,
    /// Data file handle
    data_file: data_file::AppendOnlyFile,
    /// Current state
    state: DatabaseState,
}

/// Runtime state of the database
#[derive(Debug, Clone)]
pub struct DatabaseState {
    /// Number of entries
    pub entry_count: u64,
    /// Total data size in bytes
    pub data_size: u64,
    /// Latest block height
    pub latest_height: u64,
    /// Latest block hash
    pub latest_hash: Hash,
    /// Genesis hash (immutable after creation)
    pub genesis_hash: Hash,
}

impl Database {
    /// Create a new database
    pub fn create(config: Config) -> Result<Self> {
        // Ensure directory exists
        std::fs::create_dir_all(&config.path)?;
        
        let index_path = config.path.join("adzdb.idx");
        let data_path = config.path.join("adzdb.dat");
        
        // Check if already exists
        if index_path.exists() || data_path.exists() {
            return Err(Error::AlreadyExists);
        }
        
        // Create index
        let index = index::LinearHashIndex::create(&index_path, config.bucket_count)?;
        
        // Create data file
        let data_file = data_file::AppendOnlyFile::create(&data_path)?;
        
        let state = DatabaseState {
            entry_count: 0,
            data_size: 0,
            latest_height: 0,
            latest_hash: ZERO_HASH,
            genesis_hash: ZERO_HASH,
        };
        
        println!("🗄️  ADZDB created at {:?}", config.path);
        
        Ok(Self {
            config,
            index,
            data_file,
            state,
        })
    }
    
    /// Open an existing database
    pub fn open(config: Config) -> Result<Self> {
        let index_path = config.path.join("adzdb.idx");
        let data_path = config.path.join("adzdb.dat");
        
        // Open index
        let index = index::LinearHashIndex::open(&index_path)?;
        
        // Open data file
        let data_file = data_file::AppendOnlyFile::open(&data_path)?;
        
        // Recover state
        let state = Self::recover_state(&index, &data_file)?;
        
        println!("🗄️  ADZDB opened: {} entries, height {}", 
            state.entry_count, state.latest_height);
        
        Ok(Self {
            config,
            index,
            data_file,
            state,
        })
    }
    
    /// Recover database state from files (Protocol-Aware Recovery)
    fn recover_state(
        index: &index::LinearHashIndex,
        data_file: &data_file::AppendOnlyFile,
    ) -> Result<DatabaseState> {
        // For now, just return default state
        // Full implementation would scan data file
        Ok(DatabaseState {
            entry_count: index.entry_count(),
            data_size: data_file.size(),
            latest_height: 0,
            latest_hash: ZERO_HASH,
            genesis_hash: ZERO_HASH,
        })
    }
    
    /// Store a block (content-addressable)
    /// Key is derived from content hash - cannot store under arbitrary key
    pub fn put_block(&mut self, header: &BlockHeader, data: &[u8]) -> Result<Hash> {
        // Verify content-addressable property
        let computed_hash = Self::compute_hash(header, data);
        if computed_hash != header.hash {
            return Err(Error::HashMismatch {
                expected: header.hash,
                actual: computed_hash,
            });
        }
        
        // Check if already exists (deduplication)
        if self.index.contains(&header.hash)? {
            return Ok(header.hash);
        }
        
        // Append to data file
        let offset = self.data_file.append(header, data)?;
        
        // Update index
        let entry = IndexEntry {
            key_prefix: u64::from_le_bytes(header.hash[..8].try_into().unwrap()),
            key: header.hash,
            offset,
            size: (BlockHeader::SIZE + data.len()) as u32,
            flags: 0,
        };
        self.index.insert(entry)?;
        
        // Update state
        self.state.entry_count += 1;
        self.state.data_size += entry.size as u64;
        
        if header.height > self.state.latest_height {
            self.state.latest_height = header.height;
            self.state.latest_hash = header.hash;
        }
        
        if header.height == 0 {
            self.state.genesis_hash = header.hash;
        }
        
        // Sync if configured
        if self.config.sync_on_write {
            self.data_file.sync()?;
        }
        
        Ok(header.hash)
    }
    
    /// Get a block by hash (O(1) lookup)
    pub fn get_block(&self, hash: &Hash) -> Result<(BlockHeader, Vec<u8>)> {
        // Index lookup (1 I/O)
        let entry = self.index.get(hash)?
            .ok_or(Error::NotFound)?;
        
        // Data file read (1 I/O) - "Two-Fetch Guarantee"
        let (header, data) = self.data_file.read(entry.offset, entry.size as usize)?;
        
        // Verify hash (content-addressable integrity check)
        let computed = Self::compute_hash(&header, &data);
        if computed != *hash {
            return Err(Error::Corruption(format!(
                "Hash mismatch at offset {}", entry.offset
            )));
        }
        
        Ok((header, data))
    }
    
    /// Get block by height (requires height index)
    pub fn get_block_by_height(&self, height: u64) -> Result<(BlockHeader, Vec<u8>)> {
        // TODO: Implement height-to-hash index
        // For now, this is O(n) - needs optimization
        Err(Error::NotFound)
    }
    
    /// Check if a hash exists (O(1))
    pub fn contains(&self, hash: &Hash) -> Result<bool> {
        self.index.contains(hash)
    }
    
    /// Compute hash of block (deterministic)
    fn compute_hash(header: &BlockHeader, data: &[u8]) -> Hash {
        use sha2::{Sha256, Digest};
        
        let mut hasher = Sha256::new();
        hasher.update(&header.to_bytes());
        hasher.update(data);
        
        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        hash
    }
    
    /// Get database statistics
    pub fn stats(&self) -> DatabaseState {
        self.state.clone()
    }
    
    /// Flush all pending writes to disk
    pub fn sync(&mut self) -> Result<()> {
        self.data_file.sync()?;
        self.index.sync()?;
        Ok(())
    }
}

// Placeholder modules - would be in separate files
pub mod index {
    use super::*;
    
    /// Linear hash index (NuDB-inspired)
    pub struct LinearHashIndex {
        // Placeholder
        entries: HashMap<Hash, IndexEntry>,
    }
    
    impl LinearHashIndex {
        pub fn create(_path: &Path, _buckets: u32) -> Result<Self> {
            Ok(Self { entries: HashMap::new() })
        }
        
        pub fn open(_path: &Path) -> Result<Self> {
            Ok(Self { entries: HashMap::new() })
        }
        
        pub fn insert(&mut self, entry: IndexEntry) -> Result<()> {
            self.entries.insert(entry.key, entry);
            Ok(())
        }
        
        pub fn get(&self, key: &Hash) -> Result<Option<IndexEntry>> {
            Ok(self.entries.get(key).copied())
        }
        
        pub fn contains(&self, key: &Hash) -> Result<bool> {
            Ok(self.entries.contains_key(key))
        }
        
        pub fn entry_count(&self) -> u64 {
            self.entries.len() as u64
        }
        
        pub fn sync(&self) -> Result<()> {
            Ok(())
        }
    }
}

pub mod data_file {
    use super::*;
    
    /// Append-only data file (NuDB-inspired)
    pub struct AppendOnlyFile {
        size: u64,
        data: Vec<u8>,
    }
    
    impl AppendOnlyFile {
        pub fn create(_path: &Path) -> Result<Self> {
            Ok(Self { size: 0, data: Vec::new() })
        }
        
        pub fn open(_path: &Path) -> Result<Self> {
            Ok(Self { size: 0, data: Vec::new() })
        }
        
        pub fn append(&mut self, header: &BlockHeader, data: &[u8]) -> Result<u64> {
            let offset = self.size;
            self.data.extend_from_slice(&header.to_bytes());
            self.data.extend_from_slice(data);
            self.size += (BlockHeader::SIZE + data.len()) as u64;
            Ok(offset)
        }
        
        pub fn read(&self, offset: u64, size: usize) -> Result<(BlockHeader, Vec<u8>)> {
            let start = offset as usize;
            let header_bytes: [u8; BlockHeader::SIZE] = 
                self.data[start..start + BlockHeader::SIZE].try_into()
                    .map_err(|_| Error::Corruption("Invalid header".to_string()))?;
            
            let header = BlockHeader::from_bytes(&header_bytes);
            let data_start = start + BlockHeader::SIZE;
            let data_end = start + size;
            let data = self.data[data_start..data_end].to_vec();
            
            Ok((header, data))
        }
        
        pub fn size(&self) -> u64 {
            self.size
        }
        
        pub fn sync(&self) -> Result<()> {
            Ok(())
        }
    }
}

pub mod recovery {
    //! Protocol-Aware Recovery (TigerBeetle-inspired)
    //! 
    //! When local storage is corrupted, request correct data from P2P network.
    //! The consensus protocol "knows" what data should be and can heal storage.
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_block_header_size() {
        assert_eq!(std::mem::size_of::<BlockHeader>(), BlockHeader::SIZE);
    }
    
    #[test]
    fn test_block_header_roundtrip() {
        let header = BlockHeader {
            height: 42,
            hash: [1u8; 32],
            prev_hash: [2u8; 32],
            merkle_root: [3u8; 32],
            timestamp: 1234567890,
            difficulty: 0x0000ffff,
            nonce: 999,
            problem_type: 1,
            _reserved: [0u8; 7],
        };
        
        let bytes = header.to_bytes();
        let recovered = BlockHeader::from_bytes(&bytes);
        
        assert_eq!(header, recovered);
    }
}

