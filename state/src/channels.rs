// Payment channel state tracking
// Lightning-style bidirectional payment channels

use coinject_core::{Address, Balance, Hash};
use redb::{Database, ReadableTable, TableDefinition};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// Table definition for redb
const CHANNELS_TABLE: TableDefinition<&[u8], &[u8]> = TableDefinition::new("channels");

/// Payment channel status
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ChannelStatus {
    /// Channel is open and active
    Open,
    /// Channel closed cooperatively
    ClosedCooperative,
    /// Channel closed via dispute
    ClosedDispute,
    /// Channel in dispute period
    InDispute,
}

/// Payment channel entry
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Channel {
    /// Unique channel identifier (from opening transaction)
    pub channel_id: Hash,
    /// First participant
    pub participant_a: Address,
    /// Second participant
    pub participant_b: Address,
    /// Deposit from participant A
    pub deposit_a: Balance,
    /// Deposit from participant B
    pub deposit_b: Balance,
    /// Current balance of participant A
    pub balance_a: Balance,
    /// Current balance of participant B
    pub balance_b: Balance,
    /// Latest sequence number (for dispute resolution)
    pub sequence: u64,
    /// Dispute timeout in seconds
    pub dispute_timeout: i64,
    /// Current status
    pub status: ChannelStatus,
    /// Block height when opened
    pub opened_at_height: u64,
    /// Block height when closed (if closed)
    pub closed_at_height: Option<u64>,
    /// Timestamp when dispute started (if in dispute)
    pub dispute_started_at: Option<i64>,
}

impl Channel {
    /// Get total channel capacity
    pub fn capacity(&self) -> Balance {
        self.deposit_a + self.deposit_b
    }

    /// Validate that balances equal total deposits
    pub fn is_balanced(&self) -> bool {
        self.balance_a + self.balance_b == self.capacity()
    }

    /// Check if address is a participant
    pub fn is_participant(&self, address: &Address) -> bool {
        address == &self.participant_a || address == &self.participant_b
    }

    /// Check if channel can be closed unilaterally
    pub fn can_close_unilaterally(&self) -> bool {
        self.status == ChannelStatus::Open
    }

    /// Check if dispute period has expired
    pub fn is_dispute_expired(&self) -> bool {
        if let Some(dispute_start) = self.dispute_started_at {
            let now = chrono::Utc::now().timestamp();
            now >= dispute_start + self.dispute_timeout
        } else {
            false
        }
    }
}

/// Payment channel state management
pub struct ChannelState {
    db: Arc<Database>,
}

impl ChannelState {
    /// Create new channel state manager
    pub fn new(db: Arc<Database>) -> Result<Self, redb::Error> {
        // Initialize tables
        let write_txn = db.begin_write()?;
        {
            let _ = write_txn.open_table(CHANNELS_TABLE)?;
        }
        write_txn.commit()?;

        Ok(ChannelState { db })
    }

    /// Open a new payment channel
    pub fn open_channel(&self, channel: Channel) -> Result<(), String> {
        // Check if channel already exists
        if self.get_channel(&channel.channel_id).is_some() {
            return Err("Channel already exists".to_string());
        }

        // Validate initial balances equal deposits
        if channel.balance_a != channel.deposit_a || channel.balance_b != channel.deposit_b {
            return Err("Initial balances must equal deposits".to_string());
        }

        let key = Self::make_key(&channel.channel_id);
        let value = bincode::serialize(&channel)
            .map_err(|e| format!("Failed to serialize channel: {}", e))?;

        let write_txn = self
            .db
            .begin_write()
            .map_err(|e| format!("Failed to begin write transaction: {}", e))?;
        {
            let mut table = write_txn
                .open_table(CHANNELS_TABLE)
                .map_err(|e| format!("Failed to open table: {}", e))?;
            table
                .insert(key.as_slice(), value.as_slice())
                .map_err(|e| format!("Failed to insert channel: {}", e))?;
        }
        write_txn
            .commit()
            .map_err(|e| format!("Failed to commit transaction: {}", e))?;

        Ok(())
    }

    /// Get channel by ID
    pub fn get_channel(&self, channel_id: &Hash) -> Option<Channel> {
        let read_txn = self.db.begin_read().ok()?;
        let table = read_txn.open_table(CHANNELS_TABLE).ok()?;
        let key = Self::make_key(channel_id);
        let bytes = table.get(key.as_slice()).ok()??;
        bincode::deserialize(bytes.value()).ok()
    }

    /// Update channel state (for off-chain updates)
    pub fn update_channel_state(
        &self,
        channel_id: &Hash,
        sequence: u64,
        balance_a: Balance,
        balance_b: Balance,
    ) -> Result<(), String> {
        let mut channel = self
            .get_channel(channel_id)
            .ok_or("Channel not found".to_string())?;

        // Can only update open channels
        if channel.status != ChannelStatus::Open {
            return Err("Channel is not open".to_string());
        }

        // Sequence must increase
        if sequence <= channel.sequence {
            return Err("Invalid sequence number".to_string());
        }

        // Balances must be valid
        if balance_a + balance_b != channel.capacity() {
            return Err("Balances do not equal capacity".to_string());
        }

        channel.sequence = sequence;
        channel.balance_a = balance_a;
        channel.balance_b = balance_b;

        self.save_channel(&channel)?;

        Ok(())
    }

    /// Close channel cooperatively
    pub fn close_cooperative(
        &self,
        channel_id: &Hash,
        final_balance_a: Balance,
        final_balance_b: Balance,
        block_height: u64,
    ) -> Result<(), String> {
        let mut channel = self
            .get_channel(channel_id)
            .ok_or("Channel not found".to_string())?;

        if channel.status != ChannelStatus::Open {
            return Err("Channel is not open".to_string());
        }

        // Validate final balances
        if final_balance_a + final_balance_b != channel.capacity() {
            return Err("Final balances do not equal capacity".to_string());
        }

        channel.status = ChannelStatus::ClosedCooperative;
        channel.balance_a = final_balance_a;
        channel.balance_b = final_balance_b;
        channel.closed_at_height = Some(block_height);

        self.save_channel(&channel)?;

        Ok(())
    }

    /// Start dispute for unilateral close
    pub fn start_dispute(
        &self,
        channel_id: &Hash,
        sequence: u64,
        balance_a: Balance,
        balance_b: Balance,
    ) -> Result<(), String> {
        let mut channel = self
            .get_channel(channel_id)
            .ok_or("Channel not found".to_string())?;

        if channel.status != ChannelStatus::Open {
            return Err("Channel is not open".to_string());
        }

        // Validate balances
        if balance_a + balance_b != channel.capacity() {
            return Err("Balances do not equal capacity".to_string());
        }

        channel.status = ChannelStatus::InDispute;
        channel.sequence = sequence;
        channel.balance_a = balance_a;
        channel.balance_b = balance_b;
        channel.dispute_started_at = Some(chrono::Utc::now().timestamp());

        self.save_channel(&channel)?;

        Ok(())
    }

    /// Finalize disputed channel after timeout
    pub fn finalize_dispute(
        &self,
        channel_id: &Hash,
        block_height: u64,
    ) -> Result<(), String> {
        let mut channel = self
            .get_channel(channel_id)
            .ok_or("Channel not found".to_string())?;

        if channel.status != ChannelStatus::InDispute {
            return Err("Channel is not in dispute".to_string());
        }

        if !channel.is_dispute_expired() {
            return Err("Dispute period not expired".to_string());
        }

        channel.status = ChannelStatus::ClosedDispute;
        channel.closed_at_height = Some(block_height);

        self.save_channel(&channel)?;

        Ok(())
    }

    /// Get all open channels
    pub fn get_open_channels(&self) -> Vec<Channel> {
        let mut channels = Vec::new();

        if let Ok(read_txn) = self.db.begin_read() {
            if let Ok(table) = read_txn.open_table(CHANNELS_TABLE) {
                for item in table.iter().ok().into_iter().flatten() {
                    if let Ok((_, value)) = item {
                        if let Ok(channel) = bincode::deserialize::<Channel>(value.value()) {
                            if channel.status == ChannelStatus::Open {
                                channels.push(channel);
                            }
                        }
                    }
                }
            }
        }

        channels
    }

    /// Get all channels for an address (as either participant)
    pub fn get_channels_for_address(&self, address: &Address) -> Vec<Channel> {
        let mut channels = Vec::new();

        if let Ok(read_txn) = self.db.begin_read() {
            if let Ok(table) = read_txn.open_table(CHANNELS_TABLE) {
                for item in table.iter().ok().into_iter().flatten() {
                    if let Ok((_, value)) = item {
                        if let Ok(channel) = bincode::deserialize::<Channel>(value.value()) {
                            if channel.is_participant(address) {
                                channels.push(channel);
                            }
                        }
                    }
                }
            }
        }

        channels
    }

    /// Get channels in dispute
    pub fn get_disputed_channels(&self) -> Vec<Channel> {
        let mut channels = Vec::new();

        if let Ok(read_txn) = self.db.begin_read() {
            if let Ok(table) = read_txn.open_table(CHANNELS_TABLE) {
                for item in table.iter().ok().into_iter().flatten() {
                    if let Ok((_, value)) = item {
                        if let Ok(channel) = bincode::deserialize::<Channel>(value.value()) {
                            if channel.status == ChannelStatus::InDispute {
                                channels.push(channel);
                            }
                        }
                    }
                }
            }
        }

        channels
    }

    /// Get total locked balance in channels for an address
    pub fn get_locked_in_channels(&self, address: &Address) -> Balance {
        let mut total = 0u128;

        if let Ok(read_txn) = self.db.begin_read() {
            if let Ok(table) = read_txn.open_table(CHANNELS_TABLE) {
                for item in table.iter().ok().into_iter().flatten() {
                    if let Ok((_, value)) = item {
                        if let Ok(channel) = bincode::deserialize::<Channel>(value.value()) {
                            if channel.status == ChannelStatus::Open {
                                if channel.participant_a == *address {
                                    total += channel.balance_a;
                                } else if channel.participant_b == *address {
                                    total += channel.balance_b;
                                }
                            }
                        }
                    }
                }
            }
        }

        total
    }

    /// Save channel to database
    fn save_channel(&self, channel: &Channel) -> Result<(), String> {
        let key = Self::make_key(&channel.channel_id);
        let value = bincode::serialize(channel)
            .map_err(|e| format!("Failed to serialize channel: {}", e))?;

        let write_txn = self
            .db
            .begin_write()
            .map_err(|e| format!("Failed to begin write transaction: {}", e))?;
        {
            let mut table = write_txn
                .open_table(CHANNELS_TABLE)
                .map_err(|e| format!("Failed to open table: {}", e))?;
            table
                .insert(key.as_slice(), value.as_slice())
                .map_err(|e| format!("Failed to save channel: {}", e))?;
        }
        write_txn
            .commit()
            .map_err(|e| format!("Failed to commit transaction: {}", e))?;

        Ok(())
    }

    /// Database key prefix for channels
    fn make_key(channel_id: &Hash) -> Vec<u8> {
        let mut key = vec![0x30]; // Prefix 0x30 for channels
        key.extend_from_slice(channel_id.as_bytes());
        key
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_open_and_get_channel() {
        let dir = tempdir().unwrap();
        let db = Arc::new(Database::create(dir.path().join("channel_test")).unwrap());
        let state = ChannelState::new(db).unwrap();

        let channel = Channel {
            channel_id: Hash::from_bytes([1u8; 32]),
            participant_a: Address::from_bytes([2u8; 32]),
            participant_b: Address::from_bytes([3u8; 32]),
            deposit_a: 10000,
            deposit_b: 5000,
            balance_a: 10000,
            balance_b: 5000,
            sequence: 0,
            dispute_timeout: 86400,
            status: ChannelStatus::Open,
            opened_at_height: 100,
            closed_at_height: None,
            dispute_started_at: None,
        };

        state.open_channel(channel.clone()).unwrap();

        let retrieved = state.get_channel(&channel.channel_id);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().capacity(), 15000);
    }

    #[test]
    fn test_update_channel_state() {
        let dir = tempdir().unwrap();
        let db = Arc::new(Database::create(dir.path().join("channel_update_test")).unwrap());
        let state = ChannelState::new(db).unwrap();

        let channel = Channel {
            channel_id: Hash::from_bytes([1u8; 32]),
            participant_a: Address::from_bytes([2u8; 32]),
            participant_b: Address::from_bytes([3u8; 32]),
            deposit_a: 10000,
            deposit_b: 5000,
            balance_a: 10000,
            balance_b: 5000,
            sequence: 0,
            dispute_timeout: 86400,
            status: ChannelStatus::Open,
            opened_at_height: 100,
            closed_at_height: None,
            dispute_started_at: None,
        };

        state.open_channel(channel.clone()).unwrap();

        // Update state: A sends 2000 to B
        state
            .update_channel_state(&channel.channel_id, 1, 8000, 7000)
            .unwrap();

        let updated = state.get_channel(&channel.channel_id).unwrap();
        assert_eq!(updated.balance_a, 8000);
        assert_eq!(updated.balance_b, 7000);
        assert_eq!(updated.sequence, 1);
    }

    #[test]
    fn test_cooperative_close() {
        let dir = tempdir().unwrap();
        let db = Arc::new(Database::create(dir.path().join("channel_close_test")).unwrap());
        let state = ChannelState::new(db).unwrap();

        let channel = Channel {
            channel_id: Hash::from_bytes([1u8; 32]),
            participant_a: Address::from_bytes([2u8; 32]),
            participant_b: Address::from_bytes([3u8; 32]),
            deposit_a: 10000,
            deposit_b: 5000,
            balance_a: 8000,
            balance_b: 7000,
            sequence: 5,
            dispute_timeout: 86400,
            status: ChannelStatus::Open,
            opened_at_height: 100,
            closed_at_height: None,
            dispute_started_at: None,
        };

        state.open_channel(channel.clone()).unwrap();

        state
            .close_cooperative(&channel.channel_id, 8000, 7000, 200)
            .unwrap();

        let closed = state.get_channel(&channel.channel_id).unwrap();
        assert_eq!(closed.status, ChannelStatus::ClosedCooperative);
        assert_eq!(closed.closed_at_height, Some(200));
    }
}
