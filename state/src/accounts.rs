// Account-based state management with redb database (pure Rust, ACID-compliant)
use coinject_core::{Address, Balance};
use redb::{Database, TableDefinition};
use std::path::Path;
use std::sync::Arc;

// Table definitions for redb (using fixed-size keys for addresses)
const BALANCES_TABLE: TableDefinition<&[u8; 32], u128> = TableDefinition::new("balances");
const NONCES_TABLE: TableDefinition<&[u8; 32], u64> = TableDefinition::new("nonces");

pub struct AccountState {
    db: Arc<Database>,
}

impl AccountState {
    /// Open or create the state database
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, redb::Error> {
        let db = Database::create(path)?;

        // Initialize tables
        let write_txn = db.begin_write()?;
        {
            let _ = write_txn.open_table(BALANCES_TABLE)?;
            let _ = write_txn.open_table(NONCES_TABLE)?;
        }
        write_txn.commit()?;

        Ok(AccountState { db: Arc::new(db) })
    }

    /// Create AccountState from an existing database
    pub fn from_db(db: Arc<Database>) -> Self {
        AccountState { db }
    }

    /// Get account balance
    pub fn get_balance(&self, address: &Address) -> Balance {
        let read_txn = match self.db.begin_read() {
            Ok(txn) => txn,
            Err(_) => return 0,
        };

        let table = match read_txn.open_table(BALANCES_TABLE) {
            Ok(t) => t,
            Err(_) => return 0,
        };

        table
            .get(address.as_bytes())
            .ok()
            .flatten()
            .map(|v| v.value())
            .unwrap_or(0)
    }

    /// Set account balance
    pub fn set_balance(&self, address: &Address, balance: Balance) -> Result<(), StateError> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(BALANCES_TABLE)?;
            table.insert(address.as_bytes(), balance)?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Get account nonce (for transaction replay protection)
    pub fn get_nonce(&self, address: &Address) -> u64 {
        let read_txn = match self.db.begin_read() {
            Ok(txn) => txn,
            Err(_) => return 0,
        };

        let table = match read_txn.open_table(NONCES_TABLE) {
            Ok(t) => t,
            Err(_) => return 0,
        };

        table
            .get(address.as_bytes())
            .ok()
            .flatten()
            .map(|v| v.value())
            .unwrap_or(0)
    }

    /// Set account nonce
    pub fn set_nonce(&self, address: &Address, nonce: u64) -> Result<(), StateError> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(NONCES_TABLE)?;
            table.insert(address.as_bytes(), nonce)?;
        }
        write_txn.commit()?;
        Ok(())
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

        // ACID transaction: both updates or neither
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(BALANCES_TABLE)?;
            table.insert(from.as_bytes(), from_balance - amount)?;
            table.insert(to.as_bytes(), to_balance + amount)?;
        }
        write_txn.commit()?;

        Ok(())
    }

    /// Apply a batch of balance changes atomically
    pub fn apply_batch(&self, changes: &[(Address, Balance)]) -> Result<(), StateError> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(BALANCES_TABLE)?;
            for (address, balance) in changes {
                table.insert(address.as_bytes(), *balance)?;
            }
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Flush all pending writes to disk (redb auto-flushes on commit)
    pub fn flush(&self) -> Result<(), StateError> {
        // redb automatically flushes on transaction commit
        // This method exists for API compatibility
        Ok(())
    }
}

#[derive(Debug)]
pub enum StateError {
    InsufficientBalance {
        address: Address,
        balance: Balance,
        required: Balance,
    },
    DatabaseError(redb::Error),
    StorageError(redb::StorageError),
    TableError(redb::TableError),
    CommitError(redb::CommitError),
    TransactionError(redb::TransactionError),
}

impl From<redb::Error> for StateError {
    fn from(err: redb::Error) -> Self {
        StateError::DatabaseError(err)
    }
}

impl From<redb::StorageError> for StateError {
    fn from(err: redb::StorageError) -> Self {
        StateError::StorageError(err)
    }
}

impl From<redb::TableError> for StateError {
    fn from(err: redb::TableError) -> Self {
        StateError::TableError(err)
    }
}

impl From<redb::CommitError> for StateError {
    fn from(err: redb::CommitError) -> Self {
        StateError::CommitError(err)
    }
}

impl From<redb::TransactionError> for StateError {
    fn from(err: redb::TransactionError) -> Self {
        StateError::TransactionError(err)
    }
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
            StateError::DatabaseError(e) => write!(f, "Database error: {}", e),
            StateError::StorageError(e) => write!(f, "Storage error: {}", e),
            StateError::TableError(e) => write!(f, "Table error: {}", e),
            StateError::CommitError(e) => write!(f, "Commit error: {}", e),
            StateError::TransactionError(e) => write!(f, "Transaction error: {}", e),
        }
    }
}

impl std::error::Error for StateError {}

#[cfg(test)]
mod tests {
    use super::*;
    use coinject_core::Address;

    #[test]
    fn test_account_balance() {
        let state = AccountState::new("test_db").unwrap();
        let addr = Address::from_bytes([1u8; 32]);

        // Initial balance should be 0
        assert_eq!(state.get_balance(&addr), 0);

        // Set balance
        state.set_balance(&addr, 1000).unwrap();
        assert_eq!(state.get_balance(&addr), 1000);

        // Cleanup
        std::fs::remove_file("test_db").ok();
    }

    #[test]
    fn test_transfer() {
        let state = AccountState::new("test_transfer_db").unwrap();
        let alice = Address::from_bytes([1u8; 32]);
        let bob = Address::from_bytes([2u8; 32]);

        // Setup
        state.set_balance(&alice, 1000).unwrap();
        state.set_balance(&bob, 0).unwrap();

        // Transfer
        state.transfer(&alice, &bob, 300).unwrap();

        assert_eq!(state.get_balance(&alice), 700);
        assert_eq!(state.get_balance(&bob), 300);

        // Cleanup
        std::fs::remove_file("test_transfer_db").ok();
    }

    #[test]
    fn test_nonce() {
        let state = AccountState::new("test_nonce_db").unwrap();
        let addr = Address::from_bytes([3u8; 32]);

        assert_eq!(state.get_nonce(&addr), 0);
        assert_eq!(state.increment_nonce(&addr).unwrap(), 1);
        assert_eq!(state.get_nonce(&addr), 1);
        assert_eq!(state.increment_nonce(&addr).unwrap(), 2);

        // Cleanup
        std::fs::remove_file("test_nonce_db").ok();
    }
}
