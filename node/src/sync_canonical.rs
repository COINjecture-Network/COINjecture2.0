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
}
