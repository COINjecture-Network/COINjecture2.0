// =============================================================================
// COINjecture P2P Protocol (CPP) - Block Provider Trait
// =============================================================================
// Abstraction for accessing canonical chain storage from the network layer.
// This allows the network to serve blocks to peers during sync.

use coinject_core::Block;

/// Maximum number of blocks to serve in a single GetBlocks response
/// This prevents DoS via large range requests
pub const MAX_BLOCKS_PER_REQUEST: u64 = 500;

/// Block provider trait for accessing canonical chain storage.
///
/// The network layer uses this to:
/// 1. Serve blocks to peers requesting sync (GetBlocks → Blocks)
/// 2. Validate incoming blocks against our chain
/// 3. Determine sync status (are we behind peers?)
///
/// CRITICAL: Implementations MUST return blocks from the canonical (best) chain,
/// not orphaned or stale blocks. The height→block mapping must be authoritative.
pub trait BlockProvider: Send + Sync {
    /// Get a block by height from the canonical chain.
    ///
    /// Returns:
    /// - `Some(block)` if we have the block at this height on the best chain
    /// - `None` if height is beyond our best height or we don't have it
    ///
    /// INVARIANT: If `height <= get_best_height()`, this should return `Some(block)`.
    /// A `None` at a height below best_height indicates chain corruption or a gap.
    fn get_block_by_height(&self, height: u64) -> Option<Block>;

    /// Get the current best (tip) height of the canonical chain.
    ///
    /// This is used to:
    /// 1. Validate GetBlocks range requests
    /// 2. Determine if we're behind peers (need sync)
    /// 3. Update Status messages to peers
    fn get_best_height(&self) -> u64;

    /// Get a range of blocks from the canonical chain.
    ///
    /// Returns blocks in height order from `from_height` to `to_height` (inclusive).
    ///
    /// This is a convenience method with built-in safety limits:
    /// - Caps the range to `MAX_BLOCKS_PER_REQUEST`
    /// - Caps `to_height` to `best_height`
    /// - Returns empty vec if `from_height > to_height` or `from_height > best_height`
    fn get_blocks_range(&self, from_height: u64, to_height: u64) -> Vec<Block> {
        let best = self.get_best_height();

        // Validate range
        if from_height > best || from_height > to_height {
            return Vec::new();
        }

        // Cap to_height at best and limit range
        let to_height = to_height.min(best);
        let count = (to_height - from_height + 1).min(MAX_BLOCKS_PER_REQUEST);
        let actual_to = from_height + count - 1;

        let mut blocks = Vec::with_capacity(count as usize);
        for height in from_height..=actual_to {
            if let Some(block) = self.get_block_by_height(height) {
                blocks.push(block);
            } else {
                // Gap in chain - stop here and return what we have
                // This shouldn't happen on a healthy chain but handles edge cases
                break;
            }
        }

        blocks
    }
}

/// A no-op block provider that returns empty responses.
/// Used as a placeholder during testing or when chain isn't available.
pub struct EmptyBlockProvider;

impl BlockProvider for EmptyBlockProvider {
    fn get_block_by_height(&self, _height: u64) -> Option<Block> {
        None
    }

    fn get_best_height(&self) -> u64 {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_provider() {
        let provider = EmptyBlockProvider;
        assert_eq!(provider.get_best_height(), 0);
        assert!(provider.get_block_by_height(0).is_none());
        assert!(provider.get_blocks_range(0, 10).is_empty());
    }

    #[test]
    fn test_range_validation() {
        let provider = EmptyBlockProvider;

        // Invalid ranges should return empty
        assert!(provider.get_blocks_range(10, 5).is_empty()); // from > to
        assert!(provider.get_blocks_range(100, 200).is_empty()); // from > best
    }
}
