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

use std::collections::HashMap;

use coinject_core::WorkScore;

use super::leader::NodeId;

/// A solution commitment from a single node.
#[derive(Debug, Clone)]
pub struct SolutionCommit {
    /// The node that produced this solution.
    pub node_id: NodeId,
    /// Hash of the solution (commitment, not the actual solution).
    pub solution_hash: [u8; 32],
    /// The work score achieved by this solution.
    pub work_score: WorkScore,
    /// Ed25519 signature over (epoch || solution_hash || work_score).
    pub signature: Vec<u8>,
}

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

    /// Add a commit from a node. Returns false if a commit from this node
    /// was already received (duplicate/equivocation) or the score is invalid.
    ///
    /// Rejects:
    /// - Duplicate commits (same node_id).
    /// - Zero, negative, or NaN work scores.
    /// - Non-finite work scores (infinity).
    pub fn add_commit(&mut self, commit: SolutionCommit) -> bool {
        // Reject any non-positive, NaN, or infinite score — these cannot
        // participate in deterministic ordering.
        if !commit.work_score.is_finite() || commit.work_score <= 0.0 {
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

    /// Get all commits as a slice-like iterator.
    pub fn commits(&self) -> impl Iterator<Item = &SolutionCommit> {
        self.commits.values()
    }

    /// Get a specific node's commit.
    pub fn get_commit(&self, node_id: &NodeId) -> Option<&SolutionCommit> {
        self.commits.get(node_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_commit(node_byte: u8, score: f64) -> SolutionCommit {
        let mut node_id = [0u8; 32];
        node_id[0] = node_byte;
        SolutionCommit {
            node_id,
            solution_hash: [node_byte; 32],
            work_score: score,
            signature: vec![0; 64],
        }
    }

    #[test]
    fn test_add_commit() {
        let mut collector = CommitCollector::new(1);
        assert!(collector.add_commit(make_commit(1, 100.0)));
        assert_eq!(collector.commit_count(), 1);
    }

    #[test]
    fn test_reject_duplicate() {
        let mut collector = CommitCollector::new(1);
        assert!(collector.add_commit(make_commit(1, 100.0)));
        assert!(!collector.add_commit(make_commit(1, 200.0))); // Same node
        assert_eq!(collector.commit_count(), 1);
    }

    #[test]
    fn test_reject_zero_score() {
        let mut collector = CommitCollector::new(1);
        assert!(!collector.add_commit(make_commit(1, 0.0)));
        assert!(!collector.add_commit(make_commit(2, -1.0)));
        assert_eq!(collector.commit_count(), 0);
    }

    #[test]
    fn test_select_winner_highest_score() {
        let mut collector = CommitCollector::new(1);
        collector.add_commit(make_commit(1, 100.0));
        collector.add_commit(make_commit(2, 200.0));
        collector.add_commit(make_commit(3, 150.0));

        let winner = collector.select_winner().unwrap();
        assert_eq!(winner.node_id[0], 2);
        assert_eq!(winner.work_score, 200.0);
    }

    #[test]
    fn test_tiebreak_smallest_node_id() {
        let mut collector = CommitCollector::new(1);
        collector.add_commit(make_commit(5, 100.0));
        collector.add_commit(make_commit(2, 100.0)); // Same score, smaller ID
        collector.add_commit(make_commit(8, 100.0));

        let winner = collector.select_winner().unwrap();
        assert_eq!(winner.node_id[0], 2, "tiebreak should pick smallest NodeId");
    }

    #[test]
    fn test_quorum() {
        let mut collector = CommitCollector::new(1);
        collector.add_commit(make_commit(1, 100.0));
        collector.add_commit(make_commit(2, 100.0));

        // 2 out of 3 peers = 66.7%, ceil(3*0.67) = 3 → not met
        assert!(!collector.has_quorum(3, 0.67));

        // 2 out of 3 peers, threshold 0.5 → ceil(3*0.5)=2 → met
        assert!(collector.has_quorum(3, 0.5));

        // 2 out of 2 peers = 100%, threshold 0.67 → met
        assert!(collector.has_quorum(2, 0.67));
    }

    #[test]
    fn test_quorum_edge_cases() {
        let collector = CommitCollector::new(1);
        assert!(!collector.has_quorum(0, 0.67)); // No peers
        assert!(!collector.has_quorum(5, 0.67)); // No commits
    }

    #[test]
    fn test_ranked_ordering() {
        let mut collector = CommitCollector::new(1);
        collector.add_commit(make_commit(1, 50.0));
        collector.add_commit(make_commit(2, 200.0));
        collector.add_commit(make_commit(3, 150.0));

        let ranked = collector.ranked();
        assert_eq!(ranked[0].work_score, 200.0);
        assert_eq!(ranked[1].work_score, 150.0);
        assert_eq!(ranked[2].work_score, 50.0);
    }

    #[test]
    fn test_ranked_tiebreak() {
        let mut collector = CommitCollector::new(1);
        collector.add_commit(make_commit(5, 100.0));
        collector.add_commit(make_commit(2, 100.0));
        collector.add_commit(make_commit(8, 100.0));

        let ranked = collector.ranked();
        // Tied scores sorted by ascending NodeId
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

    #[test]
    fn test_get_commit() {
        let mut collector = CommitCollector::new(1);
        let commit = make_commit(42, 100.0);
        collector.add_commit(commit.clone());

        let mut node_id = [0u8; 32];
        node_id[0] = 42;
        assert!(collector.get_commit(&node_id).is_some());

        let mut missing = [0u8; 32];
        missing[0] = 99;
        assert!(collector.get_commit(&missing).is_none());
    }
}
