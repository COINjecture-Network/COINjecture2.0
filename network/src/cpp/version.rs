// =============================================================================
// COINjecture P2P Protocol (CPP) - Protocol Versioning & Migration
// =============================================================================
//
// PROTOCOL VERSION HISTORY:
//   V1 (initial): Basic block/tx propagation, handshake, sync, light-client mining
//   V2 (current): Version negotiation, feature flags, connection nonces,
//                 flock/murmuration state in status messages, deprecation tracking
//
// BACKWARD COMPATIBILITY POLICY:
//   - Every node MUST support the current version (V2) and one prior version (V1).
//   - A V2 node MUST accept and decode V1 messages from peers that have not upgraded.
//   - Nodes with incompatible versions (below MIN_SUPPORTED_VERSION) receive a
//     Disconnect message citing version mismatch before the stream is closed.
//   - Feature flags govern soft-optional capabilities; peers that omit them are
//     treated as if the feature is off.
//
// UPGRADE PROCEDURE (see docs/PROTOCOL_UPGRADE_PROCEDURE.md):
//   1. Increment CURRENT_PROTOCOL_VERSION.
//   2. Add new feature flag(s) to FeatureFlags.
//   3. Implement version-specific handler dispatch in VersionDispatch.
//   4. Record changes in docs/PROTOCOL_CHANGELOG.md.
//   5. Announce MIN_SUPPORTED_VERSION bump at least one major release in advance.

use serde::{Deserialize, Serialize};

// ─── Version Constants ────────────────────────────────────────────────────────

/// The protocol version this build speaks natively.
pub const CURRENT_PROTOCOL_VERSION: u8 = 2;

/// Oldest protocol version we still accept connections from.
/// Nodes below this are rejected with DisconnectReason::VersionTooOld.
pub const MIN_SUPPORTED_VERSION: u8 = 1;

// ─── Protocol Version Enum ───────────────────────────────────────────────────

/// Enumerated protocol versions for pattern-matching in handlers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u8)]
pub enum ProtocolVersion {
    /// V1 — original protocol.
    /// Features: Hello/HelloAck handshake, block/tx propagation, sync, light-client mining.
    /// Limitations: no version negotiation, no flock state, no connection nonces.
    V1 = 1,

    /// V2 — current protocol.
    /// Added: version negotiation in handshake, connection nonces for tie-breaking,
    /// flock/murmuration state in StatusMessage, feature flag exchange,
    /// deprecation warnings for V1-only peers.
    V2 = 2,
}

impl ProtocolVersion {
    /// Parse from raw byte (as found in the message header).
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            1 => Some(ProtocolVersion::V1),
            2 => Some(ProtocolVersion::V2),
            _ => None,
        }
    }

    /// Whether this version is within the supported range.
    pub fn is_supported(v: u8) -> bool {
        (MIN_SUPPORTED_VERSION..=CURRENT_PROTOCOL_VERSION).contains(&v)
    }

    /// Returns the feature flags enabled for this version.
    pub fn features(self) -> FeatureFlags {
        FeatureFlags::for_version(self as u8)
    }

    /// True if this version is deprecated (older than current but still supported).
    pub fn is_deprecated(self) -> bool {
        (self as u8) < CURRENT_PROTOCOL_VERSION
    }
}

// ─── Version Negotiation ─────────────────────────────────────────────────────

/// Outcome of version negotiation between two peers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NegotiatedVersion {
    /// The agreed-upon version (highest common version).
    pub version: u8,
    /// Feature flags enabled at the negotiated version.
    pub features: FeatureFlags,
    /// True when the remote peer is running an older protocol version.
    pub remote_is_legacy: bool,
}

impl NegotiatedVersion {
    /// Negotiate from local and remote advertised versions.
    ///
    /// Returns `None` when the remote version is below `MIN_SUPPORTED_VERSION`.
    pub fn negotiate(local: u8, remote: u8) -> Option<Self> {
        if !ProtocolVersion::is_supported(remote) {
            return None;
        }
        let version = local.min(remote);
        Some(NegotiatedVersion {
            version,
            features: FeatureFlags::for_version(version),
            remote_is_legacy: remote < local,
        })
    }

    /// Deprecation warning message for logging/metrics when `remote_is_legacy`.
    pub fn deprecation_warning(&self) -> Option<String> {
        if self.remote_is_legacy {
            Some(format!(
                "DEPRECATION: peer is running protocol v{}, current is v{}. \
                 Support for v{} will be dropped in the next major release. \
                 Peer should upgrade to v{}.",
                self.version,
                CURRENT_PROTOCOL_VERSION,
                MIN_SUPPORTED_VERSION,
                CURRENT_PROTOCOL_VERSION,
            ))
        } else {
            None
        }
    }
}

// ─── Feature Flags ───────────────────────────────────────────────────────────

/// Soft capabilities negotiated during or inferred from the handshake version.
///
/// Feature flags are NOT transmitted as a bitmask over the wire; they are
/// derived deterministically from the negotiated protocol version.  This keeps
/// the handshake simple while still giving code a clean way to guard new paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeatureFlags {
    /// Connection nonce for simultaneous-connection tie-breaking (V2+).
    pub connection_nonces: bool,

    /// Flock/murmuration state included in StatusMessage (V2+).
    pub flock_state_in_status: bool,

    /// Proper version negotiation — both sides exchange versions in Hello (V2+).
    pub version_negotiation: bool,

    /// Deprecation warnings emitted for legacy peers (V2+).
    pub deprecation_warnings: bool,

    /// Block-level flock phase staggering for broadcast delays (V2+).
    pub murmuration_routing: bool,
}

impl FeatureFlags {
    /// Derive feature flags from a raw version byte.
    pub fn for_version(v: u8) -> Self {
        match v {
            0 | 1 => FeatureFlags {
                connection_nonces: false,
                flock_state_in_status: false,
                version_negotiation: false,
                deprecation_warnings: false,
                murmuration_routing: false,
            },
            _ /* 2+ */ => FeatureFlags {
                connection_nonces: true,
                flock_state_in_status: true,
                version_negotiation: true,
                deprecation_warnings: true,
                murmuration_routing: true,
            },
        }
    }

    /// All flags enabled (convenience for tests).
    #[cfg(test)]
    pub fn all_enabled() -> Self {
        FeatureFlags {
            connection_nonces: true,
            flock_state_in_status: true,
            version_negotiation: true,
            deprecation_warnings: true,
            murmuration_routing: true,
        }
    }
}

// ─── Version-Specific Message Dispatch ───────────────────────────────────────

/// Marker for which parsing path to use when handling an inbound message.
///
/// Callers switch on this enum to invoke the appropriate deserialization path
/// when the wire format or field set differs between versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionDispatch {
    /// Handle as V1 (legacy payload schema, missing optional V2 fields).
    V1Legacy,
    /// Handle as V2 (full current schema).
    V2Current,
}

impl VersionDispatch {
    pub fn for_version(v: u8) -> Self {
        if v <= 1 {
            VersionDispatch::V1Legacy
        } else {
            VersionDispatch::V2Current
        }
    }
}

// ─── Network Partition Prevention ────────────────────────────────────────────

/// Policy determining whether a connection attempt should be allowed.
///
/// During a rolling upgrade, nodes running V1 and V2 MUST still be able to
/// communicate.  `ConnectionPolicy` enforces this:
///
/// - `Allow`: The remote version is within the supported window → proceed.
/// - `AllowWithWarning`: The remote is older but still supported → log warning.
/// - `Reject`: The remote is too old (below MIN_SUPPORTED_VERSION) → disconnect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionPolicy {
    Allow,
    AllowWithWarning { remote_version: u8 },
    Reject { remote_version: u8 },
}

impl ConnectionPolicy {
    pub fn evaluate(remote_version: u8) -> Self {
        if remote_version < MIN_SUPPORTED_VERSION {
            ConnectionPolicy::Reject { remote_version }
        } else if remote_version < CURRENT_PROTOCOL_VERSION {
            ConnectionPolicy::AllowWithWarning { remote_version }
        } else {
            ConnectionPolicy::Allow
        }
    }

    pub fn is_allowed(&self) -> bool {
        !matches!(self, ConnectionPolicy::Reject { .. })
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_negotiation_prefers_lower() {
        let neg = NegotiatedVersion::negotiate(2, 1).unwrap();
        assert_eq!(neg.version, 1);
        assert!(neg.remote_is_legacy);
    }

    #[test]
    fn test_version_negotiation_same_version() {
        let neg = NegotiatedVersion::negotiate(2, 2).unwrap();
        assert_eq!(neg.version, 2);
        assert!(!neg.remote_is_legacy);
    }

    #[test]
    fn test_unsupported_version_rejected() {
        assert!(NegotiatedVersion::negotiate(2, 0).is_none());
        assert!(NegotiatedVersion::negotiate(2, 255).is_none());
    }

    #[test]
    fn test_feature_flags_v1() {
        let flags = FeatureFlags::for_version(1);
        assert!(!flags.connection_nonces);
        assert!(!flags.flock_state_in_status);
        assert!(!flags.version_negotiation);
    }

    #[test]
    fn test_feature_flags_v2() {
        let flags = FeatureFlags::for_version(2);
        assert!(flags.connection_nonces);
        assert!(flags.flock_state_in_status);
        assert!(flags.version_negotiation);
        assert!(flags.murmuration_routing);
    }

    #[test]
    fn test_connection_policy() {
        assert_eq!(
            ConnectionPolicy::evaluate(0),
            ConnectionPolicy::Reject { remote_version: 0 }
        );
        assert_eq!(
            ConnectionPolicy::evaluate(1),
            ConnectionPolicy::AllowWithWarning { remote_version: 1 }
        );
        assert_eq!(ConnectionPolicy::evaluate(2), ConnectionPolicy::Allow);
    }

    #[test]
    fn test_deprecation_warning() {
        let neg = NegotiatedVersion::negotiate(2, 1).unwrap();
        assert!(neg.deprecation_warning().is_some());

        let neg2 = NegotiatedVersion::negotiate(2, 2).unwrap();
        assert!(neg2.deprecation_warning().is_none());
    }

    #[test]
    fn test_version_is_supported() {
        assert!(!ProtocolVersion::is_supported(0));
        assert!(ProtocolVersion::is_supported(1));
        assert!(ProtocolVersion::is_supported(2));
        assert!(!ProtocolVersion::is_supported(3));
    }

    #[test]
    fn test_dispatch_routing() {
        assert_eq!(VersionDispatch::for_version(1), VersionDispatch::V1Legacy);
        assert_eq!(VersionDispatch::for_version(2), VersionDispatch::V2Current);
    }
}
