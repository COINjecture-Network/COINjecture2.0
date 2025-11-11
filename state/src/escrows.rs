// Conditional escrow tracking
// Multi-party escrows with arbiter support

use coinject_core::{Address, Balance, Hash};
use redb::{Database, ReadableTable, TableDefinition};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// Table definition for redb
const ESCROWS_TABLE: TableDefinition<&[u8], &[u8]> = TableDefinition::new("escrows");

/// Escrow status
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum EscrowStatus {
    /// Escrow is active and funds are locked
    Active,
    /// Escrow completed, funds released to recipient
    Released,
    /// Escrow refunded to sender
    Refunded,
    /// Escrow expired (timeout reached)
    Expired,
}

/// Escrow entry
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Escrow {
    /// Unique escrow identifier (from transaction hash)
    pub escrow_id: Hash,
    /// Address that created and funded the escrow
    pub sender: Address,
    /// Address that receives funds on release
    pub recipient: Address,
    /// Optional arbiter who can mediate disputes
    pub arbiter: Option<Address>,
    /// Escrowed amount
    pub amount: Balance,
    /// Unix timestamp when escrow expires
    pub timeout: i64,
    /// Hash of escrow conditions (for reference)
    pub conditions_hash: Hash,
    /// Current status
    pub status: EscrowStatus,
    /// Block height when created
    pub created_at_height: u64,
    /// Block height when resolved (if resolved)
    pub resolved_at_height: Option<u64>,
}

/// Escrow state management
pub struct EscrowState {
    db: Arc<Database>,
}

impl EscrowState {
    /// Create new escrow state manager
    pub fn new(db: Arc<Database>) -> Result<Self, redb::Error> {
        // Initialize tables
        let write_txn = db.begin_write()?;
        {
            let _ = write_txn.open_table(ESCROWS_TABLE)?;
        }
        write_txn.commit()?;

        Ok(EscrowState { db })
    }

    /// Create a new escrow
    pub fn create_escrow(&self, escrow: Escrow) -> Result<(), String> {
        // Check if escrow already exists
        if self.get_escrow(&escrow.escrow_id).is_some() {
            return Err("Escrow already exists".to_string());
        }

        let key = Self::make_key(&escrow.escrow_id);
        let value = bincode::serialize(&escrow)
            .map_err(|e| format!("Failed to serialize escrow: {}", e))?;

        let write_txn = self
            .db
            .begin_write()
            .map_err(|e| format!("Failed to begin write transaction: {}", e))?;
        {
            let mut table = write_txn
                .open_table(ESCROWS_TABLE)
                .map_err(|e| format!("Failed to open table: {}", e))?;
            table
                .insert(key.as_slice(), value.as_slice())
                .map_err(|e| format!("Failed to insert escrow: {}", e))?;
        }
        write_txn
            .commit()
            .map_err(|e| format!("Failed to commit transaction: {}", e))?;

        Ok(())
    }

    /// Get escrow by ID
    pub fn get_escrow(&self, escrow_id: &Hash) -> Option<Escrow> {
        let read_txn = self.db.begin_read().ok()?;
        let table = read_txn.open_table(ESCROWS_TABLE).ok()?;
        let key = Self::make_key(escrow_id);
        let bytes = table.get(key.as_slice()).ok()??;
        bincode::deserialize(bytes.value()).ok()
    }

    /// Update escrow status
    pub fn update_escrow_status(
        &self,
        escrow_id: &Hash,
        status: EscrowStatus,
        resolved_height: Option<u64>,
    ) -> Result<(), String> {
        let mut escrow = self
            .get_escrow(escrow_id)
            .ok_or("Escrow not found".to_string())?;

        escrow.status = status;
        escrow.resolved_at_height = resolved_height;

        let key = Self::make_key(escrow_id);
        let value = bincode::serialize(&escrow)
            .map_err(|e| format!("Failed to serialize escrow: {}", e))?;

        let write_txn = self
            .db
            .begin_write()
            .map_err(|e| format!("Failed to begin write transaction: {}", e))?;
        {
            let mut table = write_txn
                .open_table(ESCROWS_TABLE)
                .map_err(|e| format!("Failed to open table: {}", e))?;
            table
                .insert(key.as_slice(), value.as_slice())
                .map_err(|e| format!("Failed to update escrow: {}", e))?;
        }
        write_txn
            .commit()
            .map_err(|e| format!("Failed to commit transaction: {}", e))?;

        Ok(())
    }

    /// Get all active escrows
    pub fn get_active_escrows(&self) -> Vec<Escrow> {
        let mut escrows = Vec::new();

        if let Ok(read_txn) = self.db.begin_read() {
            if let Ok(table) = read_txn.open_table(ESCROWS_TABLE) {
                for item in table.iter().ok().into_iter().flatten() {
                    if let Ok((_, value)) = item {
                        if let Ok(escrow) = bincode::deserialize::<Escrow>(value.value()) {
                            if escrow.status == EscrowStatus::Active {
                                escrows.push(escrow);
                            }
                        }
                    }
                }
            }
        }

        escrows
    }

    /// Get all escrows for a sender
    pub fn get_escrows_by_sender(&self, sender: &Address) -> Vec<Escrow> {
        let mut escrows = Vec::new();

        if let Ok(read_txn) = self.db.begin_read() {
            if let Ok(table) = read_txn.open_table(ESCROWS_TABLE) {
                for item in table.iter().ok().into_iter().flatten() {
                    if let Ok((_, value)) = item {
                        if let Ok(escrow) = bincode::deserialize::<Escrow>(value.value()) {
                            if escrow.sender == *sender {
                                escrows.push(escrow);
                            }
                        }
                    }
                }
            }
        }

        escrows
    }

    /// Get all escrows for a recipient
    pub fn get_escrows_by_recipient(&self, recipient: &Address) -> Vec<Escrow> {
        let mut escrows = Vec::new();

        if let Ok(read_txn) = self.db.begin_read() {
            if let Ok(table) = read_txn.open_table(ESCROWS_TABLE) {
                for item in table.iter().ok().into_iter().flatten() {
                    if let Ok((_, value)) = item {
                        if let Ok(escrow) = bincode::deserialize::<Escrow>(value.value()) {
                            if escrow.recipient == *recipient {
                                escrows.push(escrow);
                            }
                        }
                    }
                }
            }
        }

        escrows
    }

    /// Get expired escrows (timeout passed but still active)
    pub fn get_expired_escrows(&self) -> Vec<Escrow> {
        let now = chrono::Utc::now().timestamp();
        let mut expired = Vec::new();

        if let Ok(read_txn) = self.db.begin_read() {
            if let Ok(table) = read_txn.open_table(ESCROWS_TABLE) {
                for item in table.iter().ok().into_iter().flatten() {
                    if let Ok((_, value)) = item {
                        if let Ok(escrow) = bincode::deserialize::<Escrow>(value.value()) {
                            if escrow.status == EscrowStatus::Active && escrow.timeout <= now {
                                expired.push(escrow);
                            }
                        }
                    }
                }
            }
        }

        expired
    }

    /// Check if an escrow can be released
    /// Requires arbiter or recipient signature
    pub fn can_release(&self, escrow_id: &Hash, releaser: &Address) -> bool {
        if let Some(escrow) = self.get_escrow(escrow_id) {
            if escrow.status != EscrowStatus::Active {
                return false;
            }

            // Arbiter can always release
            if let Some(arbiter) = &escrow.arbiter {
                if releaser == arbiter {
                    return true;
                }
            }

            // Recipient can release
            releaser == &escrow.recipient
        } else {
            false
        }
    }

    /// Check if an escrow can be refunded
    /// Requires arbiter signature or timeout
    pub fn can_refund(&self, escrow_id: &Hash, refunder: &Address) -> bool {
        if let Some(escrow) = self.get_escrow(escrow_id) {
            if escrow.status != EscrowStatus::Active {
                return false;
            }

            let now = chrono::Utc::now().timestamp();

            // Arbiter can always refund
            if let Some(arbiter) = &escrow.arbiter {
                if refunder == arbiter {
                    return true;
                }
            }

            // Sender can refund after timeout
            refunder == &escrow.sender && escrow.timeout <= now
        } else {
            false
        }
    }

    /// Get total escrowed balance for an address (as sender)
    pub fn get_escrowed_balance(&self, address: &Address) -> Balance {
        let mut total = 0u128;

        if let Ok(read_txn) = self.db.begin_read() {
            if let Ok(table) = read_txn.open_table(ESCROWS_TABLE) {
                for item in table.iter().ok().into_iter().flatten() {
                    if let Ok((_, value)) = item {
                        if let Ok(escrow) = bincode::deserialize::<Escrow>(value.value()) {
                            if escrow.sender == *address && escrow.status == EscrowStatus::Active {
                                total += escrow.amount;
                            }
                        }
                    }
                }
            }
        }

        total
    }

    /// Database key prefix for escrows
    fn make_key(escrow_id: &Hash) -> Vec<u8> {
        let mut key = vec![0x20]; // Prefix 0x20 for escrows
        key.extend_from_slice(escrow_id.as_bytes());
        key
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_create_and_get_escrow() {
        let dir = tempdir().unwrap();
        let db = Arc::new(Database::create(dir.path().join("escrow_test")).unwrap());
        let state = EscrowState::new(db).unwrap();

        let escrow = Escrow {
            escrow_id: Hash::from_bytes([1u8; 32]),
            sender: Address::from_bytes([2u8; 32]),
            recipient: Address::from_bytes([3u8; 32]),
            arbiter: Some(Address::from_bytes([4u8; 32])),
            amount: 5000,
            timeout: chrono::Utc::now().timestamp() + 86400,
            conditions_hash: Hash::from_bytes([5u8; 32]),
            status: EscrowStatus::Active,
            created_at_height: 100,
            resolved_at_height: None,
        };

        state.create_escrow(escrow.clone()).unwrap();

        let retrieved = state.get_escrow(&escrow.escrow_id);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().amount, 5000);
    }

    #[test]
    fn test_can_release() {
        let dir = tempdir().unwrap();
        let db = Arc::new(Database::create(dir.path().join("escrow_release_test")).unwrap());
        let state = EscrowState::new(db).unwrap();

        let recipient = Address::from_bytes([3u8; 32]);
        let arbiter = Address::from_bytes([4u8; 32]);

        let escrow = Escrow {
            escrow_id: Hash::from_bytes([1u8; 32]),
            sender: Address::from_bytes([2u8; 32]),
            recipient,
            arbiter: Some(arbiter),
            amount: 5000,
            timeout: chrono::Utc::now().timestamp() + 86400,
            conditions_hash: Hash::from_bytes([5u8; 32]),
            status: EscrowStatus::Active,
            created_at_height: 100,
            resolved_at_height: None,
        };

        state.create_escrow(escrow.clone()).unwrap();

        // Recipient can release
        assert!(state.can_release(&escrow.escrow_id, &recipient));

        // Arbiter can release
        assert!(state.can_release(&escrow.escrow_id, &arbiter));

        // Sender cannot release
        assert!(!state.can_release(&escrow.escrow_id, &escrow.sender));
    }
}
