// COINjecture Core — Unified Error Types
//
// All library crates use thiserror for typed errors.
// Application crates (node, wallet) use anyhow for context enrichment.

use thiserror::Error;

// ---------------------------------------------------------------------------
// Crypto errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("invalid key length: expected {expected}, got {got}")]
    InvalidKeyLength { expected: usize, got: usize },

    #[error("signature verification failed")]
    SignatureVerification,

    #[error("key generation failed")]
    KeyGeneration,

    #[error("invalid hex encoding: {0}")]
    HexDecode(#[from] hex::FromHexError),

    #[error("serialization failed: {0}")]
    Serialization(String),
}

// ---------------------------------------------------------------------------
// Block / chain errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum BlockError {
    #[error("chain is empty — no genesis block")]
    EmptyChain,

    #[error("block at height {0} not found")]
    NotFound(u64),

    #[error("invalid block hash")]
    InvalidHash,

    #[error("invalid block signature")]
    InvalidSignature,

    #[error("block height mismatch: expected {expected}, got {got}")]
    HeightMismatch { expected: u64, got: u64 },

    #[error("invalid previous hash")]
    InvalidPrevHash,

    #[error("timestamp too old or in the future")]
    InvalidTimestamp,

    #[error("serialization error: {0}")]
    Serialization(String),
}

// ---------------------------------------------------------------------------
// Transaction errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum TransactionError {
    #[error("insufficient balance: have {have}, need {need}")]
    InsufficientBalance { have: u128, need: u128 },

    #[error("invalid signature")]
    InvalidSignature,

    #[error("invalid nonce: expected {expected}, got {got}")]
    InvalidNonce { expected: u64, got: u64 },

    #[error("transaction already in mempool")]
    Duplicate,

    #[error("transaction expired")]
    Expired,

    #[error("zero-amount transfer is not allowed")]
    ZeroAmount,

    #[error("recipient address is invalid")]
    InvalidRecipient,

    #[error("timelock has not expired yet (unlocks at {unlock_time})")]
    TimeLockActive { unlock_time: i64 },

    #[error("serialization error: {0}")]
    Serialization(String),
}

// ---------------------------------------------------------------------------
// Consensus errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum ConsensusError {
    #[error("no peers available for leader election")]
    NoPeers,

    #[error("invalid solution for problem")]
    InvalidSolution,

    #[error("work score below minimum threshold")]
    InsufficientWork,

    #[error("epoch transition failed: {0}")]
    EpochError(String),

    #[error("problem registry entry not found: {0}")]
    ProblemNotRegistered(String),

    #[error("mining timeout")]
    MiningTimeout,

    #[error("difficulty adjustment error: {0}")]
    DifficultyAdjustment(String),
}

// ---------------------------------------------------------------------------
// Network errors  (re-exported from network crate but defined here for core use)
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum NetworkError {
    #[error("connection refused to {addr}")]
    ConnectionRefused { addr: String },

    #[error("peer disconnected")]
    PeerDisconnected,

    #[error("message deserialization failed: {0}")]
    DeserializationFailed(String),

    #[error("message too large: {size} bytes (limit: {limit})")]
    MessageTooLarge { size: usize, limit: usize },

    #[error("handshake failed: {0}")]
    HandshakeFailed(String),

    #[error("protocol version mismatch: we={ours}, peer={theirs}")]
    VersionMismatch { ours: u8, theirs: u8 },

    #[error("rate limit exceeded")]
    RateLimited,

    #[error("operation timed out")]
    Timeout,
}

// ---------------------------------------------------------------------------
// State errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum StateError {
    #[error("account not found: {0}")]
    AccountNotFound(String),

    #[error("insufficient balance")]
    InsufficientBalance,

    #[error("database error: {0}")]
    Database(String),

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("nonce overflow")]
    NonceOverflow,
}

// ---------------------------------------------------------------------------
// Configuration errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("missing required field: {0}")]
    MissingField(String),

    #[error("invalid value for {field}: {reason}")]
    InvalidValue { field: String, reason: String },

    #[error("config file not found at {path}")]
    FileNotFound { path: String },

    #[error("failed to parse config file: {0}")]
    ParseError(String),

    #[error("invalid network address: {0}")]
    InvalidAddress(String),
}

// ---------------------------------------------------------------------------
// Helper: current Unix timestamp (seconds), never panics
// ---------------------------------------------------------------------------

/// Returns the current Unix timestamp in seconds.
/// Falls back to 0 if the system clock is before the Unix epoch (essentially impossible
/// on production hardware, but we handle it gracefully rather than panicking).
#[inline]
pub fn unix_now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Returns the current Unix timestamp in seconds as i64.
#[inline]
pub fn unix_now_secs_i64() -> i64 {
    unix_now_secs() as i64
}
