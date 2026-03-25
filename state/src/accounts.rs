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
    use tempfile::tempdir;

    fn temp_state() -> (AccountState, tempfile::TempDir) {
        let dir = tempdir().unwrap();
        let path = dir.path().join("state.db");
        let state = AccountState::new(&path).unwrap();
        (state, dir) // keep dir alive so it isn't dropped/deleted early
    }

    #[test]
    fn test_initial_balance_is_zero() {
        let (state, _dir) = temp_state();
        let addr = Address::from_bytes([1u8; 32]);
        assert_eq!(state.get_balance(&addr), 0);
    }

    #[test]
    fn test_set_and_get_balance() {
        let (state, _dir) = temp_state();
        let addr = Address::from_bytes([1u8; 32]);

        state.set_balance(&addr, 1000).unwrap();
        assert_eq!(state.get_balance(&addr), 1000);

        state.set_balance(&addr, 5000).unwrap();
        assert_eq!(state.get_balance(&addr), 5000);
    }

    #[test]
    fn test_transfer_moves_funds() {
        let (state, _dir) = temp_state();
        let sender = Address::from_bytes([1u8; 32]);
        let receiver = Address::from_bytes([2u8; 32]);

        state.set_balance(&sender, 1000).unwrap();
        state.set_balance(&receiver, 0).unwrap();
        state.transfer(&sender, &receiver, 300).unwrap();

        assert_eq!(state.get_balance(&sender), 700);
        assert_eq!(state.get_balance(&receiver), 300);
    }

    #[test]
    fn test_transfer_insufficient_balance_errors() {
        let (state, _dir) = temp_state();
        let sender = Address::from_bytes([1u8; 32]);
        let receiver = Address::from_bytes([2u8; 32]);

        state.set_balance(&sender, 100).unwrap();

        let result = state.transfer(&sender, &receiver, 500);
        assert!(
            matches!(result, Err(StateError::InsufficientBalance { .. })),
            "Expected InsufficientBalance error"
        );
        // Funds must not have moved
        assert_eq!(state.get_balance(&sender), 100);
    }

    #[test]
    fn test_transfer_full_balance() {
        let (state, _dir) = temp_state();
        let sender = Address::from_bytes([10u8; 32]);
        let receiver = Address::from_bytes([11u8; 32]);

        state.set_balance(&sender, 500).unwrap();
        state.transfer(&sender, &receiver, 500).unwrap();

        assert_eq!(state.get_balance(&sender), 0);
        assert_eq!(state.get_balance(&receiver), 500);
    }

    #[test]
    fn test_nonce_starts_at_zero() {
        let (state, _dir) = temp_state();
        let addr = Address::from_bytes([3u8; 32]);
        assert_eq!(state.get_nonce(&addr), 0);
    }

    #[test]
    fn test_nonce_increments_sequentially() {
        let (state, _dir) = temp_state();
        let addr = Address::from_bytes([3u8; 32]);

        assert_eq!(state.increment_nonce(&addr).unwrap(), 1);
        assert_eq!(state.get_nonce(&addr), 1);
        assert_eq!(state.increment_nonce(&addr).unwrap(), 2);
        assert_eq!(state.get_nonce(&addr), 2);
    }

    #[test]
    fn test_apply_batch_sets_multiple_balances() {
        let (state, _dir) = temp_state();
        let a = Address::from_bytes([1u8; 32]);
        let b = Address::from_bytes([2u8; 32]);
        let c = Address::from_bytes([3u8; 32]);

        let changes = vec![(a, 100), (b, 200), (c, 300)];
        state.apply_batch(&changes).unwrap();

        assert_eq!(state.get_balance(&a), 100);
        assert_eq!(state.get_balance(&b), 200);
        assert_eq!(state.get_balance(&c), 300);
    }

    #[test]
    fn test_different_addresses_are_isolated() {
        let (state, _dir) = temp_state();
        let addr_a = Address::from_bytes([0xAA; 32]);
        let addr_b = Address::from_bytes([0xBB; 32]);

        state.set_balance(&addr_a, 999).unwrap();
        assert_eq!(state.get_balance(&addr_a), 999);
        assert_eq!(state.get_balance(&addr_b), 0); // Unaffected
    }
}
