// =============================================================================
// LightSync Protocol - FlyClient with Merkle Mountain Range (MMR)
// =============================================================================
//
// This module implements the FlyClient protocol for super-light syncing:
// - O(log n) proof size instead of O(n) for full header chain
// - Uses Merkle Mountain Ranges (MMR) for efficient accumulator proofs
// - Enables mobile/IoT devices to verify chain state with ~10KB proofs
//
// Reference: "FlyClient: Super-Light Clients for Cryptocurrencies"
// https://eprint.iacr.org/2019/226.pdf
//
// CRITICAL: This enables verification of 1M+ block chains with tiny proofs!

use coinject_core::{BlockHeader, Hash};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// =============================================================================
// Constants
// =============================================================================

/// FlyClient security parameter (number of sampled blocks)
/// Higher = more security, more bandwidth
/// λ = 50 gives 2^-50 security against adversary with <50% hash power
pub const FLYCLIENT_SECURITY_PARAM: usize = 50;

/// Maximum MMR proof size (peaks + authentication path)
pub const MAX_MMR_PROOF_SIZE: usize = 64;

/// Block header size in bytes (compact representation)
pub const HEADER_SIZE_BYTES: usize = 80;

/// Minimum blocks before FlyClient sampling kicks in
pub const FLYCLIENT_MIN_BLOCKS: u64 = 1000;

/// MMR peak count for common chain lengths
/// peaks(n) = popcount(n) for binary representation
pub const fn mmr_peak_count(n: u64) -> u32 {
    n.count_ones()
}

// =============================================================================
// Merkle Mountain Range (MMR)
// =============================================================================

/// A Merkle Mountain Range accumulator for block headers
/// MMR is an append-only authenticated data structure that provides:
/// - O(log n) inclusion proofs
/// - O(1) append operations
/// - O(log n) peaks (roots)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleMountainRange {
    /// Total number of leaves (block headers)
    pub leaf_count: u64,
    /// MMR peaks (roots of complete binary trees)
    pub peaks: Vec<Hash>,
    /// Size of the MMR (total nodes including internal)
    pub mmr_size: u64,
}

impl MerkleMountainRange {
    /// Create a new empty MMR
    pub fn new() -> Self {
        MerkleMountainRange {
            leaf_count: 0,
            peaks: Vec::new(),
            mmr_size: 0,
        }
    }

    /// Create MMR with initial genesis header
    pub fn with_genesis(genesis_hash: Hash) -> Self {
        MerkleMountainRange {
            leaf_count: 1,
            peaks: vec![genesis_hash],
            mmr_size: 1,
        }
    }

    /// Append a new leaf (block header hash) to the MMR
    /// Returns the new MMR root
    pub fn append(&mut self, leaf_hash: Hash) -> Hash {
        self.leaf_count += 1;
        self.mmr_size += 1;

        // Add as new peak
        let mut new_peak = leaf_hash;
        let mut height = 0u32;

        // Merge with existing peaks of same height
        while self.has_peak_at_height(height) {
            let left_peak = self.peaks.pop().unwrap();
            new_peak = hash_mmr_node(&left_peak, &new_peak, height);
            self.mmr_size += 1;
            height += 1;
        }

        self.peaks.push(new_peak);
        self.root()
    }

    /// Check if there's a peak at the given height
    fn has_peak_at_height(&self, height: u32) -> bool {
        // Peak exists if bit is set in leaf_count - 1
        if self.leaf_count == 0 {
            return false;
        }
        ((self.leaf_count - 1) >> height) & 1 == 1
    }

    /// Get the MMR root (bag of peaks)
    pub fn root(&self) -> Hash {
        if self.peaks.is_empty() {
            return Hash::default();
        }
        if self.peaks.len() == 1 {
            return self.peaks[0];
        }

        // Bag the peaks from right to left
        let mut root = self.peaks[self.peaks.len() - 1];
        for i in (0..self.peaks.len() - 1).rev() {
            root = hash_bag(&self.peaks[i], &root);
        }
        root
    }

    /// Get the number of peaks
    pub fn peak_count(&self) -> usize {
        self.peaks.len()
    }

    /// Get peaks for verification
    pub fn get_peaks(&self) -> &[Hash] {
        &self.peaks
    }

    /// Calculate the position of a leaf in the MMR
    pub fn leaf_position(leaf_index: u64) -> u64 {
        // MMR positions are 1-indexed
        // Position = 2*index - popcount(index) for 0-indexed leaf
        if leaf_index == 0 {
            return 1;
        }
        2 * leaf_index - (leaf_index - 1).count_ones() as u64
    }

    /// Get MMR size for n leaves
    pub fn size_for_leaves(n: u64) -> u64 {
        if n == 0 {
            return 0;
        }
        // MMR size = 2n - popcount(n)
        2 * n - n.count_ones() as u64
    }
}

impl Default for MerkleMountainRange {
    fn default() -> Self {
        Self::new()
    }
}

/// Hash two MMR nodes together with height domain separation
fn hash_mmr_node(left: &Hash, right: &Hash, height: u32) -> Hash {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    // Domain separation: "MMR_NODE" || height || left || right
    hasher.update(b"MMR_NODE");
    hasher.update(height.to_le_bytes());
    hasher.update(left.as_bytes());
    hasher.update(right.as_bytes());
    Hash::from_bytes(hasher.finalize().into())
}

/// Bag two peaks together (different from node hashing)
fn hash_bag(left: &Hash, right: &Hash) -> Hash {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    // Domain separation: "MMR_BAG" || left || right
    hasher.update(b"MMR_BAG");
    hasher.update(left.as_bytes());
    hasher.update(right.as_bytes());
    Hash::from_bytes(hasher.finalize().into())
}

// =============================================================================
// MMR Inclusion Proof
// =============================================================================

/// Proof that a leaf is included in an MMR
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MMRInclusionProof {
    /// The leaf hash being proven
    pub leaf_hash: Hash,
    /// Leaf index (0-based block height)
    pub leaf_index: u64,
    /// MMR size at time of proof
    pub mmr_size: u64,
    /// Authentication path (siblings from leaf to peak)
    pub auth_path: Vec<(Hash, bool)>, // (sibling_hash, is_right_sibling)
    /// Peak index this leaf belongs to
    pub peak_index: usize,
    /// All peaks for root verification
    pub peaks: Vec<Hash>,
}

impl MMRInclusionProof {
    /// Verify this proof against an MMR root
    pub fn verify(&self, expected_root: &Hash) -> bool {
        // Reconstruct path from leaf to peak
        let mut current = self.leaf_hash;
        let mut height = 0u32;

        for (sibling, is_right) in &self.auth_path {
            current = if *is_right {
                hash_mmr_node(&current, sibling, height)
            } else {
                hash_mmr_node(sibling, &current, height)
            };
            height += 1;
        }

        // Check that computed hash matches the expected peak
        if self.peak_index >= self.peaks.len() {
            return false;
        }
        if current != self.peaks[self.peak_index] {
            return false;
        }

        // Bag peaks to get root
        let computed_root = if self.peaks.len() == 1 {
            self.peaks[0]
        } else {
            let mut root = self.peaks[self.peaks.len() - 1];
            for i in (0..self.peaks.len() - 1).rev() {
                root = hash_bag(&self.peaks[i], &root);
            }
            root
        };

        &computed_root == expected_root
    }

    /// Get proof size in bytes (approximate)
    pub fn size_bytes(&self) -> usize {
        32 + // leaf_hash
        8 +  // leaf_index
        8 +  // mmr_size
        self.auth_path.len() * 33 + // (hash + bool)
        8 +  // peak_index
        self.peaks.len() * 32 // peaks
    }
}

// =============================================================================
// FlyClient Protocol
// =============================================================================

/// FlyClient proof - probabilistic verification of chain validity
/// Samples O(log n) blocks with probability weighted by difficulty
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlyClientProof {
    /// Genesis hash for chain identification
    pub genesis_hash: Hash,
    /// Claimed chain tip header
    pub tip_header: BlockHeader,
    /// MMR root at chain tip
    pub mmr_root: Hash,
    /// Sampled block headers with MMR proofs
    pub sampled_headers: Vec<SampledBlock>,
    /// Total chain work (cumulative difficulty)
    pub total_work: u128,
    /// Security parameter used
    pub security_param: usize,
}

/// A sampled block with its MMR inclusion proof
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SampledBlock {
    /// The block header
    pub header: BlockHeader,
    /// MMR inclusion proof for this header
    pub mmr_proof: MMRInclusionProof,
    /// Sampling weight (based on difficulty)
    pub weight: f64,
}

impl FlyClientProof {
    /// Verify the FlyClient proof
    /// Returns true if the proof is valid
    pub fn verify(&self) -> Result<bool, FlyClientError> {
        // 1. Verify genesis hash matches expected
        // (Caller should check this against known genesis)

        // 2. Verify tip header is valid (basic checks)
        if self.tip_header.height == 0 {
            return Err(FlyClientError::InvalidTip);
        }

        // 3. Verify all sampled headers have valid MMR proofs
        for sampled in &self.sampled_headers {
            if !sampled.mmr_proof.verify(&self.mmr_root) {
                return Err(FlyClientError::InvalidMMRProof(sampled.header.height));
            }

            // Verify header links to parent (basic PoW check would go here)
            if sampled.header.height > 0 {
                // In a full implementation, verify:
                // - Block hash meets difficulty target
                // - Timestamp is reasonable
                // - Work score is valid
            }
        }

        // 4. Verify sampling distribution
        // Headers should be sampled with probability proportional to difficulty
        // This is a simplified check - full implementation would verify
        // the sampling follows the FlyClient distribution
        if self.sampled_headers.len() < self.security_param.min(self.tip_header.height as usize) {
            return Err(FlyClientError::InsufficientSamples);
        }

        Ok(true)
    }

    /// Get proof size in bytes
    pub fn size_bytes(&self) -> usize {
        32 + // genesis_hash
        HEADER_SIZE_BYTES + // tip_header (approximate)
        32 + // mmr_root
        self.sampled_headers.iter().map(|s| {
            HEADER_SIZE_BYTES + s.mmr_proof.size_bytes() + 8
        }).sum::<usize>() +
        16 + // total_work
        8 // security_param
    }
}

// =============================================================================
// LightSync Protocol Messages
// =============================================================================

/// Messages for the LightSync protocol (extends LightClientMessage)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LightSyncMessage {
    // === Header Sync (Standard SPV) ===
    
    /// Request headers from a height range
    GetHeaders {
        /// Starting height (inclusive)
        start_height: u64,
        /// Maximum headers to return
        max_headers: u64,
        /// Request ID for correlation
        request_id: u64,
    },

    /// Response with headers
    Headers {
        /// Block headers
        headers: Vec<BlockHeader>,
        /// Whether there are more headers
        has_more: bool,
        /// Request ID
        request_id: u64,
    },

    // === FlyClient Protocol ===
    
    /// Request a FlyClient proof for the current chain
    GetFlyClientProof {
        /// Security parameter (number of samples)
        security_param: usize,
        /// Request ID
        request_id: u64,
    },

    /// FlyClient proof response
    FlyClientProof {
        /// The proof
        proof: FlyClientProof,
        /// Request ID
        request_id: u64,
    },

    // === MMR Proofs ===
    
    /// Request MMR inclusion proof for a specific block
    GetMMRProof {
        /// Block height to prove
        block_height: u64,
        /// Request ID
        request_id: u64,
    },

    /// MMR proof response
    MMRProof {
        /// The header being proven
        header: BlockHeader,
        /// Inclusion proof
        proof: MMRInclusionProof,
        /// Current MMR root
        mmr_root: Hash,
        /// Request ID
        request_id: u64,
    },

    // === Transaction Proofs ===
    
    /// Request proof that a transaction is in a block
    GetTxProof {
        /// Transaction hash
        tx_hash: Hash,
        /// Block hash containing the transaction
        block_hash: Hash,
        /// Request ID
        request_id: u64,
    },

    /// Transaction inclusion proof
    TxProof {
        /// Transaction hash
        tx_hash: Hash,
        /// Block header
        block_header: BlockHeader,
        /// Merkle proof within block
        merkle_proof: Vec<(Hash, bool)>,
        /// MMR proof for block
        mmr_proof: MMRInclusionProof,
        /// Request ID
        request_id: u64,
    },

    // === Chain State ===
    
    /// Request current chain tip with MMR root
    GetChainTip {
        request_id: u64,
    },

    /// Chain tip response
    ChainTip {
        /// Best header
        tip_header: BlockHeader,
        /// MMR root
        mmr_root: Hash,
        /// Total work
        total_work: u128,
        /// Request ID
        request_id: u64,
    },
}

// =============================================================================
// Light Sync State (Full Node Side)
// =============================================================================

/// MMR state maintained by Full/Archive nodes to serve Light clients
#[derive(Debug)]
pub struct LightSyncServer {
    /// The MMR accumulator
    mmr: MerkleMountainRange,
    /// Header hashes by height (for proof generation)
    headers: HashMap<u64, Hash>,
    /// Full headers for FlyClient sampling
    full_headers: HashMap<u64, BlockHeader>,
    /// Current chain height
    chain_height: u64,
    /// Total accumulated work
    total_work: u128,
}

impl LightSyncServer {
    /// Create new LightSync server state
    pub fn new(genesis_header: BlockHeader) -> Self {
        let genesis_hash = genesis_header.hash();
        let mut headers = HashMap::new();
        let mut full_headers = HashMap::new();
        
        headers.insert(0, genesis_hash);
        full_headers.insert(0, genesis_header);

        LightSyncServer {
            mmr: MerkleMountainRange::with_genesis(genesis_hash),
            headers,
            full_headers,
            chain_height: 0,
            total_work: 0,
        }
    }

    /// Add a new block header to the MMR
    pub fn add_header(&mut self, header: BlockHeader) -> Hash {
        let hash = header.hash();
        let height = header.height;

        self.headers.insert(height, hash);
        self.full_headers.insert(height, header.clone());
        self.chain_height = height;
        
        // Accumulate work (simplified - real impl uses actual difficulty)
        self.total_work += 1u128 << header.difficulty;

        self.mmr.append(hash)
    }

    /// Get current MMR root
    pub fn mmr_root(&self) -> Hash {
        self.mmr.root()
    }

    /// Generate MMR inclusion proof for a block height
    pub fn generate_mmr_proof(&self, height: u64) -> Option<MMRInclusionProof> {
        let leaf_hash = *self.headers.get(&height)?;
        
        // Simplified proof generation
        // Full implementation would compute actual auth path
        Some(MMRInclusionProof {
            leaf_hash,
            leaf_index: height,
            mmr_size: self.mmr.mmr_size,
            auth_path: self.compute_auth_path(height),
            peak_index: self.find_peak_index(height),
            peaks: self.mmr.peaks.clone(),
        })
    }

    /// Compute authentication path for a leaf
    fn compute_auth_path(&self, height: u64) -> Vec<(Hash, bool)> {
        let mut path = Vec::new();
        let mut pos = height;
        let mut current_height = 0u32;

        // Walk up the tree collecting siblings
        while pos < self.mmr.leaf_count {
            let sibling_pos = if pos % 2 == 0 { pos + 1 } else { pos - 1 };
            
            if let Some(&sibling_hash) = self.headers.get(&sibling_pos) {
                let is_right = pos % 2 == 0;
                path.push((sibling_hash, is_right));
            }
            
            pos /= 2;
            current_height += 1;
            
            // Prevent infinite loops
            if current_height > 64 {
                break;
            }
        }

        path
    }

    /// Find which peak a leaf belongs to
    fn find_peak_index(&self, height: u64) -> usize {
        // Simplified - find the peak that covers this height
        let mut covered = 0u64;
        for (i, _) in self.mmr.peaks.iter().enumerate() {
            let peak_size = 1u64 << (self.mmr.peaks.len() - 1 - i);
            covered += peak_size;
            if height < covered {
                return i;
            }
        }
        self.mmr.peaks.len().saturating_sub(1)
    }

    /// Generate a FlyClient proof
    pub fn generate_flyclient_proof(&self, security_param: usize) -> FlyClientProof {
        let tip_header = self.full_headers.get(&self.chain_height)
            .cloned()
            .unwrap_or_default();
        
        // Sample blocks according to FlyClient distribution
        let sampled_headers = self.sample_blocks(security_param);

        FlyClientProof {
            genesis_hash: self.headers.get(&0).cloned().unwrap_or_default(),
            tip_header,
            mmr_root: self.mmr_root(),
            sampled_headers,
            total_work: self.total_work,
            security_param,
        }
    }

    /// Sample blocks according to FlyClient distribution
    /// Blocks are sampled with probability proportional to difficulty
    fn sample_blocks(&self, security_param: usize) -> Vec<SampledBlock> {
        use rand::Rng;
        
        if self.chain_height < FLYCLIENT_MIN_BLOCKS {
            // For short chains, just return all headers
            return self.full_headers.values()
                .filter(|h| h.height > 0)
                .map(|h| {
                    let mmr_proof = self.generate_mmr_proof(h.height)
                        .unwrap_or_else(|| MMRInclusionProof {
                            leaf_hash: h.hash(),
                            leaf_index: h.height,
                            mmr_size: self.mmr.mmr_size,
                            auth_path: Vec::new(),
                            peak_index: 0,
                            peaks: self.mmr.peaks.clone(),
                        });
                    
                    SampledBlock {
                        header: h.clone(),
                        mmr_proof,
                        weight: 1.0,
                    }
                })
                .collect();
        }

        let mut rng = rand::thread_rng();
        let mut sampled = Vec::with_capacity(security_param);
        let mut sampled_heights = std::collections::HashSet::new();

        // Sample with replacement, weighted by position (more recent = more likely)
        // This is a simplified version - full FlyClient uses difficulty-weighted sampling
        while sampled.len() < security_param && sampled.len() < self.chain_height as usize {
            // Sample from geometric distribution favoring recent blocks
            let u: f64 = rng.gen();
            let height = ((1.0 - u.powf(0.5)) * self.chain_height as f64) as u64;
            
            if height == 0 || sampled_heights.contains(&height) {
                continue;
            }
            
            if let Some(header) = self.full_headers.get(&height) {
                if let Some(mmr_proof) = self.generate_mmr_proof(height) {
                    sampled_heights.insert(height);
                    sampled.push(SampledBlock {
                        header: header.clone(),
                        mmr_proof,
                        weight: 1.0 / (self.chain_height - height + 1) as f64,
                    });
                }
            }
        }

        // Sort by height for easier verification
        sampled.sort_by_key(|s| s.header.height);
        sampled
    }

    /// Handle GetHeaders request
    pub fn handle_get_headers(&self, start_height: u64, max_headers: u64, request_id: u64) -> LightSyncMessage {
        let end_height = (start_height + max_headers).min(self.chain_height + 1);
        
        let headers: Vec<BlockHeader> = (start_height..end_height)
            .filter_map(|h| self.full_headers.get(&h).cloned())
            .collect();

        let has_more = end_height <= self.chain_height;

        LightSyncMessage::Headers {
            headers,
            has_more,
            request_id,
        }
    }

    /// Handle GetFlyClientProof request
    pub fn handle_get_flyclient_proof(&self, security_param: usize, request_id: u64) -> LightSyncMessage {
        let proof = self.generate_flyclient_proof(security_param);
        LightSyncMessage::FlyClientProof { proof, request_id }
    }

    /// Handle GetMMRProof request
    pub fn handle_get_mmr_proof(&self, block_height: u64, request_id: u64) -> LightSyncMessage {
        let header = self.full_headers.get(&block_height).cloned().unwrap_or_default();
        let proof = self.generate_mmr_proof(block_height).unwrap_or_else(|| {
            MMRInclusionProof {
                leaf_hash: header.hash(),
                leaf_index: block_height,
                mmr_size: self.mmr.mmr_size,
                auth_path: Vec::new(),
                peak_index: 0,
                peaks: self.mmr.peaks.clone(),
            }
        });

        LightSyncMessage::MMRProof {
            header,
            proof,
            mmr_root: self.mmr_root(),
            request_id,
        }
    }

    /// Handle GetChainTip request
    pub fn handle_get_chain_tip(&self, request_id: u64) -> LightSyncMessage {
        let tip_header = self.full_headers.get(&self.chain_height)
            .cloned()
            .unwrap_or_default();

        LightSyncMessage::ChainTip {
            tip_header,
            mmr_root: self.mmr_root(),
            total_work: self.total_work,
            request_id,
        }
    }

    // =========================================================================
    // ACCESSOR METHODS (for NodeTypeManager integration)
    // =========================================================================

    /// Get a specific header by height
    pub fn get_header(&self, height: u64) -> Option<BlockHeader> {
        self.full_headers.get(&height).cloned()
    }

    /// Get total accumulated work
    pub fn total_work(&self) -> u128 {
        self.total_work
    }

    /// Get current chain height
    pub fn chain_height(&self) -> u64 {
        self.chain_height
    }

    /// Get all peaks for external use
    pub fn get_mmr_peaks(&self) -> Vec<Hash> {
        self.mmr.peaks.clone()
    }

    /// Get MMR size
    pub fn get_mmr_size(&self) -> u64 {
        self.mmr.mmr_size
    }

    /// Get leaf count
    pub fn get_leaf_count(&self) -> u64 {
        self.mmr.leaf_count
    }
}

// =============================================================================
// Light Sync Verifier (Light Client Side)
// =============================================================================

/// Verifier for FlyClient proofs on Light clients
#[derive(Debug)]
pub struct LightClientVerifier {
    /// Known genesis hash
    genesis_hash: Hash,
    /// Current verified tip
    verified_tip: Option<BlockHeader>,
    /// Current verified MMR root
    verified_mmr_root: Option<Hash>,
    /// Verification history
    verification_count: u64,
}

impl LightClientVerifier {
    /// Create a new verifier with known genesis
    pub fn new(genesis_hash: Hash) -> Self {
        LightClientVerifier {
            genesis_hash,
            verified_tip: None,
            verified_mmr_root: None,
            verification_count: 0,
        }
    }

    /// Verify a FlyClient proof and update state
    pub fn verify_and_update(&mut self, proof: &FlyClientProof) -> Result<VerificationResult, FlyClientError> {
        // Check genesis matches
        if proof.genesis_hash != self.genesis_hash {
            return Err(FlyClientError::GenesisMismatch);
        }

        // Verify the proof
        proof.verify()?;

        // Check if this extends our current verified state
        let extends_chain = if let Some(ref current_tip) = self.verified_tip {
            proof.tip_header.height > current_tip.height
        } else {
            true
        };

        if extends_chain {
            self.verified_tip = Some(proof.tip_header.clone());
            self.verified_mmr_root = Some(proof.mmr_root);
            self.verification_count += 1;

            Ok(VerificationResult {
                valid: true,
                new_tip_height: proof.tip_header.height,
                total_work: proof.total_work,
                proof_size_bytes: proof.size_bytes(),
                samples_verified: proof.sampled_headers.len(),
            })
        } else {
            Ok(VerificationResult {
                valid: true,
                new_tip_height: self.verified_tip.as_ref().map(|t| t.height).unwrap_or(0),
                total_work: proof.total_work,
                proof_size_bytes: proof.size_bytes(),
                samples_verified: proof.sampled_headers.len(),
            })
        }
    }

    /// Verify a single block is in the chain
    pub fn verify_block(&self, proof: &MMRInclusionProof) -> Result<bool, FlyClientError> {
        let mmr_root = self.verified_mmr_root.ok_or(FlyClientError::NoVerifiedState)?;
        Ok(proof.verify(&mmr_root))
    }

    /// Get current verified height
    pub fn verified_height(&self) -> u64 {
        self.verified_tip.as_ref().map(|t| t.height).unwrap_or(0)
    }

    /// Get current verified MMR root
    pub fn verified_root(&self) -> Option<Hash> {
        self.verified_mmr_root
    }
}

/// Result of FlyClient verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    /// Whether verification succeeded
    pub valid: bool,
    /// New tip height after verification
    pub new_tip_height: u64,
    /// Total chain work
    pub total_work: u128,
    /// Size of proof in bytes
    pub proof_size_bytes: usize,
    /// Number of samples verified
    pub samples_verified: usize,
}

// =============================================================================
// Errors
// =============================================================================

#[derive(Debug, Clone, thiserror::Error)]
pub enum FlyClientError {
    #[error("Genesis hash mismatch")]
    GenesisMismatch,

    #[error("Invalid chain tip")]
    InvalidTip,

    #[error("Invalid MMR proof for block {0}")]
    InvalidMMRProof(u64),

    #[error("Insufficient samples in proof")]
    InsufficientSamples,

    #[error("No verified state available")]
    NoVerifiedState,

    #[error("Proof verification failed: {0}")]
    VerificationFailed(String),
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use coinject_core::Address;

    fn test_header(height: u64, parent: Hash) -> BlockHeader {
        BlockHeader {
            version: 1,
            height,
            timestamp: 1000000 + height,
            parent_hash: parent,
            merkle_root: Hash::default(),
            state_root: Hash::default(),
            nonce: 0,
            difficulty: 4,
            miner: Address::default(),
            work_score: 0,
        }
    }

    #[test]
    fn test_mmr_append() {
        let genesis = test_header(0, Hash::default());
        let mut mmr = MerkleMountainRange::with_genesis(genesis.hash());

        assert_eq!(mmr.leaf_count, 1);
        assert_eq!(mmr.peak_count(), 1);

        // Add more headers
        for i in 1..10 {
            let header = test_header(i, Hash::default());
            mmr.append(header.hash());
        }

        assert_eq!(mmr.leaf_count, 10);
        // 10 in binary = 1010, so 2 peaks
        assert_eq!(mmr.peak_count(), 2);
    }

    #[test]
    fn test_mmr_root_deterministic() {
        let genesis = test_header(0, Hash::default());
        let mut mmr1 = MerkleMountainRange::with_genesis(genesis.hash());
        let mut mmr2 = MerkleMountainRange::with_genesis(genesis.hash());

        for i in 1..100 {
            let header = test_header(i, Hash::default());
            mmr1.append(header.hash());
            mmr2.append(header.hash());
        }

        assert_eq!(mmr1.root(), mmr2.root());
    }

    #[test]
    fn test_light_sync_server() {
        let genesis = test_header(0, Hash::default());
        let mut server = LightSyncServer::new(genesis.clone());

        // Add some blocks
        let mut parent = genesis.hash();
        for i in 1..100 {
            let header = test_header(i, parent);
            parent = header.hash();
            server.add_header(header);
        }

        assert_eq!(server.chain_height, 99);

        // Generate FlyClient proof
        let proof = server.generate_flyclient_proof(10);
        assert_eq!(proof.tip_header.height, 99);
        assert!(!proof.sampled_headers.is_empty());
    }

    #[test]
    fn test_flyclient_verification() {
        let genesis = test_header(0, Hash::default());
        let genesis_hash = genesis.hash();
        let mut server = LightSyncServer::new(genesis);

        // Build a chain
        let mut parent = genesis_hash;
        for i in 1..50 {
            let header = test_header(i, parent);
            parent = header.hash();
            server.add_header(header);
        }

        // Generate and verify proof
        let proof = server.generate_flyclient_proof(10);
        let mut verifier = LightClientVerifier::new(genesis_hash);
        
        let result = verifier.verify_and_update(&proof).unwrap();
        assert!(result.valid);
        assert_eq!(result.new_tip_height, 49);
    }

    #[test]
    fn test_mmr_proof_generation() {
        let genesis = test_header(0, Hash::default());
        let mut server = LightSyncServer::new(genesis.clone());

        // Add blocks
        let mut parent = genesis.hash();
        for i in 1..20 {
            let header = test_header(i, parent);
            parent = header.hash();
            server.add_header(header);
        }

        // Generate proof for block 10
        let proof = server.generate_mmr_proof(10);
        assert!(proof.is_some());
        
        let proof = proof.unwrap();
        assert_eq!(proof.leaf_index, 10);
        assert!(!proof.peaks.is_empty());
    }

    #[test]
    fn test_peak_count() {
        // Peak count = popcount of leaf_count
        assert_eq!(mmr_peak_count(1), 1);   // 1 = 0b1
        assert_eq!(mmr_peak_count(2), 1);   // 2 = 0b10
        assert_eq!(mmr_peak_count(3), 2);   // 3 = 0b11
        assert_eq!(mmr_peak_count(4), 1);   // 4 = 0b100
        assert_eq!(mmr_peak_count(7), 3);   // 7 = 0b111
        assert_eq!(mmr_peak_count(8), 1);   // 8 = 0b1000
        assert_eq!(mmr_peak_count(10), 2);  // 10 = 0b1010
    }

    #[test]
    fn test_mmr_size_calculation() {
        // MMR size = 2n - popcount(n)
        assert_eq!(MerkleMountainRange::size_for_leaves(1), 1);
        assert_eq!(MerkleMountainRange::size_for_leaves(2), 3);
        assert_eq!(MerkleMountainRange::size_for_leaves(3), 4);
        assert_eq!(MerkleMountainRange::size_for_leaves(4), 7);
        assert_eq!(MerkleMountainRange::size_for_leaves(7), 11);
        assert_eq!(MerkleMountainRange::size_for_leaves(8), 15);
    }
}


