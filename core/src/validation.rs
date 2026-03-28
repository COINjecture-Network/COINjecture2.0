// =============================================================================
// Phase 6: Input Validation & Sanitization
// =============================================================================
// Central validation module for all consensus-critical inputs.
//
// Provides:
//   - Amount type helpers with overflow-safe arithmetic
//   - Transaction field validation (amounts, fees, payload sizes)
//   - Block header structural validation (version, timestamps, f64 bounds)
//   - Network message validation (range limits, string lengths, enum bounds)
//   - Peer address policy enforcement (reject private/loopback in production)
//   - File path traversal prevention
//   - Log injection / XSS sanitization

use crate::{Balance, Timestamp};
use std::net::SocketAddr;
use std::path::Path;

// =============================================================================
// Bounds constants
// =============================================================================

/// Maximum amount in a single field (half of u128::MAX prevents amount+fee overflow)
pub const MAX_AMOUNT: Balance = u128::MAX / 2;

/// Minimum transaction fee (non-zero prevents spam)
pub const MIN_FEE: Balance = 1;

/// Maximum transaction data payload (64 KB)
pub const MAX_TX_DATA_SIZE: usize = 64 * 1024;

/// Maximum number of transactions per block
pub const MAX_BLOCK_TRANSACTIONS: usize = 10_000;

/// Maximum additional signatures on escrow / channel transactions
pub const MAX_ADDITIONAL_SIGNATURES: usize = 8;

/// Maximum dispute proof bytes
pub const MAX_DISPUTE_PROOF_SIZE: usize = 1_024;

/// Maximum length for human-readable reason / label fields in messages
pub const MAX_REASON_STRING_LEN: usize = 256;

/// Maximum blocks that may be requested in a single GetBlocks message
pub const MAX_BLOCKS_PER_REQUEST: u64 = 512;

/// Maximum headers that may be requested in a single GetHeaders message
pub const MAX_HEADERS_PER_REQUEST: u32 = 2_048;

/// Maximum future timestamp drift allowed (seconds)
pub const MAX_FUTURE_DRIFT_SECS: i64 = 120;

/// Supported block versions (must stay in sync with block.rs constants)
pub const SUPPORTED_BLOCK_VERSIONS: &[u32] = &[1, 2];

// =============================================================================
// ValidationError
// =============================================================================

#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    // --- Amount / fee ---
    #[error("amount is zero")]
    ZeroAmount,

    #[error("amount {0} exceeds maximum allowed value")]
    AmountOverflow(Balance),

    #[error("fee {fee} is below the required minimum")]
    FeeTooLow { fee: Balance },

    #[error("amount {amount} + fee {fee} would overflow u128")]
    AmountPlusFeeOverflow { amount: Balance, fee: Balance },

    // --- Payload / size ---
    #[error("data payload size {0} bytes exceeds maximum")]
    DataTooLarge(usize),

    #[error("block transaction count {0} exceeds maximum")]
    TooManyTransactions(usize),

    #[error("too many additional signatures: {0}")]
    TooManySignatures(usize),

    #[error("dispute proof size {0} bytes exceeds maximum")]
    DisputeProofTooLarge(usize),

    #[error("string field length {0} bytes exceeds maximum")]
    StringTooLong(usize),

    // --- Block header ---
    #[error("block version {0} is not supported")]
    UnsupportedVersion(u32),

    #[error("timestamp {0} is negative")]
    NegativeTimestamp(i64),

    #[error("timestamp {0} is too far in the future")]
    FutureTimestamp(i64),

    #[error("work_score {0} is not finite (NaN or Inf)")]
    NonFiniteWorkScore(f64),

    #[error("work_score {0} is negative")]
    NegativeWorkScore(f64),

    #[error("time_asymmetry_ratio {0} is not finite")]
    NonFiniteTimeAsymmetry(f64),

    #[error("solution_quality {0} is outside valid range [0.0, 1.0]")]
    InvalidSolutionQuality(f64),

    #[error("field value {0} is negative or not finite")]
    InvalidNonNegativeFloat(f64),

    // --- Network messages ---
    #[error("GetBlocks range inverted: from={from} > to={to}")]
    BlockRangeInverted { from: u64, to: u64 },

    #[error("GetBlocks requested {0} blocks, exceeds limit")]
    BlockRequestRangeExceeded(u64),

    #[error("GetHeaders requested {0} headers, exceeds limit")]
    HeadersRequestTooBig(u32),

    #[error("Blocks response contained {0} blocks but request range was smaller")]
    TooManyBlocksInResponse(usize),

    #[error("node_type byte {0} is not a valid NodeType variant")]
    InvalidNodeType(u8),

    // --- Peer / address ---
    #[error("peer address {0} is forbidden (loopback/private/broadcast)")]
    ForbiddenPeerAddress(String),

    #[error("port {0} is out of valid range 1–65535")]
    InvalidPort(u32),

    #[error("socket address parse failed: {0}")]
    InvalidSocketAddr(String),

    // --- File path ---
    #[error("path traversal detected in: {0}")]
    PathTraversal(String),

    #[error("path contains null bytes: {0}")]
    PathContainsNull(String),
}

// =============================================================================
// Amount arithmetic helpers
// =============================================================================

/// Assert `amount > 0` and `amount <= MAX_AMOUNT`.
#[inline]
pub fn validate_amount(amount: Balance) -> Result<(), ValidationError> {
    if amount == 0 {
        return Err(ValidationError::ZeroAmount);
    }
    if amount > MAX_AMOUNT {
        return Err(ValidationError::AmountOverflow(amount));
    }
    Ok(())
}

/// Assert `fee >= MIN_FEE` and `fee <= MAX_AMOUNT`.
#[inline]
pub fn validate_fee(fee: Balance) -> Result<(), ValidationError> {
    if fee < MIN_FEE {
        return Err(ValidationError::FeeTooLow { fee });
    }
    if fee > MAX_AMOUNT {
        return Err(ValidationError::AmountOverflow(fee));
    }
    Ok(())
}

/// Overflow-safe addition for Balance values.
#[inline]
pub fn checked_add(a: Balance, b: Balance) -> Result<Balance, ValidationError> {
    a.checked_add(b)
        .ok_or(ValidationError::AmountPlusFeeOverflow { amount: a, fee: b })
}

/// Overflow-safe subtraction for Balance values (returns error on underflow).
#[inline]
pub fn checked_sub(a: Balance, b: Balance) -> Result<Balance, ValidationError> {
    a.checked_sub(b)
        .ok_or(ValidationError::AmountPlusFeeOverflow { amount: a, fee: b })
}

/// Validate both amount and fee, plus their sum (prevents overflow in cost checks).
pub fn validate_amount_and_fee(amount: Balance, fee: Balance) -> Result<(), ValidationError> {
    validate_amount(amount)?;
    validate_fee(fee)?;
    // Ensure amount + fee does not overflow u128
    amount
        .checked_add(fee)
        .ok_or(ValidationError::AmountPlusFeeOverflow { amount, fee })?;
    Ok(())
}

// =============================================================================
// Transaction field validation
// =============================================================================

/// Validate Transfer transaction fields.
pub fn validate_transfer_fields(amount: Balance, fee: Balance) -> Result<(), ValidationError> {
    validate_amount_and_fee(amount, fee)
}

/// Validate TimeLock transaction fields.
pub fn validate_timelock_fields(amount: Balance, fee: Balance) -> Result<(), ValidationError> {
    validate_amount_and_fee(amount, fee)
}

/// Validate Escrow Create amount and fee.
pub fn validate_escrow_fields(amount: Balance, fee: Balance) -> Result<(), ValidationError> {
    validate_amount_and_fee(amount, fee)
}

/// Validate the number of additional signatures on an escrow / channel tx.
pub fn validate_additional_signatures_count(count: usize) -> Result<(), ValidationError> {
    if count > MAX_ADDITIONAL_SIGNATURES {
        return Err(ValidationError::TooManySignatures(count));
    }
    Ok(())
}

/// Validate dispute proof size.
pub fn validate_dispute_proof(proof: &[u8]) -> Result<(), ValidationError> {
    if proof.len() > MAX_DISPUTE_PROOF_SIZE {
        return Err(ValidationError::DisputeProofTooLarge(proof.len()));
    }
    Ok(())
}

/// Validate an arbitrary data payload (marketplace / custom problems).
pub fn validate_data_payload(data: &[u8]) -> Result<(), ValidationError> {
    if data.len() > MAX_TX_DATA_SIZE {
        return Err(ValidationError::DataTooLarge(data.len()));
    }
    Ok(())
}

/// Validate a human-readable string field (reason, label, etc.).
pub fn validate_string_field(s: &str) -> Result<(), ValidationError> {
    if s.len() > MAX_REASON_STRING_LEN {
        return Err(ValidationError::StringTooLong(s.len()));
    }
    Ok(())
}

// =============================================================================
// Block header field validation
// =============================================================================

/// Validate all derived f64 / structural fields of a block header.
///
/// Checks:
/// - `version` is in SUPPORTED_BLOCK_VERSIONS
/// - `timestamp` is non-negative and not too far in the future
/// - `work_score` is finite and non-negative
/// - `time_asymmetry_ratio` is finite
/// - `solution_quality` is in [0.0, 1.0]
/// - `complexity_weight` and `energy_estimate_joules` are finite and non-negative
/// - `tx_count` ≤ MAX_BLOCK_TRANSACTIONS
pub fn validate_block_header_fields(
    version: u32,
    timestamp: Timestamp,
    work_score: f64,
    time_asymmetry_ratio: f64,
    solution_quality: f64,
    complexity_weight: f64,
    energy_estimate_joules: f64,
    tx_count: usize,
    now_secs: i64,
) -> Result<(), ValidationError> {
    // Block version
    if !SUPPORTED_BLOCK_VERSIONS.contains(&version) {
        return Err(ValidationError::UnsupportedVersion(version));
    }

    // Timestamp: no negative values
    if timestamp < 0 {
        return Err(ValidationError::NegativeTimestamp(timestamp));
    }

    // Timestamp: not too far in the future
    if timestamp > now_secs + MAX_FUTURE_DRIFT_SECS {
        return Err(ValidationError::FutureTimestamp(timestamp));
    }

    // Work score: finite and non-negative
    if !work_score.is_finite() {
        return Err(ValidationError::NonFiniteWorkScore(work_score));
    }
    if work_score < 0.0 {
        return Err(ValidationError::NegativeWorkScore(work_score));
    }

    // Time asymmetry: finite
    if !time_asymmetry_ratio.is_finite() {
        return Err(ValidationError::NonFiniteTimeAsymmetry(
            time_asymmetry_ratio,
        ));
    }

    // Solution quality: in [0.0, 1.0]
    if !solution_quality.is_finite() || solution_quality < 0.0 || solution_quality > 1.0 {
        return Err(ValidationError::InvalidSolutionQuality(solution_quality));
    }

    // Complexity weight: finite and non-negative
    if !complexity_weight.is_finite() || complexity_weight < 0.0 {
        return Err(ValidationError::InvalidNonNegativeFloat(complexity_weight));
    }

    // Energy estimate: finite and non-negative
    if !energy_estimate_joules.is_finite() || energy_estimate_joules < 0.0 {
        return Err(ValidationError::InvalidNonNegativeFloat(
            energy_estimate_joules,
        ));
    }

    // Transaction count
    if tx_count > MAX_BLOCK_TRANSACTIONS {
        return Err(ValidationError::TooManyTransactions(tx_count));
    }

    Ok(())
}

// =============================================================================
// Network message validation
// =============================================================================

/// Validate a GetBlocks height range.
pub fn validate_get_blocks_range(from_height: u64, to_height: u64) -> Result<(), ValidationError> {
    if from_height > to_height {
        return Err(ValidationError::BlockRangeInverted {
            from: from_height,
            to: to_height,
        });
    }
    let range = to_height.saturating_sub(from_height).saturating_add(1);
    if range > MAX_BLOCKS_PER_REQUEST {
        return Err(ValidationError::BlockRequestRangeExceeded(range));
    }
    Ok(())
}

/// Validate a GetHeaders `max_headers` field.
pub fn validate_get_headers(max_headers: u32) -> Result<(), ValidationError> {
    if max_headers > MAX_HEADERS_PER_REQUEST {
        return Err(ValidationError::HeadersRequestTooBig(max_headers));
    }
    Ok(())
}

/// Validate the `node_type` byte used in handshake messages.
/// Valid range: 0–5 (Light=0, Full=1, Archive=2, Validator=3, Bounty=4, Oracle=5).
pub fn validate_node_type_byte(node_type: u8) -> Result<(), ValidationError> {
    if node_type > 5 {
        return Err(ValidationError::InvalidNodeType(node_type));
    }
    Ok(())
}

/// Validate a disconnect / rejection reason string.
pub fn validate_reason_string(reason: &str) -> Result<(), ValidationError> {
    validate_string_field(reason)
}

/// Validate that a Blocks response does not contain more blocks than the
/// request range implied.
pub fn validate_blocks_response_count(
    count: usize,
    requested_range: u64,
) -> Result<(), ValidationError> {
    if count as u64 > requested_range {
        return Err(ValidationError::TooManyBlocksInResponse(count));
    }
    Ok(())
}

// =============================================================================
// Peer address policy
// =============================================================================

/// Returns `true` when the IP belongs to a loopback / private / broadcast /
/// unspecified / link-local address class.
pub fn is_private_or_loopback(addr: &SocketAddr) -> bool {
    use std::net::IpAddr;
    match addr.ip() {
        IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_broadcast()
                || v4.is_multicast()
                || v4.is_unspecified()
                || v4.is_link_local()
        }
        IpAddr::V6(v6) => v6.is_loopback() || v6.is_multicast() || v6.is_unspecified(),
    }
}

/// Validate a peer socket address.
///
/// - Port must be non-zero.
/// - When `allow_private = false` (production mode), rejects loopback / private /
///   broadcast / unspecified addresses.
pub fn validate_peer_address(
    addr: &SocketAddr,
    allow_private: bool,
) -> Result<(), ValidationError> {
    if addr.port() == 0 {
        return Err(ValidationError::InvalidPort(0));
    }
    if !allow_private && is_private_or_loopback(addr) {
        return Err(ValidationError::ForbiddenPeerAddress(addr.to_string()));
    }
    Ok(())
}

/// Parse and validate a peer address string.
pub fn validate_peer_addr_str(
    addr_str: &str,
    allow_private: bool,
) -> Result<SocketAddr, ValidationError> {
    let addr: SocketAddr = addr_str
        .parse()
        .map_err(|_| ValidationError::InvalidSocketAddr(addr_str.to_string()))?;
    validate_peer_address(&addr, allow_private)?;
    Ok(addr)
}

// =============================================================================
// Configuration validation helpers
// =============================================================================

/// Validate that a TCP port number is in the valid range 1–65535.
pub fn validate_port(port: u32) -> Result<(), ValidationError> {
    if port == 0 || port > 65535 {
        return Err(ValidationError::InvalidPort(port));
    }
    Ok(())
}

/// Parse a "host:port" socket address string and validate its port.
pub fn validate_socket_addr_str(addr: &str) -> Result<(), ValidationError> {
    let parsed: SocketAddr = addr
        .parse()
        .map_err(|_| ValidationError::InvalidSocketAddr(addr.to_string()))?;
    validate_port(parsed.port() as u32)
}

// =============================================================================
// File path traversal prevention
// =============================================================================

/// Validate a file path string against path traversal attacks.
///
/// Rejects:
/// - Paths containing null bytes
/// - Paths with `..` components (e.g., `../../etc/passwd`)
pub fn validate_file_path(path: &str) -> Result<(), ValidationError> {
    // Null byte injection
    if path.contains('\0') {
        return Err(ValidationError::PathContainsNull(path.to_string()));
    }

    // Normalise separators for the string-level check
    let normalised = path.replace('\\', "/");
    if normalised.contains("../") || normalised.ends_with("/..") || normalised == ".." {
        return Err(ValidationError::PathTraversal(path.to_string()));
    }

    // Component-level check (handles Windows UNC, root-relative, etc.)
    for component in Path::new(path).components() {
        if matches!(component, std::path::Component::ParentDir) {
            return Err(ValidationError::PathTraversal(path.to_string()));
        }
    }

    Ok(())
}

// =============================================================================
// Log sanitization
// =============================================================================

/// Sanitize a user-provided string before writing it to a log file.
///
/// Replaces newlines and carriage returns with a space to prevent log injection,
/// and replaces other control characters with `?`.
pub fn sanitize_for_log(input: &str) -> String {
    input
        .chars()
        .map(|c| match c {
            '\n' | '\r' | '\t' => ' ',
            c if c.is_control() => '?',
            c => c,
        })
        .collect()
}

/// Sanitize a user-provided string for inclusion in HTML output (basic XSS prevention).
pub fn sanitize_for_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ---- amount helpers ----

    #[test]
    fn amount_zero_rejected() {
        assert!(matches!(
            validate_amount(0),
            Err(ValidationError::ZeroAmount)
        ));
    }

    #[test]
    fn amount_max_valid() {
        assert!(validate_amount(MAX_AMOUNT).is_ok());
    }

    #[test]
    fn amount_over_max_rejected() {
        assert!(matches!(
            validate_amount(MAX_AMOUNT + 1),
            Err(ValidationError::AmountOverflow(_))
        ));
        assert!(matches!(
            validate_amount(u128::MAX),
            Err(ValidationError::AmountOverflow(_))
        ));
    }

    #[test]
    fn fee_zero_rejected() {
        assert!(matches!(
            validate_fee(0),
            Err(ValidationError::FeeTooLow { fee: 0 })
        ));
    }

    #[test]
    fn fee_minimum_valid() {
        assert!(validate_fee(MIN_FEE).is_ok());
    }

    #[test]
    fn amount_and_fee_sum_overflow() {
        // Both individually at MAX_AMOUNT — sum overflows.
        assert!(matches!(
            validate_amount_and_fee(MAX_AMOUNT, MAX_AMOUNT),
            Err(ValidationError::AmountPlusFeeOverflow { .. })
        ));
    }

    #[test]
    fn checked_add_overflow() {
        assert!(matches!(
            checked_add(u128::MAX, 1),
            Err(ValidationError::AmountPlusFeeOverflow { .. })
        ));
    }

    #[test]
    fn checked_add_valid() {
        assert_eq!(checked_add(100, 200).unwrap(), 300);
    }

    #[test]
    fn checked_sub_underflow() {
        assert!(checked_sub(5, 10).is_err());
    }

    #[test]
    fn checked_sub_valid() {
        assert_eq!(checked_sub(10, 3).unwrap(), 7);
    }

    // ---- transaction size guards ----

    #[test]
    fn data_payload_too_large() {
        let huge = vec![0u8; MAX_TX_DATA_SIZE + 1];
        assert!(matches!(
            validate_data_payload(&huge),
            Err(ValidationError::DataTooLarge(_))
        ));
    }

    #[test]
    fn data_payload_at_limit_ok() {
        let ok = vec![0u8; MAX_TX_DATA_SIZE];
        assert!(validate_data_payload(&ok).is_ok());
    }

    #[test]
    fn dispute_proof_too_large() {
        let big = vec![0u8; MAX_DISPUTE_PROOF_SIZE + 1];
        assert!(matches!(
            validate_dispute_proof(&big),
            Err(ValidationError::DisputeProofTooLarge(_))
        ));
    }

    #[test]
    fn additional_signatures_too_many() {
        assert!(validate_additional_signatures_count(MAX_ADDITIONAL_SIGNATURES + 1).is_err());
        assert!(validate_additional_signatures_count(MAX_ADDITIONAL_SIGNATURES).is_ok());
    }

    #[test]
    fn string_field_too_long() {
        let long = "x".repeat(MAX_REASON_STRING_LEN + 1);
        assert!(validate_string_field(&long).is_err());
    }

    #[test]
    fn string_field_empty_ok() {
        assert!(validate_string_field("").is_ok());
    }

    // ---- block header fields ----

    fn valid_header_call(tx_count: usize, now: i64) -> Result<(), ValidationError> {
        validate_block_header_fields(1, now, 1.5, 100.0, 0.8, 10.0, 50.0, tx_count, now)
    }

    #[test]
    fn block_header_valid() {
        assert!(valid_header_call(100, 1_700_000_000).is_ok());
    }

    #[test]
    fn block_header_bad_version() {
        let now = 1_700_000_000i64;
        assert!(matches!(
            validate_block_header_fields(99, now, 1.5, 100.0, 0.8, 10.0, 50.0, 100, now),
            Err(ValidationError::UnsupportedVersion(99))
        ));
    }

    #[test]
    fn block_header_future_timestamp() {
        let now = 1_700_000_000i64;
        let future = now + MAX_FUTURE_DRIFT_SECS + 1;
        assert!(matches!(
            validate_block_header_fields(1, future, 1.5, 100.0, 0.8, 10.0, 50.0, 100, now),
            Err(ValidationError::FutureTimestamp(_))
        ));
    }

    #[test]
    fn block_header_negative_timestamp() {
        let now = 1_700_000_000i64;
        assert!(matches!(
            validate_block_header_fields(1, -1, 1.5, 100.0, 0.8, 10.0, 50.0, 100, now),
            Err(ValidationError::NegativeTimestamp(_))
        ));
    }

    #[test]
    fn block_header_nan_work_score() {
        let now = 1_700_000_000i64;
        assert!(matches!(
            validate_block_header_fields(1, now, f64::NAN, 100.0, 0.8, 10.0, 50.0, 100, now),
            Err(ValidationError::NonFiniteWorkScore(_))
        ));
    }

    #[test]
    fn block_header_inf_work_score() {
        let now = 1_700_000_000i64;
        assert!(matches!(
            validate_block_header_fields(1, now, f64::INFINITY, 100.0, 0.8, 10.0, 50.0, 100, now),
            Err(ValidationError::NonFiniteWorkScore(_))
        ));
    }

    #[test]
    fn block_header_negative_work_score() {
        let now = 1_700_000_000i64;
        assert!(matches!(
            validate_block_header_fields(1, now, -0.1, 100.0, 0.8, 10.0, 50.0, 100, now),
            Err(ValidationError::NegativeWorkScore(_))
        ));
    }

    #[test]
    fn block_header_solution_quality_out_of_range() {
        let now = 1_700_000_000i64;
        assert!(matches!(
            validate_block_header_fields(1, now, 1.5, 100.0, 1.5, 10.0, 50.0, 100, now),
            Err(ValidationError::InvalidSolutionQuality(1.5))
        ));
        assert!(matches!(
            validate_block_header_fields(1, now, 1.5, 100.0, -0.1, 10.0, 50.0, 100, now),
            Err(ValidationError::InvalidSolutionQuality(-0.1))
        ));
        assert!(matches!(
            validate_block_header_fields(1, now, 1.5, 100.0, f64::NAN, 10.0, 50.0, 100, now),
            Err(ValidationError::InvalidSolutionQuality(_))
        ));
    }

    #[test]
    fn block_header_too_many_transactions() {
        let now = 1_700_000_000i64;
        assert!(matches!(
            valid_header_call(MAX_BLOCK_TRANSACTIONS + 1, now),
            Err(ValidationError::TooManyTransactions(_))
        ));
    }

    // ---- network message validation ----

    #[test]
    fn get_blocks_range_valid() {
        assert!(validate_get_blocks_range(0, 10).is_ok());
        assert!(validate_get_blocks_range(5, 5).is_ok()); // single block
    }

    #[test]
    fn get_blocks_range_inverted() {
        assert!(matches!(
            validate_get_blocks_range(100, 50),
            Err(ValidationError::BlockRangeInverted { from: 100, to: 50 })
        ));
    }

    #[test]
    fn get_blocks_range_too_big() {
        // range = MAX_BLOCKS_PER_REQUEST + 1
        let to = MAX_BLOCKS_PER_REQUEST; // from=0, to=512 → range=513
        assert!(matches!(
            validate_get_blocks_range(0, to),
            Err(ValidationError::BlockRequestRangeExceeded(_))
        ));
    }

    #[test]
    fn get_headers_valid() {
        assert!(validate_get_headers(1).is_ok());
        assert!(validate_get_headers(MAX_HEADERS_PER_REQUEST).is_ok());
    }

    #[test]
    fn get_headers_too_many() {
        assert!(matches!(
            validate_get_headers(MAX_HEADERS_PER_REQUEST + 1),
            Err(ValidationError::HeadersRequestTooBig(_))
        ));
    }

    #[test]
    fn node_type_byte_valid_range() {
        for i in 0u8..=5 {
            assert!(
                validate_node_type_byte(i).is_ok(),
                "byte {} should be valid",
                i
            );
        }
    }

    #[test]
    fn node_type_byte_invalid() {
        assert!(matches!(
            validate_node_type_byte(6),
            Err(ValidationError::InvalidNodeType(6))
        ));
        assert!(matches!(
            validate_node_type_byte(255),
            Err(ValidationError::InvalidNodeType(255))
        ));
    }

    // ---- peer address ----

    #[test]
    fn loopback_rejected_in_prod() {
        let addr: SocketAddr = "127.0.0.1:707".parse().unwrap();
        assert!(validate_peer_address(&addr, false).is_err());
    }

    #[test]
    fn loopback_allowed_in_dev() {
        let addr: SocketAddr = "127.0.0.1:707".parse().unwrap();
        assert!(validate_peer_address(&addr, true).is_ok());
    }

    #[test]
    fn private_address_rejected_in_prod() {
        for ip in ["192.168.1.1:707", "10.0.0.1:707", "172.16.0.1:707"] {
            let addr: SocketAddr = ip.parse().unwrap();
            assert!(
                validate_peer_address(&addr, false).is_err(),
                "{ip} should be rejected"
            );
        }
    }

    #[test]
    fn public_address_accepted() {
        let addr: SocketAddr = "8.8.8.8:707".parse().unwrap();
        assert!(validate_peer_address(&addr, false).is_ok());
    }

    #[test]
    fn zero_port_rejected() {
        let addr: SocketAddr = "8.8.8.8:0".parse().unwrap();
        assert!(matches!(
            validate_peer_address(&addr, true),
            Err(ValidationError::InvalidPort(0))
        ));
    }

    // ---- port validation ----

    #[test]
    fn port_zero_rejected() {
        assert!(matches!(
            validate_port(0),
            Err(ValidationError::InvalidPort(0))
        ));
    }

    #[test]
    fn port_over_max_rejected() {
        assert!(matches!(
            validate_port(65536),
            Err(ValidationError::InvalidPort(65536))
        ));
    }

    #[test]
    fn port_valid_extremes() {
        assert!(validate_port(1).is_ok());
        assert!(validate_port(707).is_ok());
        assert!(validate_port(65535).is_ok());
    }

    // ---- path traversal ----

    #[test]
    fn path_traversal_detected() {
        assert!(validate_file_path("../etc/passwd").is_err());
        assert!(validate_file_path("data/../../secret").is_err());
        assert!(validate_file_path("..").is_err());
        assert!(validate_file_path("foo/..").is_err());
    }

    #[test]
    fn valid_paths_accepted() {
        assert!(validate_file_path("data/chain.db").is_ok());
        assert!(validate_file_path("./data/chain.db").is_ok());
        assert!(validate_file_path("chain.db").is_ok());
    }

    #[test]
    fn path_null_byte_rejected() {
        assert!(matches!(
            validate_file_path("data/file\0.db"),
            Err(ValidationError::PathContainsNull(_))
        ));
    }

    // ---- log sanitization ----

    #[test]
    fn sanitize_log_newlines() {
        let s = sanitize_for_log("user\ninjected\nlines");
        assert!(!s.contains('\n'));
        assert_eq!(s, "user injected lines");
    }

    #[test]
    fn sanitize_log_control_chars() {
        let s = sanitize_for_log("ok\x01\x02\x7ftext");
        // control chars replaced with '?'
        assert!(!s.chars().any(|c| c.is_control()));
    }

    #[test]
    fn sanitize_html_xss() {
        let s = sanitize_for_html("<script>alert('xss')</script>");
        assert!(!s.contains('<'));
        assert!(!s.contains('>'));
        assert!(s.contains("&lt;script&gt;"));
    }
}
