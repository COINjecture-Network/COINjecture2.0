// Conditional escrow tracking
// Multi-party escrows with arbiter support
//
// Security model for authorization (P0):
//   All release and refund operations require the caller to present an
//   ed25519 signature over a canonical authorization message, proving they
//   control the authorizing address (recipient, sender, or arbiter).
//
//   Canonical message format:
//     domain_tag || escrow_id_bytes[32] || action_tag[1] || timestamp_le64
//
//   domain_tag  = b"COINJECT_ESCROW_AUTH_V1"  (22 bytes)
//   action_tag  = 0x01 for RELEASE, 0x02 for REFUND
//   timestamp   = Unix seconds (caller must supply; validated ± 300 s of now)
//
//   The timestamp window prevents replay attacks: a signature captured from
//   one operation cannot be reused after 5 minutes.
//
//   IMPORTANT: The `can_release` / `can_refund` methods check address-level
//   authorization only (no signature).  They are intended for quick eligibility
//   checks before constructing a transaction.  All ACTUAL state changes MUST
//   use `release_with_auth` / `refund_with_auth` which verify the signature.

use coinject_core::{Address, Balance, Hash};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
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

    // ─── Authorized release / refund (require ed25519 signature) ─────────────

    /// Release an escrow, transferring funds to the recipient.
    ///
    /// `releaser_pubkey` must correspond to an authorized address (recipient or arbiter).
    /// `signature` must be a valid ed25519 signature over the canonical auth message for
    /// this escrow, constructed with `escrow_auth_message(escrow_id, RELEASE_TAG, timestamp)`.
    /// `timestamp` is the Unix second that was signed; must be within ±300 s of `now`.
    ///
    /// Returns `Ok(())` on success or an error string describing the failure.
    pub fn release_with_auth(
        &self,
        escrow_id: &Hash,
        releaser_pubkey: &[u8; 32],
        signature: &[u8; 64],
        timestamp: i64,
        resolved_height: u64,
    ) -> Result<(), String> {
        let escrow = self
            .get_escrow(escrow_id)
            .ok_or_else(|| "Escrow not found".to_string())?;

        if escrow.status != EscrowStatus::Active {
            return Err(format!(
                "Escrow is not active (status: {:?})",
                escrow.status
            ));
        }

        // Derive the address the key claims to belong to and check eligibility.
        let releaser_address = address_from_pubkey(releaser_pubkey);
        if !self.can_release(escrow_id, &releaser_address) {
            return Err("Address is not authorized to release this escrow".to_string());
        }

        // Verify the ed25519 signature.
        verify_escrow_auth(
            escrow_id,
            releaser_pubkey,
            signature,
            timestamp,
            ACTION_RELEASE,
        )?;

        // All checks passed — update status.
        self.update_escrow_status(escrow_id, EscrowStatus::Released, Some(resolved_height))
    }

    /// Refund an escrow, returning funds to the sender.
    ///
    /// Authorization rules mirror [`can_refund`]: arbiter at any time, or sender after timeout.
    /// Requires a valid ed25519 signature (same format as [`release_with_auth`]).
    pub fn refund_with_auth(
        &self,
        escrow_id: &Hash,
        refunder_pubkey: &[u8; 32],
        signature: &[u8; 64],
        timestamp: i64,
        resolved_height: u64,
    ) -> Result<(), String> {
        let escrow = self
            .get_escrow(escrow_id)
            .ok_or_else(|| "Escrow not found".to_string())?;

        if escrow.status != EscrowStatus::Active {
            return Err(format!(
                "Escrow is not active (status: {:?})",
                escrow.status
            ));
        }

        let refunder_address = address_from_pubkey(refunder_pubkey);
        if !self.can_refund(escrow_id, &refunder_address) {
            return Err("Address is not authorized to refund this escrow".to_string());
        }

        verify_escrow_auth(
            escrow_id,
            refunder_pubkey,
            signature,
            timestamp,
            ACTION_REFUND,
        )?;

        self.update_escrow_status(escrow_id, EscrowStatus::Refunded, Some(resolved_height))
    }

    // ─── Internal helpers ──────────────────────────────────────────────────────

    /// Database key prefix for escrows
    fn make_key(escrow_id: &Hash) -> Vec<u8> {
        let mut key = vec![0x20]; // Prefix 0x20 for escrows
        key.extend_from_slice(escrow_id.as_bytes());
        key
    }
}

// ─── Authorization helpers ────────────────────────────────────────────────────

/// Action tag for RELEASE operations.
const ACTION_RELEASE: u8 = 0x01;
/// Action tag for REFUND operations.
const ACTION_REFUND: u8 = 0x02;
/// Domain tag prepended to all escrow auth messages.
const AUTH_DOMAIN: &[u8] = b"COINJECT_ESCROW_AUTH_V1";
/// Allowed timestamp skew in seconds (±5 minutes).
const MAX_TIMESTAMP_SKEW_SECS: i64 = 300;

/// Build the canonical escrow authorization message that the authorizer signs.
///
/// Layout: `domain[23] || escrow_id[32] || action[1] || timestamp_le64[8]` = 64 bytes
pub fn escrow_auth_message(escrow_id: &Hash, action: u8, timestamp: i64) -> [u8; 64] {
    let mut msg = [0u8; 64];
    let d = AUTH_DOMAIN.len(); // 23
    msg[..d].copy_from_slice(AUTH_DOMAIN);
    msg[d..d + 32].copy_from_slice(escrow_id.as_bytes());
    msg[d + 32] = action;
    msg[d + 33..d + 41].copy_from_slice(&(timestamp as u64).to_le_bytes());
    msg
}

/// Verify an escrow authorization signature.
fn verify_escrow_auth(
    escrow_id: &Hash,
    pubkey: &[u8; 32],
    signature: &[u8; 64],
    timestamp: i64,
    action: u8,
) -> Result<(), String> {
    // Validate timestamp freshness.
    let now = chrono::Utc::now().timestamp();
    let skew = (timestamp - now).abs();
    if skew > MAX_TIMESTAMP_SKEW_SECS {
        return Err(format!(
            "Authorization timestamp too far from current time (skew: {}s, max: {}s)",
            skew, MAX_TIMESTAMP_SKEW_SECS
        ));
    }

    // Parse the verifying key.
    let vk = VerifyingKey::from_bytes(pubkey).map_err(|e| format!("Invalid public key: {}", e))?;

    let sig = Signature::from_bytes(signature);
    let msg = escrow_auth_message(escrow_id, action, timestamp);

    vk.verify(&msg, &sig)
        .map_err(|_| "Escrow authorization signature verification failed".to_string())
}

/// Derive an `Address` from a 32-byte ed25519 public key using BLAKE3.
///
/// This matches the derivation used by the node's validator keystore.
fn address_from_pubkey(pubkey: &[u8; 32]) -> Address {
    let hash = blake3::hash(pubkey);
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(hash.as_bytes());
    Address::from_bytes(bytes)
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

    /// Build an escrow whose releaser/refunder addresses are derived from `key`.
    fn make_escrow_for_key(escrow_id: Hash, sk: &ed25519_dalek::SigningKey, role: &str) -> Escrow {
        let pk = sk.verifying_key().to_bytes();
        let addr = super::address_from_pubkey(&pk);
        let other = Address::from_bytes([0xAA; 32]);

        match role {
            "recipient" => Escrow {
                escrow_id,
                sender: other,
                recipient: addr,
                arbiter: None,
                amount: 100,
                timeout: chrono::Utc::now().timestamp() + 86400,
                conditions_hash: Hash::from_bytes([9u8; 32]),
                status: EscrowStatus::Active,
                created_at_height: 1,
                resolved_at_height: None,
            },
            "sender" => Escrow {
                escrow_id,
                sender: addr,
                recipient: other,
                arbiter: None,
                amount: 100,
                timeout: chrono::Utc::now().timestamp() - 10, // already expired
                conditions_hash: Hash::from_bytes([9u8; 32]),
                status: EscrowStatus::Active,
                created_at_height: 1,
                resolved_at_height: None,
            },
            _ => panic!("Unknown role"),
        }
    }

    #[test]
    fn test_release_with_valid_signature() {
        use ed25519_dalek::{Signer, SigningKey};
        use rand::rngs::OsRng;

        let dir = tempdir().unwrap();
        let db = Arc::new(Database::create(dir.path().join("signed_release_test")).unwrap());
        let state = EscrowState::new(db).unwrap();

        let sk = SigningKey::generate(&mut OsRng);
        let pk = sk.verifying_key().to_bytes();
        let escrow_id = Hash::from_bytes([0xCC; 32]);

        let escrow = make_escrow_for_key(escrow_id, &sk, "recipient");
        state.create_escrow(escrow).unwrap();

        let now = chrono::Utc::now().timestamp();
        let msg = super::escrow_auth_message(&escrow_id, super::ACTION_RELEASE, now);
        let sig = sk.sign(&msg);
        let sig_bytes: [u8; 64] = sig.to_bytes();

        let result = state.release_with_auth(&escrow_id, &pk, &sig_bytes, now, 42);
        assert!(
            result.is_ok(),
            "Valid signature should succeed: {:?}",
            result
        );

        let updated = state.get_escrow(&escrow_id).unwrap();
        assert_eq!(updated.status, EscrowStatus::Released);
    }

    #[test]
    fn test_release_with_wrong_signature_rejected() {
        use ed25519_dalek::{Signer, SigningKey};
        use rand::rngs::OsRng;

        let dir = tempdir().unwrap();
        let db = Arc::new(Database::create(dir.path().join("bad_sig_test")).unwrap());
        let state = EscrowState::new(db).unwrap();

        let sk = SigningKey::generate(&mut OsRng);
        let pk = sk.verifying_key().to_bytes();
        let escrow_id = Hash::from_bytes([0xDD; 32]);

        let escrow = make_escrow_for_key(escrow_id, &sk, "recipient");
        state.create_escrow(escrow).unwrap();

        let now = chrono::Utc::now().timestamp();
        // Sign with wrong action (REFUND instead of RELEASE)
        let msg = super::escrow_auth_message(&escrow_id, super::ACTION_REFUND, now);
        let sig = sk.sign(&msg);
        let sig_bytes: [u8; 64] = sig.to_bytes();

        let result = state.release_with_auth(&escrow_id, &pk, &sig_bytes, now, 42);
        assert!(
            result.is_err(),
            "Signature for wrong action must be rejected"
        );
    }

    #[test]
    fn test_refund_after_timeout_with_valid_signature() {
        use ed25519_dalek::{Signer, SigningKey};
        use rand::rngs::OsRng;

        let dir = tempdir().unwrap();
        let db = Arc::new(Database::create(dir.path().join("refund_test")).unwrap());
        let state = EscrowState::new(db).unwrap();

        let sk = SigningKey::generate(&mut OsRng);
        let pk = sk.verifying_key().to_bytes();
        let escrow_id = Hash::from_bytes([0xEE; 32]);

        // Escrow already expired — sender can refund
        let escrow = make_escrow_for_key(escrow_id, &sk, "sender");
        state.create_escrow(escrow).unwrap();

        let now = chrono::Utc::now().timestamp();
        let msg = super::escrow_auth_message(&escrow_id, super::ACTION_REFUND, now);
        let sig = sk.sign(&msg);
        let sig_bytes: [u8; 64] = sig.to_bytes();

        let result = state.refund_with_auth(&escrow_id, &pk, &sig_bytes, now, 43);
        assert!(
            result.is_ok(),
            "Valid refund after timeout should succeed: {:?}",
            result
        );
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
