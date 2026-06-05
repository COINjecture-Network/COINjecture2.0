//! Canonical-chain helpers for P2P sync (hash-anchored validation, fork avoidance).
//!
//! See `docs/FORKING_AND_REORG.md` and `docs/github-issues/ISSUE-sync-by-tip-hash-and-cumulative-work.md`.

#[cfg(not(feature = "adzdb"))]
use crate::chain::ChainState;
#[cfg(feature = "adzdb")]
use crate::chain_adzdb::AdzdbChainState as ChainState;
use coinject_core::{Block, Hash};
use std::sync::Arc;

#[cfg(not(feature = "adzdb"))]
type SyncChainError = crate::chain::ChainError;
#[cfg(feature = "adzdb")]
type SyncChainError = crate::chain_adzdb::ChainError;

/// Walk from `(tip_hash, tip_height)` toward genesis; true if `candidate` is on that chain.
fn is_hash_on_chain_from_tip_impl<F>(
    mut get_block_by_hash: F,
    tip_hash: Hash,
    mut tip_height: u64,
    candidate: &Hash,
) -> Result<bool, SyncChainError>
where
    F: FnMut(&Hash) -> Result<Option<Block>, SyncChainError>,
{
    if *candidate == tip_hash {
        return Ok(true);
    }
    let mut hash = tip_hash;
    while tip_height > 0 {
        let Some(block) = get_block_by_hash(&hash)? else {
            return Ok(false);
        };
        hash = block.header.prev_hash;
        tip_height -= 1;
        if hash == *candidate {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Walk from `(tip_hash, tip_height)` toward genesis; true if `candidate` is on that chain.
pub fn is_hash_on_chain_from_tip(
    chain: &ChainState,
    tip_hash: Hash,
    tip_height: u64,
    candidate: &Hash,
) -> Result<bool, SyncChainError> {
    is_hash_on_chain_from_tip_impl(
        |hash| chain.get_block_by_hash(hash),
        tip_hash,
        tip_height,
        candidate,
    )
}

/// Default batch size for hash-anchored sync (matches CPP `MAX_BLOCKS_PER_RESPONSE`).
pub const SYNC_BATCH_BLOCKS: u64 = 16;

/// Planned inclusive height range for a single GetBlocks request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncBatchPlan {
    pub from_height: u64,
    pub to_height: u64,
}

/// Local canonical tip inputs for hash-anchored sync planning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalSyncTip {
    pub height: u64,
    pub hash: Hash,
    pub cumulative_work: u128,
}

/// Peer-advertised tip inputs for hash-anchored sync planning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PeerSyncTip {
    pub height: u64,
    pub hash: Hash,
    pub cumulative_work: u128,
}

/// First height to request when catching up to a peer tip.
///
/// When our tip hash is not on the peer's advertised chain and the peer has greater cumulative
/// work, include the current tip height so we can replace a divergent block at the fork point
/// (production incident: stuck at orphan h=746 while canonical peer at h=2750).
pub fn sync_from_height_for_heavier_peer(
    chain: &ChainState,
    local: LocalSyncTip,
    peer: PeerSyncTip,
    suspect_fork: bool,
) -> Result<u64, SyncChainError> {
    if peer.height <= local.height && !suspect_fork {
        return Ok(local.height.saturating_add(1));
    }

    // Always verify branch membership when behind a taller peer — do not require
    // cumulative_work > 0 (Status can arrive before work is advertised).
    if peer.height > local.height {
        let on_peer_branch = is_hash_on_chain_from_tip(chain, peer.hash, peer.height, &local.hash)?;
        if !on_peer_branch {
            return Ok(local.height);
        }
    }

    Ok(local.height.saturating_add(1))
}

/// Plan an inclusive `[from, to]` sync window toward the peer tip.
pub async fn plan_sync_batch(
    chain: &ChainState,
    local_height: u64,
    local_hash: Hash,
    peer: PeerSyncTip,
    suspect_fork: bool,
    max_batch: u64,
) -> Result<SyncBatchPlan, SyncChainError> {
    let local_work = chain.best_cumulative_work().await;
    let local = LocalSyncTip {
        height: local_height,
        hash: local_hash,
        cumulative_work: local_work,
    };
    let from = sync_from_height_for_heavier_peer(chain, local, peer, suspect_fork)?;
    let batch = max_batch.max(1);
    let to = if peer.height >= from {
        peer.height.min(from.saturating_add(batch - 1))
    } else {
        from
    };
    Ok(SyncBatchPlan {
        from_height: from,
        to_height: to.max(from),
    })
}

/// Whether to apply a sequential sync extension that matches our local tip parent.
///
/// When the peer advertises a heavier tip ahead of this block, require the new block's hash to
/// lie on the peer's advertised chain (hash-anchored sync). Otherwise buffer for reorg.
pub async fn should_apply_sync_extension(
    chain: &Arc<ChainState>,
    block_hash: &Hash,
    block_height: u64,
    peer_tip_height: u64,
    peer_tip_hash: Hash,
    peer_cumulative_work: u128,
) -> Result<bool, SyncChainError> {
    if peer_tip_height <= block_height || peer_cumulative_work == 0 {
        return Ok(true);
    }

    let local_work = chain.best_cumulative_work().await;
    if peer_cumulative_work <= local_work {
        return Ok(true);
    }

    is_hash_on_chain_from_tip(chain.as_ref(), peer_tip_hash, peer_tip_height, block_hash)
}

/// When the next buffered block parents off a hash that is not our tip, return that parent hash.
///
/// The winning fork-point header at `tip_height` must be fetched and buffered before reorg can
/// walk the alternate branch (production: orphan tip h=746 while buffered h=747+ parent canonical h=746).
pub fn missing_fork_point_parent_hash(
    _tip_height: u64,
    tip_hash: Hash,
    next_buffered: Option<&Block>,
    fork_point_buffered: Option<&Block>,
) -> Option<Hash> {
    let next = next_buffered?;
    let parent = next.header.prev_hash;
    if parent == tip_hash {
        return None;
    }
    if fork_point_buffered.is_some_and(|block| block.header.hash() == parent) {
        return None;
    }
    Some(parent)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genesis::{create_genesis_block, GenesisConfig};

    #[tokio::test]
    async fn ancestor_walk_finds_genesis() {
        let dir = std::env::temp_dir().join("coinject-sync-canonical-ancestor");
        let _ = std::fs::remove_dir_all(&dir);
        let genesis = create_genesis_block(GenesisConfig::default());
        #[cfg(not(feature = "adzdb"))]
        let chain = ChainState::new(&dir, &genesis, 64).unwrap();
        #[cfg(feature = "adzdb")]
        let chain = ChainState::new(&dir, &genesis).unwrap();
        let tip = chain.best_block_hash().await;
        assert!(is_hash_on_chain_from_tip(&chain, tip, 0, &genesis.header.hash()).unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn taller_peer_off_branch_syncs_from_local_tip_without_cumulative_work() {
        let dir = std::env::temp_dir().join("coinject-sync-canonical-fork-no-work");
        let _ = std::fs::remove_dir_all(&dir);
        let genesis = create_genesis_block(GenesisConfig::default());
        #[cfg(not(feature = "adzdb"))]
        let chain = ChainState::new(&dir, &genesis, 64).unwrap();
        #[cfg(feature = "adzdb")]
        let chain = ChainState::new(&dir, &genesis).unwrap();

        let local_hash = genesis.header.hash();
        let peer_tip = Hash::from_bytes([7u8; 32]);
        let from = sync_from_height_for_heavier_peer(
            &chain,
            LocalSyncTip {
                height: 746,
                hash: local_hash,
                cumulative_work: 100,
            },
            PeerSyncTip {
                height: 2750,
                hash: peer_tip,
                cumulative_work: 0,
            },
            false,
        )
        .unwrap();
        assert_eq!(
            from, 746,
            "must re-fetch fork height when off branch even if peer work is 0"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn heavier_peer_restarts_sync_at_local_tip_when_off_branch() {
        let dir = std::env::temp_dir().join("coinject-sync-canonical-fork-from");
        let _ = std::fs::remove_dir_all(&dir);
        let genesis = create_genesis_block(GenesisConfig::default());
        #[cfg(not(feature = "adzdb"))]
        let chain = ChainState::new(&dir, &genesis, 64).unwrap();
        #[cfg(feature = "adzdb")]
        let chain = ChainState::new(&dir, &genesis).unwrap();

        let local_hash = genesis.header.hash();
        let peer_tip = Hash::from_bytes([7u8; 32]);
        let from = sync_from_height_for_heavier_peer(
            &chain,
            LocalSyncTip {
                height: 746,
                hash: local_hash,
                cumulative_work: 100,
            },
            PeerSyncTip {
                height: 2750,
                hash: peer_tip,
                cumulative_work: 10_000,
            },
            false,
        )
        .unwrap();
        assert_eq!(from, 746, "must re-fetch fork height when off peer branch");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn on_branch_sync_starts_after_local_tip() {
        let dir = std::env::temp_dir().join("coinject-sync-canonical-fork-after");
        let _ = std::fs::remove_dir_all(&dir);
        let genesis = create_genesis_block(GenesisConfig::default());
        #[cfg(not(feature = "adzdb"))]
        let chain = ChainState::new(&dir, &genesis, 64).unwrap();
        #[cfg(feature = "adzdb")]
        let chain = ChainState::new(&dir, &genesis).unwrap();

        let local_hash = genesis.header.hash();
        let from = sync_from_height_for_heavier_peer(
            &chain,
            LocalSyncTip {
                height: 0,
                hash: local_hash,
                cumulative_work: 0,
            },
            PeerSyncTip {
                height: 100,
                hash: local_hash,
                cumulative_work: 10_000,
            },
            false,
        )
        .unwrap();
        assert_eq!(from, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn heavier_peer_requires_candidate_on_peer_branch() {
        let dir = std::env::temp_dir().join("coinject-sync-canonical-heavy");
        let _ = std::fs::remove_dir_all(&dir);
        let genesis = create_genesis_block(GenesisConfig::default());
        #[cfg(not(feature = "adzdb"))]
        let chain = Arc::new(ChainState::new(&dir, &genesis, 64).unwrap());
        #[cfg(feature = "adzdb")]
        let chain = Arc::new(ChainState::new(&dir, &genesis).unwrap());

        let block_hash = genesis.header.hash();
        let decision = should_apply_sync_extension(
            &chain,
            &block_hash,
            0,
            10,
            Hash::from_bytes([42u8; 32]),
            10_000,
        )
        .await
        .unwrap();
        assert!(
            !decision,
            "off-branch candidate must be rejected for heavier peers"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_fork_point_detects_canonical_parent_gap() {
        use crate::genesis::{create_genesis_block, GenesisConfig};

        let genesis = create_genesis_block(GenesisConfig::default());
        let tip_hash = Hash::from_bytes([1u8; 32]);
        let parent_hash = Hash::from_bytes([2u8; 32]);
        let mut header = genesis.header.clone();
        header.height = 747;
        header.prev_hash = parent_hash;
        let next = Block {
            header,
            coinbase: genesis.coinbase.clone(),
            transactions: vec![],
            solution_reveal: genesis.solution_reveal.clone(),
        };
        assert_eq!(
            missing_fork_point_parent_hash(746, tip_hash, Some(&next), None),
            Some(parent_hash)
        );
    }
}
