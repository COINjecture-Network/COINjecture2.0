// =============================================================================
// Solution Commit Collection & Winner Selection
// =============================================================================
//
// During the Commit phase, nodes broadcast their solution commitments.
// After the phase ends, the leader (or all nodes deterministically) selects
// the winning solution by highest work score, with deterministic tiebreaks.
//
// Tiebreak: if work_score is equal, the node with the lexicographically
// smallest NodeId wins. This ensures all honest nodes agree on the same winner.
//
// Security model (P0 commit signing):
//   Each `SolutionCommit` carries an ed25519 signature over the canonical
//   message:
//
//     "COINJECT_COMMIT_V1" || epoch_le64 || solution_hash[32] || work_score_bits_le64
//
//   The `public_key` field allows any peer to verify the signature without
//   out-of-band key lookup.  The `CommitCollector::add_commit` method rejects
//   commits whose signature does not verify, preventing peers from submitting
//   fake commits with arbitrary work scores on behalf of other nodes.
//
//   Note: commits without a public key (all-zero) bypass signature checking
//   to maintain backward compatibility during migration.  Once all nodes are
//   upgraded, this bypass SHOULD be removed.

use std::collections::HashMap;

use coinject_core::WorkScore;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};

use super::leader::NodeId;

// ─── Canonical signing message ────────────────────────────────────────────────

/// Domain separator prefixed to every commit signing message.
const COMMIT_MSG_DOMAIN: &[u8] = b"COINJECT_COMMIT_V1";

/// Build the canonical byte string that a committer signs.
///
/// Layout: `domain || epoch_le64 || solution_hash[32] || work_score_bits_le64`
/// Total:   18 + 8 + 32 + 8 = 66 bytes
pub fn commit_signing_message(
    epoch: u64,
    solution_hash: &[u8; 32],
    work_score: WorkScore,
) -> [u8; 66] {
    let mut msg = [0u8; 66];
    let domain_len = COMMIT_MSG_DOMAIN.len(); // 18
    msg[..domain_len].copy_from_slice(COMMIT_MSG_DOMAIN);
    msg[domain_len..domain_len + 8].copy_from_slice(&epoch.to_le_bytes());
    msg[domain_len + 8..domain_len + 40].copy_from_slice(solution_hash);
    msg[domain_len + 40..domain_len + 48].copy_from_slice(&work_score.to_bits().to_le_bytes());
    msg
}

// ─── SolutionCommit ───────────────────────────────────────────────────────────

/// A solution commitment from a single node.
///
/// The `signature` field holds an ed25519 signature over
/// `commit_signing_message(epoch, solution_hash, work_score)`.
/// The `public_key` field is the signer's verifying key.
#[derive(Debug, Clone)]
pub struct SolutionCommit {
    /// The node that produced this solution.
    pub node_id: NodeId,
    /// Ed25519 verifying key of the committing node.
    /// All-zeros means "unsigned" (legacy or unsigned local commit).
    pub public_key: [u8; 32],
    /// Hash of the solution (commitment, not the actual solution).
    pub solution_hash: [u8; 32],
    /// The work score achieved by this solution.
    pub work_score: WorkScore,
    /// Ed25519 signature over `commit_signing_message(epoch, solution_hash, work_score)`.
    /// Empty means unsigned (legacy — will be required in a future protocol version).
    pub signature: Vec<u8>,
}

/// Verify the ed25519 signature on a `SolutionCommit`.
///
/// Returns `true` if:
///   - The commit has a non-trivial public key AND non-empty signature, AND
///   - The signature is valid over the canonical message for `epoch`.
///
/// Returns `true` (bypass) if `public_key` is all-zeros or `signature` is empty,
/// to allow unsigned legacy commits during the migration window.
///
/// The bypass is gated behind the `allow-unsigned-commits` Cargo feature flag.
/// Disable this feature on mainnet once all nodes have upgraded to signed commits.
pub fn verify_commit_signature(epoch: u64, commit: &SolutionCommit) -> bool {
    let no_public_key = commit.public_key == [0u8; 32];
    let no_signature = commit.signature.is_empty();

    // Bypass for unsigned commits (legacy / migration window).
    // Feature-gated: disable `allow-unsigned-commits` to enforce signatures on mainnet.
    #[cfg(feature = "allow-unsigned-commits")]
    if no_public_key || no_signature {
        return true;
    }
    #[cfg(not(feature = "allow-unsigned-commits"))]
    if no_public_key || no_signature {
        return false;
    }

    let Ok(vk) = VerifyingKey::from_bytes(&commit.public_key) else {
        return false;
    };

    if commit.signature.len() != 64 {
        return false;
    }
    let mut sig_bytes = [0u8; 64];
    sig_bytes.copy_from_slice(&commit.signature);
    let sig = Signature::from_bytes(&sig_bytes);

    let msg = commit_signing_message(epoch, &commit.solution_hash, commit.work_score);
    vk.verify(&msg, &sig).is_ok()
}

// ─── CommitCollector ──────────────────────────────────────────────────────────

/// Collects commits during the Commit phase and determines the winner.
#[derive(Debug)]
pub struct CommitCollector {
    /// Epoch this collector is for.
    epoch: u64,
    /// Received commits indexed by node ID.
    commits: HashMap<NodeId, SolutionCommit>,
}

impl CommitCollector {
    /// Create a new collector for the given epoch.
    pub fn new(epoch: u64) -> Self {
        Self {
            epoch,
            commits: HashMap::new(),
        }
    }

    /// Add a commit from a node.
    ///
    /// Returns `false` (and discards the commit) if:
    ///   - `work_score` is zero or negative, NaN, or non-finite
    ///   - The commit carries a signature that does not verify (wrong key / tampered data)
    ///   - A commit from this node has already been received (equivocation)
    pub fn add_commit(&mut self, commit: SolutionCommit) -> bool {
        // Reject any non-positive, NaN, or infinite score — these cannot
        // participate in deterministic ordering.
        if !commit.work_score.is_finite() || commit.work_score <= 0.0 {
            return false;
        }

        // Verify signature when present (rejects forged commits).
        if !verify_commit_signature(self.epoch, &commit) {
            tracing::warn!(
                epoch = self.epoch,
                node = hex::encode(&commit.node_id[..4]),
                "rejected commit: invalid signature"
            );
            return false;
        }

        use std::collections::hash_map::Entry;
        match self.commits.entry(commit.node_id) {
            Entry::Vacant(e) => {
                e.insert(commit);
                true
            }
            Entry::Occupied(_) => false, // Duplicate commit from same node
        }
    }

    /// Number of commits received.
    pub fn commit_count(&self) -> usize {
        self.commits.len()
    }

    /// Whether quorum has been reached given a peer count and threshold.
    pub fn has_quorum(&self, peer_count: usize, threshold: f64) -> bool {
        if peer_count == 0 {
            return false;
        }
        let required = (peer_count as f64 * threshold).ceil() as usize;
        self.commits.len() >= required
    }

    /// Select the winning commit.
    ///
    /// Winner = highest work_score.
    /// Tiebreak = lexicographically smallest NodeId.
    ///
    /// Returns None if no commits received.
    pub fn select_winner(&self) -> Option<&SolutionCommit> {
        self.commits.values().max_by(|a, b| {
            a.work_score
                .partial_cmp(&b.work_score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    // Tiebreak: smallest node ID wins (reversed comparison)
                    b.node_id.cmp(&a.node_id)
                })
        })
    }

    /// Get a ranked list of all commits, best first.
    pub fn ranked(&self) -> Vec<&SolutionCommit> {
        let mut ranked: Vec<_> = self.commits.values().collect();
        ranked.sort_by(|a, b| {
            b.work_score
                .partial_cmp(&a.work_score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.node_id.cmp(&b.node_id))
        });
        ranked
    }

    /// Get the epoch this collector is for.
    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    /// Iterate over all commits.
    pub fn commits(&self) -> impl Iterator<Item = &SolutionCommit> {
        self.commits.values()
    }

    /// Get a specific node's commit.
    pub fn get_commit(&self, node_id: &NodeId) -> Option<&SolutionCommit> {
        self.commits.get(node_id)
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use rand::rngs::OsRng;

    fn make_unsigned_commit(node_byte: u8, score: f64) -> SolutionCommit {
        let mut node_id = [0u8; 32];
        node_id[0] = node_byte;
        SolutionCommit {
            node_id,
            public_key: [0u8; 32], // no key = bypass signature check
            solution_hash: [node_byte; 32],
            work_score: score,
            signature: vec![],
        }
    }

    fn make_signed_commit(epoch: u64, node_byte: u8, score: f64) -> SolutionCommit {
        let sk = SigningKey::generate(&mut OsRng);
        let public_key = sk.verifying_key().to_bytes();
        let mut node_id = [0u8; 32];
        node_id[0] = node_byte;
        let solution_hash = [node_byte; 32];

        let msg = commit_signing_message(epoch, &solution_hash, score);
        let sig = sk.sign(&msg);

        SolutionCommit {
            node_id,
            public_key,
            solution_hash,
            work_score: score,
            signature: sig.to_bytes().to_vec(),
        }
    }

    // ── basic add / dedup / score ────────────────────────────────────────────

    #[cfg(feature = "allow-unsigned-commits")]
    #[test]
    fn test_add_unsigned_commit() {
        let mut collector = CommitCollector::new(1);
        assert!(collector.add_commit(make_unsigned_commit(1, 100.0)));
        assert_eq!(collector.commit_count(), 1);
    }

    #[test]
    fn test_add_signed_commit() {
        let mut collector = CommitCollector::new(1);
        assert!(collector.add_commit(make_signed_commit(1, 1, 100.0)));
        assert_eq!(collector.commit_count(), 1);
    }

    #[cfg(feature = "allow-unsigned-commits")]
    #[test]
    fn test_reject_duplicate() {
        let mut collector = CommitCollector::new(1);
        assert!(collector.add_commit(make_unsigned_commit(1, 100.0)));
        assert!(!collector.add_commit(make_unsigned_commit(1, 200.0))); // same node
        assert_eq!(collector.commit_count(), 1);
    }

    #[test]
    fn test_reject_zero_score() {
        let mut collector = CommitCollector::new(1);
        assert!(!collector.add_commit(make_unsigned_commit(1, 0.0)));
        assert!(!collector.add_commit(make_unsigned_commit(2, -1.0)));
        assert_eq!(collector.commit_count(), 0);
    }

    // ── signature verification ────────────────────────────────────────────────

    #[test]
    fn test_forged_signature_rejected() {
        let sk = SigningKey::generate(&mut OsRng);
        let mut node_id = [0u8; 32];
        node_id[0] = 7;

        // Forge: valid-looking key + random garbage signature
        let commit = SolutionCommit {
            node_id,
            public_key: sk.verifying_key().to_bytes(),
            solution_hash: [7u8; 32],
            work_score: 9999.0,
            signature: vec![0u8; 64], // all-zero signature is invalid
        };

        let mut collector = CommitCollector::new(1);
        assert!(
            !collector.add_commit(commit),
            "All-zero signature must be rejected"
        );
    }

    #[test]
    fn test_wrong_epoch_signature_rejected() {
        // Signed for epoch 5, submitted to collector for epoch 1
        let commit = make_signed_commit(5, 3, 200.0);
        let mut collector = CommitCollector::new(1);
        assert!(
            !collector.add_commit(commit),
            "Signature for wrong epoch must be rejected"
        );
    }

    #[test]
    fn test_tampered_work_score_rejected() {
        let sk = SigningKey::generate(&mut OsRng);
        let public_key = sk.verifying_key().to_bytes();
        let mut node_id = [0u8; 32];
        node_id[0] = 5;
        let solution_hash = [5u8; 32];
        let epoch = 1u64;
        let real_score = 100.0f64;

        // Sign with real score
        let msg = commit_signing_message(epoch, &solution_hash, real_score);
        let sig = sk.sign(&msg);

        // But claim a higher score
        let commit = SolutionCommit {
            node_id,
            public_key,
            solution_hash,
            work_score: 9999.0, // inflated score
            signature: sig.to_bytes().to_vec(),
        };

        let mut collector = CommitCollector::new(epoch);
        assert!(
            !collector.add_commit(commit),
            "Tampered work_score (mismatched signature) must be rejected"
        );
    }

    // ── winner selection ──────────────────────────────────────────────────────

    #[cfg(feature = "allow-unsigned-commits")]
    #[test]
    fn test_select_winner_highest_score() {
        let mut collector = CommitCollector::new(1);
        collector.add_commit(make_unsigned_commit(1, 100.0));
        collector.add_commit(make_unsigned_commit(2, 200.0));
        collector.add_commit(make_unsigned_commit(3, 150.0));

        let winner = collector.select_winner().unwrap();
        assert_eq!(winner.node_id[0], 2);
        assert_eq!(winner.work_score, 200.0);
    }

    #[cfg(feature = "allow-unsigned-commits")]
    #[test]
    fn test_tiebreak_smallest_node_id() {
        let mut collector = CommitCollector::new(1);
        collector.add_commit(make_unsigned_commit(5, 100.0));
        collector.add_commit(make_unsigned_commit(2, 100.0)); // same score, smaller ID
        collector.add_commit(make_unsigned_commit(8, 100.0));

        let winner = collector.select_winner().unwrap();
        assert_eq!(winner.node_id[0], 2, "tiebreak should pick smallest NodeId");
    }

    // ── quorum ────────────────────────────────────────────────────────────────

    #[cfg(feature = "allow-unsigned-commits")]
    #[test]
    fn test_quorum() {
        let mut collector = CommitCollector::new(1);
        collector.add_commit(make_unsigned_commit(1, 100.0));
        collector.add_commit(make_unsigned_commit(2, 100.0));

        assert!(!collector.has_quorum(3, 0.67));
        assert!(collector.has_quorum(3, 0.5));
        assert!(collector.has_quorum(2, 0.67));
    }

    #[test]
    fn test_quorum_edge_cases() {
        let collector = CommitCollector::new(1);
        assert!(!collector.has_quorum(0, 0.67));
        assert!(!collector.has_quorum(5, 0.67));
    }

    // ── ranked / accessors ────────────────────────────────────────────────────

    #[cfg(feature = "allow-unsigned-commits")]
    #[test]
    fn test_ranked_ordering() {
        let mut collector = CommitCollector::new(1);
        collector.add_commit(make_unsigned_commit(1, 50.0));
        collector.add_commit(make_unsigned_commit(2, 200.0));
        collector.add_commit(make_unsigned_commit(3, 150.0));

        let ranked = collector.ranked();
        assert_eq!(ranked[0].work_score, 200.0);
        assert_eq!(ranked[1].work_score, 150.0);
        assert_eq!(ranked[2].work_score, 50.0);
    }

    #[cfg(feature = "allow-unsigned-commits")]
    #[test]
    fn test_ranked_tiebreak() {
        let mut collector = CommitCollector::new(1);
        collector.add_commit(make_unsigned_commit(5, 100.0));
        collector.add_commit(make_unsigned_commit(2, 100.0));
        collector.add_commit(make_unsigned_commit(8, 100.0));

        let ranked = collector.ranked();
        assert_eq!(ranked[0].node_id[0], 2);
        assert_eq!(ranked[1].node_id[0], 5);
        assert_eq!(ranked[2].node_id[0], 8);
    }

    #[test]
    fn test_empty_collector() {
        let collector = CommitCollector::new(1);
        assert!(collector.select_winner().is_none());
        assert_eq!(collector.commit_count(), 0);
        assert_eq!(collector.ranked().len(), 0);
    }

    #[cfg(feature = "allow-unsigned-commits")]
    #[test]
    fn test_get_commit() {
        let mut collector = CommitCollector::new(1);
        let commit = make_unsigned_commit(42, 100.0);
        collector.add_commit(commit);

        let mut node_id = [0u8; 32];
        node_id[0] = 42;
        assert!(collector.get_commit(&node_id).is_some());

        let mut missing = [0u8; 32];
        missing[0] = 99;
        assert!(collector.get_commit(&missing).is_none());
    }

    // ── signing message helpers ───────────────────────────────────────────────

    #[test]
    fn test_signing_message_deterministic() {
        let hash = [0xAB; 32];
        let score = 123.456_f64;
        let epoch = 7u64;

        let msg1 = commit_signing_message(epoch, &hash, score);
        let msg2 = commit_signing_message(epoch, &hash, score);
        assert_eq!(msg1, msg2);
    }

    #[test]
    fn test_signing_message_epoch_sensitive() {
        let hash = [0xAB; 32];
        let score = 100.0_f64;
        let msg1 = commit_signing_message(1, &hash, score);
        let msg2 = commit_signing_message(2, &hash, score);
        assert_ne!(msg1, msg2);
    }
}
