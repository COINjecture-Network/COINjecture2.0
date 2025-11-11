// Time-locked balance tracking
// Funds locked until specific Unix timestamp

use coinject_core::{Address, Balance, Hash};
use serde::{Deserialize, Serialize};
use redb::{Database, ReadableTable, TableDefinition};
use std::sync::Arc;

// Table definition for timelocks
const TIMELOCKS_TABLE: TableDefinition<&[u8], &[u8]> = TableDefinition::new("timelocks");

/// Time-locked balance entry
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TimeLock {
    /// Transaction hash that created this lock
    pub tx_hash: Hash,
    /// Address that locked the funds
    pub from: Address,
    /// Address that can claim after unlock
    pub recipient: Address,
    /// Locked amount
    pub amount: Balance,
    /// Unix timestamp when funds unlock
    pub unlock_time: i64,
    /// Block height when created
    pub created_at_height: u64,
}

/// Time-lock state management
pub struct TimeLockState {
    db: Arc<Database>,
}

impl TimeLockState {
    /// Create new time-lock state manager
    pub fn new(db: Arc<Database>) -> Result<Self, redb::Error> {
        // Initialize table
        let write_txn = db.begin_write()?;
        {
            let _ = write_txn.open_table(TIMELOCKS_TABLE)?;
        }
        write_txn.commit()?;

        Ok(TimeLockState { db })
    }

    /// Add a new time-lock
    pub fn add_timelock(&self, timelock: TimeLock) -> Result<(), String> {
        let key = Self::make_key(&timelock.tx_hash);
        let value = bincode::serialize(&timelock)
            .map_err(|e| format!("Failed to serialize timelock: {}", e))?;

        let write_txn = self.db.begin_write()
            .map_err(|e| format!("Failed to begin write: {}", e))?;
        {
            let mut table = write_txn.open_table(TIMELOCKS_TABLE)
                .map_err(|e| format!("Failed to open table: {}", e))?;
            table.insert(&key[..], value.as_slice())
                .map_err(|e| format!("Failed to insert timelock: {}", e))?;
        }
        write_txn.commit()
            .map_err(|e| format!("Failed to commit: {}", e))?;

        Ok(())
    }

    /// Get time-lock by transaction hash
    pub fn get_timelock(&self, tx_hash: &Hash) -> Option<TimeLock> {
        let key = Self::make_key(tx_hash);
        let read_txn = self.db.begin_read().ok()?;
        let table = read_txn.open_table(TIMELOCKS_TABLE).ok()?;

        let bytes = table.get(&key[..]).ok()??;
        bincode::deserialize(bytes.value()).ok()
    }

    /// Remove time-lock (after unlocking)
    pub fn remove_timelock(&self, tx_hash: &Hash) -> Result<(), String> {
        let key = Self::make_key(tx_hash);

        let write_txn = self.db.begin_write()
            .map_err(|e| format!("Failed to begin write: {}", e))?;
        {
            let mut table = write_txn.open_table(TIMELOCKS_TABLE)
                .map_err(|e| format!("Failed to open table: {}", e))?;
            table.remove(&key[..])
                .map_err(|e| format!("Failed to remove timelock: {}", e))?;
        }
        write_txn.commit()
            .map_err(|e| format!("Failed to commit: {}", e))?;

        Ok(())
    }

    /// Get all time-locks for a recipient
    pub fn get_timelocks_for_recipient(&self, recipient: &Address) -> Vec<TimeLock> {
        let mut timelocks = Vec::new();

        if let Ok(read_txn) = self.db.begin_read() {
            if let Ok(table) = read_txn.open_table(TIMELOCKS_TABLE) {
                for item in table.iter().ok().into_iter().flatten() {
                    if let Ok((_, value)) = item {
                        if let Ok(timelock) = bincode::deserialize::<TimeLock>(value.value()) {
                            if timelock.recipient == *recipient {
                                timelocks.push(timelock);
                            }
                        }
                    }
                }
            }
        }

        timelocks
    }

    /// Get all unlocked time-locks (current time >= unlock_time)
    pub fn get_unlocked_timelocks(&self) -> Vec<TimeLock> {
        let now = chrono::Utc::now().timestamp();
        let mut unlocked = Vec::new();

        if let Ok(read_txn) = self.db.begin_read() {
            if let Ok(table) = read_txn.open_table(TIMELOCKS_TABLE) {
                for item in table.iter().ok().into_iter().flatten() {
                    if let Ok((_, value)) = item {
                        if let Ok(timelock) = bincode::deserialize::<TimeLock>(value.value()) {
                            if timelock.unlock_time <= now {
                                unlocked.push(timelock);
                            }
                        }
                    }
                }
            }
        }

        unlocked
    }

    /// Check if a specific time-lock is unlocked
    pub fn is_unlocked(&self, tx_hash: &Hash) -> bool {
        if let Some(timelock) = self.get_timelock(tx_hash) {
            let now = chrono::Utc::now().timestamp();
            timelock.unlock_time <= now
        } else {
            false
        }
    }

    /// Get total locked balance for an address
    pub fn get_locked_balance(&self, address: &Address) -> Balance {
        let now = chrono::Utc::now().timestamp();
        let mut total = 0u128;

        if let Ok(read_txn) = self.db.begin_read() {
            if let Ok(table) = read_txn.open_table(TIMELOCKS_TABLE) {
                for item in table.iter().ok().into_iter().flatten() {
                    if let Ok((_, value)) = item {
                        if let Ok(timelock) = bincode::deserialize::<TimeLock>(value.value()) {
                            // Count locks where address is the locker and not yet unlocked
                            if timelock.from == *address && timelock.unlock_time > now {
                                total += timelock.amount;
                            }
                        }
                    }
                }
            }
        }

        total
    }

    /// Database key prefix for time-locks
    fn make_key(tx_hash: &Hash) -> Vec<u8> {
        let mut key = vec![0x10]; // Prefix 0x10 for time-locks
        key.extend_from_slice(tx_hash.as_bytes());
        key
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_timelock_add_and_get() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.redb");
        let db = Arc::new(Database::create(&db_path).unwrap());
        let state = TimeLockState::new(db).unwrap();

        let timelock = TimeLock {
            tx_hash: Hash::from_bytes([1u8; 32]),
            from: Address::from_bytes([2u8; 32]),
            recipient: Address::from_bytes([3u8; 32]),
            amount: 1000,
            unlock_time: chrono::Utc::now().timestamp() + 3600,
            created_at_height: 100,
        };

        state.add_timelock(timelock.clone()).unwrap();

        let retrieved = state.get_timelock(&timelock.tx_hash);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().amount, 1000);
    }

    #[test]
    fn test_unlocked_timelocks() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.redb");
        let db = Arc::new(Database::create(&db_path).unwrap());
        let state = TimeLockState::new(db).unwrap();

        // Create unlocked timelock (in the past)
        let unlocked_timelock = TimeLock {
            tx_hash: Hash::from_bytes([1u8; 32]),
            from: Address::from_bytes([2u8; 32]),
            recipient: Address::from_bytes([3u8; 32]),
            amount: 1000,
            unlock_time: chrono::Utc::now().timestamp() - 100,
            created_at_height: 100,
        };

        state.add_timelock(unlocked_timelock).unwrap();

        let unlocked = state.get_unlocked_timelocks();
        assert_eq!(unlocked.len(), 1);
    }
}
