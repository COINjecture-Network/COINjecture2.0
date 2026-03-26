//! ADZDB-backed Account State Manager
//!
//! Alternative to redb-based accounts.rs, using a simple file-based key-value store
//! that avoids Windows file locking issues.
//!
//! Enable with: --features adzdb

#![allow(clippy::duplicated_attributes)]
#![cfg(feature = "adzdb")]

use coinject_core::{Address, Balance};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

/// Magic bytes for state file
const STATE_MAGIC: &[u8; 4] = b"ADST";

/// Current file format version
const STATE_VERSION: u32 = 1;

/// ADZDB-backed account state manager
///
/// Uses in-memory HashMaps with atomic file persistence.
/// No file locking - state is loaded entirely into memory.
pub struct AdzdbAccountState {
    /// Path to state directory
    path: PathBuf,
    /// In-memory balance storage
    balances: Arc<RwLock<HashMap<Address, Balance>>>,
    /// In-memory nonce storage
    nonces: Arc<RwLock<HashMap<Address, u64>>>,
}

impl AdzdbAccountState {
    /// Open or create the state database
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, StateError> {
        let path = path.as_ref();

        // If path points to a file, use its parent directory
        let dir_path = if path.is_file() || path.extension().is_some() {
            path.parent()
                .ok_or_else(|| StateError::IoError("Cannot get parent directory".to_string()))?
                .to_path_buf()
        } else {
            path.to_path_buf()
        };

        // Create state subdirectory
        let state_dir = dir_path.join("adzdb_state");
        fs::create_dir_all(&state_dir)
            .map_err(|e| StateError::IoError(format!("Failed to create state directory: {}", e)))?;

        let balances_path = state_dir.join("balances.dat");
        let nonces_path = state_dir.join("nonces.dat");

        // Load existing data or create empty
        let balances = if balances_path.exists() {
            Self::load_balances(&balances_path)?
        } else {
            HashMap::new()
        };

        let nonces = if nonces_path.exists() {
            Self::load_nonces(&nonces_path)?
        } else {
            HashMap::new()
        };

        println!(
            "🗄️  ADZDB AccountState: {} balances, {} nonces loaded",
            balances.len(),
            nonces.len()
        );

        Ok(AdzdbAccountState {
            path: state_dir,
            balances: Arc::new(RwLock::new(balances)),
            nonces: Arc::new(RwLock::new(nonces)),
        })
    }

    /// Load balances from file
    fn load_balances(path: &Path) -> Result<HashMap<Address, Balance>, StateError> {
        let file = File::open(path)
            .map_err(|e| StateError::IoError(format!("Failed to open balances file: {}", e)))?;
        let mut reader = BufReader::new(file);

        // Read and verify header
        let mut magic = [0u8; 4];
        reader
            .read_exact(&mut magic)
            .map_err(|e| StateError::IoError(format!("Failed to read magic bytes: {}", e)))?;

        if &magic != STATE_MAGIC {
            return Err(StateError::CorruptionError(
                "Invalid magic bytes in balances file".to_string(),
            ));
        }

        let mut version_bytes = [0u8; 4];
        reader
            .read_exact(&mut version_bytes)
            .map_err(|e| StateError::IoError(format!("Failed to read version: {}", e)))?;
        let _version = u32::from_le_bytes(version_bytes);

        // Read entry count
        let mut count_bytes = [0u8; 8];
        reader
            .read_exact(&mut count_bytes)
            .map_err(|e| StateError::IoError(format!("Failed to read entry count: {}", e)))?;
        let count = u64::from_le_bytes(count_bytes);

        // Read entries: [32-byte address][16-byte balance (u128)]
        let mut balances = HashMap::with_capacity(count as usize);
        for _ in 0..count {
            let mut addr_bytes = [0u8; 32];
            let mut balance_bytes = [0u8; 16];

            reader
                .read_exact(&mut addr_bytes)
                .map_err(|e| StateError::IoError(format!("Failed to read address: {}", e)))?;
            reader
                .read_exact(&mut balance_bytes)
                .map_err(|e| StateError::IoError(format!("Failed to read balance: {}", e)))?;

            let address = Address::from_bytes(addr_bytes);
            let balance = u128::from_le_bytes(balance_bytes);
            balances.insert(address, balance);
        }

        Ok(balances)
    }

    /// Load nonces from file
    fn load_nonces(path: &Path) -> Result<HashMap<Address, u64>, StateError> {
        let file = File::open(path)
            .map_err(|e| StateError::IoError(format!("Failed to open nonces file: {}", e)))?;
        let mut reader = BufReader::new(file);

        // Read and verify header
        let mut magic = [0u8; 4];
        reader
            .read_exact(&mut magic)
            .map_err(|e| StateError::IoError(format!("Failed to read magic bytes: {}", e)))?;

        if &magic != STATE_MAGIC {
            return Err(StateError::CorruptionError(
                "Invalid magic bytes in nonces file".to_string(),
            ));
        }

        let mut version_bytes = [0u8; 4];
        reader
            .read_exact(&mut version_bytes)
            .map_err(|e| StateError::IoError(format!("Failed to read version: {}", e)))?;
        let _version = u32::from_le_bytes(version_bytes);

        // Read entry count
        let mut count_bytes = [0u8; 8];
        reader
            .read_exact(&mut count_bytes)
            .map_err(|e| StateError::IoError(format!("Failed to read entry count: {}", e)))?;
        let count = u64::from_le_bytes(count_bytes);

        // Read entries: [32-byte address][8-byte nonce (u64)]
        let mut nonces = HashMap::with_capacity(count as usize);
        for _ in 0..count {
            let mut addr_bytes = [0u8; 32];
            let mut nonce_bytes = [0u8; 8];

            reader
                .read_exact(&mut addr_bytes)
                .map_err(|e| StateError::IoError(format!("Failed to read address: {}", e)))?;
            reader
                .read_exact(&mut nonce_bytes)
                .map_err(|e| StateError::IoError(format!("Failed to read nonce: {}", e)))?;

            let address = Address::from_bytes(addr_bytes);
            let nonce = u64::from_le_bytes(nonce_bytes);
            nonces.insert(address, nonce);
        }

        Ok(nonces)
    }

    /// Save balances to file (atomic write via temp file + rename)
    fn save_balances(&self) -> Result<(), StateError> {
        let balances = self.balances.read().map_err(|_| {
            StateError::LockError("Failed to acquire balances read lock".to_string())
        })?;

        let temp_path = self.path.join("balances.dat.tmp");
        let final_path = self.path.join("balances.dat");

        {
            let file = File::create(&temp_path).map_err(|e| {
                StateError::IoError(format!("Failed to create temp balances file: {}", e))
            })?;
            let mut writer = BufWriter::new(file);

            // Write header
            writer
                .write_all(STATE_MAGIC)
                .map_err(|e| StateError::IoError(format!("Failed to write magic bytes: {}", e)))?;
            writer
                .write_all(&STATE_VERSION.to_le_bytes())
                .map_err(|e| StateError::IoError(format!("Failed to write version: {}", e)))?;
            writer
                .write_all(&(balances.len() as u64).to_le_bytes())
                .map_err(|e| StateError::IoError(format!("Failed to write entry count: {}", e)))?;

            // Write entries
            for (address, balance) in balances.iter() {
                writer
                    .write_all(address.as_bytes())
                    .map_err(|e| StateError::IoError(format!("Failed to write address: {}", e)))?;
                writer
                    .write_all(&balance.to_le_bytes())
                    .map_err(|e| StateError::IoError(format!("Failed to write balance: {}", e)))?;
            }

            writer.flush().map_err(|e| {
                StateError::IoError(format!("Failed to flush balances file: {}", e))
            })?;
        }

        // Atomic rename
        fs::rename(&temp_path, &final_path)
            .map_err(|e| StateError::IoError(format!("Failed to rename balances file: {}", e)))?;

        Ok(())
    }

    /// Save nonces to file (atomic write via temp file + rename)
    fn save_nonces(&self) -> Result<(), StateError> {
        let nonces = self
            .nonces
            .read()
            .map_err(|_| StateError::LockError("Failed to acquire nonces read lock".to_string()))?;

        let temp_path = self.path.join("nonces.dat.tmp");
        let final_path = self.path.join("nonces.dat");

        {
            let file = File::create(&temp_path).map_err(|e| {
                StateError::IoError(format!("Failed to create temp nonces file: {}", e))
            })?;
            let mut writer = BufWriter::new(file);

            // Write header
            writer
                .write_all(STATE_MAGIC)
                .map_err(|e| StateError::IoError(format!("Failed to write magic bytes: {}", e)))?;
            writer
                .write_all(&STATE_VERSION.to_le_bytes())
                .map_err(|e| StateError::IoError(format!("Failed to write version: {}", e)))?;
            writer
                .write_all(&(nonces.len() as u64).to_le_bytes())
                .map_err(|e| StateError::IoError(format!("Failed to write entry count: {}", e)))?;

            // Write entries
            for (address, nonce) in nonces.iter() {
                writer
                    .write_all(address.as_bytes())
                    .map_err(|e| StateError::IoError(format!("Failed to write address: {}", e)))?;
                writer
                    .write_all(&nonce.to_le_bytes())
                    .map_err(|e| StateError::IoError(format!("Failed to write nonce: {}", e)))?;
            }

            writer
                .flush()
                .map_err(|e| StateError::IoError(format!("Failed to flush nonces file: {}", e)))?;
        }

        // Atomic rename
        fs::rename(&temp_path, &final_path)
            .map_err(|e| StateError::IoError(format!("Failed to rename nonces file: {}", e)))?;

        Ok(())
    }

    /// Get account balance
    pub fn get_balance(&self, address: &Address) -> Balance {
        match self.balances.read() {
            Ok(balances) => *balances.get(address).unwrap_or(&0),
            Err(_) => 0,
        }
    }

    /// Set account balance
    pub fn set_balance(&self, address: &Address, balance: Balance) -> Result<(), StateError> {
        {
            let mut balances = self.balances.write().map_err(|_| {
                StateError::LockError("Failed to acquire balances write lock".to_string())
            })?;
            balances.insert(*address, balance);
        }
        self.save_balances()
    }

    /// Get account nonce (for transaction replay protection)
    pub fn get_nonce(&self, address: &Address) -> u64 {
        match self.nonces.read() {
            Ok(nonces) => *nonces.get(address).unwrap_or(&0),
            Err(_) => 0,
        }
    }

    /// Set account nonce
    pub fn set_nonce(&self, address: &Address, nonce: u64) -> Result<(), StateError> {
        {
            let mut nonces = self.nonces.write().map_err(|_| {
                StateError::LockError("Failed to acquire nonces write lock".to_string())
            })?;
            nonces.insert(*address, nonce);
        }
        self.save_nonces()
    }

    /// Increment account nonce
    pub fn increment_nonce(&self, address: &Address) -> Result<u64, StateError> {
        let new_nonce = self.get_nonce(address) + 1;
        self.set_nonce(address, new_nonce)?;
        Ok(new_nonce)
    }

    /// Transfer tokens between accounts (atomic operation)
    pub fn transfer(
        &self,
        from: &Address,
        to: &Address,
        amount: Balance,
    ) -> Result<(), StateError> {
        let from_balance = self.get_balance(from);
        if from_balance < amount {
            return Err(StateError::InsufficientBalance {
                address: *from,
                balance: from_balance,
                required: amount,
            });
        }

        let to_balance = self.get_balance(to);

        // Atomic update: both balances at once
        {
            let mut balances = self.balances.write().map_err(|_| {
                StateError::LockError("Failed to acquire balances write lock".to_string())
            })?;
            balances.insert(*from, from_balance - amount);
            balances.insert(*to, to_balance + amount);
        }

        self.save_balances()
    }

    /// Apply a batch of balance changes atomically
    pub fn apply_batch(&self, changes: &[(Address, Balance)]) -> Result<(), StateError> {
        {
            let mut balances = self.balances.write().map_err(|_| {
                StateError::LockError("Failed to acquire balances write lock".to_string())
            })?;
            for (address, balance) in changes {
                balances.insert(*address, *balance);
            }
        }
        self.save_balances()
    }

    /// Flush all pending writes to disk
    pub fn flush(&self) -> Result<(), StateError> {
        self.save_balances()?;
        self.save_nonces()?;
        Ok(())
    }
}

/// State error types for ADZDB account state
#[derive(Debug)]
pub enum StateError {
    InsufficientBalance {
        address: Address,
        balance: Balance,
        required: Balance,
    },
    IoError(String),
    CorruptionError(String),
    LockError(String),
}

impl std::fmt::Display for StateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StateError::InsufficientBalance {
                address,
                balance,
                required,
            } => write!(
                f,
                "Insufficient balance for address {:?}: have {}, need {}",
                address, balance, required
            ),
            StateError::IoError(msg) => write!(f, "I/O error: {}", msg),
            StateError::CorruptionError(msg) => write!(f, "Corruption error: {}", msg),
            StateError::LockError(msg) => write!(f, "Lock error: {}", msg),
        }
    }
}

impl std::error::Error for StateError {}

#[cfg(test)]
mod tests {
    use super::*;
    use coinject_core::Address;

    #[test]
    fn test_adzdb_account_balance() {
        let temp_dir = std::env::temp_dir().join("coinject-adzdb-state-test");
        let _ = std::fs::remove_dir_all(&temp_dir);

        let state = AdzdbAccountState::new(&temp_dir).unwrap();
        let addr = Address::from_bytes([1u8; 32]);

        // Initial balance should be 0
        assert_eq!(state.get_balance(&addr), 0);

        // Set balance
        state.set_balance(&addr, 1000).unwrap();
        assert_eq!(state.get_balance(&addr), 1000);

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_adzdb_transfer() {
        let temp_dir = std::env::temp_dir().join("coinject-adzdb-transfer-test");
        let _ = std::fs::remove_dir_all(&temp_dir);

        let state = AdzdbAccountState::new(&temp_dir).unwrap();
        let sender = Address::from_bytes([1u8; 32]);
        let receiver = Address::from_bytes([2u8; 32]);

        // Setup
        state.set_balance(&sender, 1000).unwrap();
        state.set_balance(&receiver, 0).unwrap();

        // Transfer
        state.transfer(&sender, &receiver, 300).unwrap();

        assert_eq!(state.get_balance(&sender), 700);
        assert_eq!(state.get_balance(&receiver), 300);

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_adzdb_nonce() {
        let temp_dir = std::env::temp_dir().join("coinject-adzdb-nonce-test");
        let _ = std::fs::remove_dir_all(&temp_dir);

        let state = AdzdbAccountState::new(&temp_dir).unwrap();
        let addr = Address::from_bytes([3u8; 32]);

        assert_eq!(state.get_nonce(&addr), 0);
        assert_eq!(state.increment_nonce(&addr).unwrap(), 1);
        assert_eq!(state.get_nonce(&addr), 1);
        assert_eq!(state.increment_nonce(&addr).unwrap(), 2);

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_adzdb_persistence() {
        let temp_dir = std::env::temp_dir().join("coinject-adzdb-persist-test");
        let _ = std::fs::remove_dir_all(&temp_dir);

        let addr = Address::from_bytes([5u8; 32]);

        // Create and populate
        {
            let state = AdzdbAccountState::new(&temp_dir).unwrap();
            state.set_balance(&addr, 5000).unwrap();
            state.set_nonce(&addr, 42).unwrap();
        }

        // Reopen and verify
        {
            let state = AdzdbAccountState::new(&temp_dir).unwrap();
            assert_eq!(state.get_balance(&addr), 5000);
            assert_eq!(state.get_nonce(&addr), 42);
        }

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
