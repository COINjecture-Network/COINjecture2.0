//! Thread-safe, TTL-expiring nonce store for SIWB challenge–response auth.
//!
//! - DashMap-backed for lock-free concurrent access
//! - 300 s per-nonce TTL
//! - Background cleanup every 60 s (spawned by caller)
//! - Hard cap of `max_entries` to prevent memory exhaustion

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use std::fmt;

/// A pending SIWB nonce with its associated challenge data.
pub struct NonceEntry {
    pub wallet_address: String,
    pub message: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

/// Errors that can occur when interacting with the nonce store.
#[derive(Debug)]
pub enum NonceError {
    NotFound,
    Expired,
    StoreFull,
    WalletMismatch,
    MessageMismatch,
}

impl fmt::Display for NonceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => write!(f, "Nonce not found or already used"),
            Self::Expired => write!(f, "Nonce expired"),
            Self::StoreFull => write!(f, "Too many pending challenges"),
            Self::WalletMismatch => write!(f, "Wallet address mismatch"),
            Self::MessageMismatch => write!(f, "Message mismatch"),
        }
    }
}

pub struct NonceStore {
    entries: DashMap<String, NonceEntry>,
    max_entries: usize,
}

impl NonceStore {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: DashMap::new(),
            max_entries,
        }
    }

    /// Insert a nonce. Fails if the store is at capacity.
    pub fn insert(&self, nonce: String, entry: NonceEntry) -> Result<(), NonceError> {
        if self.entries.len() >= self.max_entries {
            return Err(NonceError::StoreFull);
        }
        self.entries.insert(nonce, entry);
        Ok(())
    }

    /// Atomically remove and validate a nonce against the expected wallet and message.
    ///
    /// The nonce is consumed regardless of validation outcome (one-time use).
    pub fn validate_and_remove(
        &self,
        nonce: &str,
        wallet_address: &str,
        message: &str,
    ) -> Result<NonceEntry, NonceError> {
        let (_, entry) = self.entries.remove(nonce).ok_or(NonceError::NotFound)?;

        if entry.expires_at < Utc::now() {
            return Err(NonceError::Expired);
        }
        if entry.wallet_address != wallet_address {
            return Err(NonceError::WalletMismatch);
        }
        if entry.message != message {
            return Err(NonceError::MessageMismatch);
        }

        Ok(entry)
    }

    /// Remove all expired entries. Returns the number removed.
    pub fn cleanup_expired(&self) -> usize {
        let now = Utc::now();
        let before = self.entries.len();
        self.entries.retain(|_, entry| entry.expires_at > now);
        before - self.entries.len()
    }

    /// Current number of pending nonces.
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the store has no pending nonces.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
