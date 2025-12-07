// =============================================================================
// COINjecture Mobile SDK
// =============================================================================
//
// Standalone, lightweight SDK for mobile and WASM light clients.
//
// Design Principles:
// - ZERO dependencies on full node code
// - WASM-compatible (no tokio, no async in core types)
// - Simple C-FFI for Kotlin/Swift bindings
// - Focus on verification, not storage
// - Size budget: < 200KB compiled WASM
//
// Compilation Targets:
// - wasm32-unknown-unknown (Web/WASM)
// - aarch64-apple-ios (iOS)
// - aarch64-linux-android (Android)
// - Any other Rust-supported platform
//
// Usage:
//   # WASM build
//   wasm-pack build --target web --release
//
//   # iOS build
//   cargo build --target aarch64-apple-ios --release
//
//   # Android build
//   cargo build --target aarch64-linux-android --release

#![cfg_attr(target_arch = "wasm32", allow(unused_imports))]

use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use std::fmt;

// =============================================================================
// CORE TYPES
// =============================================================================

/// 32-byte cryptographic hash
/// Compatible with coinject_core::Hash but standalone
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[repr(C)]
pub struct Hash {
    bytes: [u8; 32],
}

impl Hash {
    /// Create hash from raw bytes
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Hash { bytes }
    }

    /// Get raw bytes
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.bytes
    }

    /// Convert to hex string
    pub fn to_hex(&self) -> String {
        hex::encode(&self.bytes)
    }

    /// Parse from hex string
    pub fn from_hex(hex_str: &str) -> Result<Self, ParseError> {
        let bytes = hex::decode(hex_str).map_err(|_| ParseError::InvalidHex)?;
        if bytes.len() != 32 {
            return Err(ParseError::InvalidLength);
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(Hash { bytes: arr })
    }

    /// Check if hash is all zeros
    pub fn is_zero(&self) -> bool {
        self.bytes.iter().all(|&b| b == 0)
    }

    /// Calculate SHA256(SHA256(data)) - Bitcoin-style double hash
    pub fn double_sha256(data: &[u8]) -> Self {
        let first = Sha256::digest(data);
        let second = Sha256::digest(&first);
        Hash::from_bytes(second.into())
    }

    /// Calculate SHA256(data) - single hash
    pub fn sha256(data: &[u8]) -> Self {
        let hash = Sha256::digest(data);
        Hash::from_bytes(hash.into())
    }
}

impl fmt::Debug for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Hash({})", &self.to_hex()[..16])
    }
}

impl fmt::Display for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

/// Parse errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseError {
    InvalidHex,
    InvalidLength,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::InvalidHex => write!(f, "Invalid hex string"),
            ParseError::InvalidLength => write!(f, "Invalid length"),
        }
    }
}

impl std::error::Error for ParseError {}

// =============================================================================
// BLOCK HEADER
// =============================================================================

/// Compact block header (80 bytes)
/// Contains all data needed for light client verification
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[repr(C)]
pub struct BlockHeader {
    /// Protocol version
    pub version: u32,
    /// Block height in chain
    pub height: u64,
    /// Unix timestamp
    pub timestamp: i64,
    /// Parent block hash
    pub parent_hash: Hash,
    /// Merkle root of transactions
    pub merkle_root: Hash,
    /// Mining nonce
    pub nonce: u64,
    /// Difficulty target
    pub difficulty: u32,
    /// Work score (for NP-hard PoW)
    pub work_score: u32,
}

impl BlockHeader {
    /// Calculate header hash
    pub fn hash(&self) -> Hash {
        let mut data = Vec::with_capacity(92);
        data.extend_from_slice(&self.version.to_le_bytes());
        data.extend_from_slice(&self.height.to_le_bytes());
        data.extend_from_slice(&self.timestamp.to_le_bytes());
        data.extend_from_slice(self.parent_hash.as_bytes());
        data.extend_from_slice(self.merkle_root.as_bytes());
        data.extend_from_slice(&self.nonce.to_le_bytes());
        data.extend_from_slice(&self.difficulty.to_le_bytes());
        data.extend_from_slice(&self.work_score.to_le_bytes());
        Hash::double_sha256(&data)
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(92);
        bytes.extend_from_slice(&self.version.to_le_bytes());
        bytes.extend_from_slice(&self.height.to_le_bytes());
        bytes.extend_from_slice(&self.timestamp.to_le_bytes());
        bytes.extend_from_slice(self.parent_hash.as_bytes());
        bytes.extend_from_slice(self.merkle_root.as_bytes());
        bytes.extend_from_slice(&self.nonce.to_le_bytes());
        bytes.extend_from_slice(&self.difficulty.to_le_bytes());
        bytes.extend_from_slice(&self.work_score.to_le_bytes());
        bytes
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ParseError> {
        if bytes.len() < 92 {
            return Err(ParseError::InvalidLength);
        }
        
        Ok(BlockHeader {
            version: u32::from_le_bytes(bytes[0..4].try_into().unwrap()),
            height: u64::from_le_bytes(bytes[4..12].try_into().unwrap()),
            timestamp: i64::from_le_bytes(bytes[12..20].try_into().unwrap()),
            parent_hash: Hash::from_bytes(bytes[20..52].try_into().unwrap()),
            merkle_root: Hash::from_bytes(bytes[52..84].try_into().unwrap()),
            nonce: u64::from_le_bytes(bytes[84..92].try_into().unwrap_or([0; 8])),
            difficulty: 4,
            work_score: 0,
        })
    }
}

// =============================================================================
// MMR (MERKLE MOUNTAIN RANGE) TYPES
// =============================================================================

/// MMR inclusion proof
/// Proves a leaf (block header) is in the MMR
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MMRProof {
    /// Hash of the leaf being proven
    pub leaf_hash: Hash,
    /// Leaf index (block height)
    pub leaf_index: u64,
    /// MMR size at time of proof
    pub mmr_size: u64,
    /// Authentication path (sibling_hash, is_right_sibling)
    pub auth_path: Vec<(Hash, bool)>,
    /// Peak index this leaf belongs to
    pub peak_index: usize,
    /// All MMR peaks for root calculation
    pub peaks: Vec<Hash>,
}

impl MMRProof {
    /// Verify proof against expected MMR root
    pub fn verify(&self, expected_root: &Hash) -> bool {
        // Reconstruct from leaf to peak
        let mut current = self.leaf_hash;

        for (height, (sibling, is_right)) in self.auth_path.iter().enumerate() {
            current = Self::hash_mmr_node(&current, sibling, *is_right, height as u32);
        }

        // Verify we reached the correct peak
        if self.peak_index >= self.peaks.len() {
            return false;
        }
        if current != self.peaks[self.peak_index] {
            return false;
        }

        // Bag peaks to compute root
        let computed_root = Self::bag_peaks(&self.peaks);
        &computed_root == expected_root
    }

    /// Hash two MMR nodes together
    fn hash_mmr_node(left: &Hash, right: &Hash, is_right: bool, height: u32) -> Hash {
        let mut hasher = Sha256::new();
        hasher.update(b"MMR_NODE");
        hasher.update(height.to_le_bytes());
        
        if is_right {
            // Our node is on the left
            hasher.update(left.as_bytes());
            hasher.update(right.as_bytes());
        } else {
            // Our node is on the right
            hasher.update(right.as_bytes());
            hasher.update(left.as_bytes());
        }
        
        Hash::from_bytes(hasher.finalize().into())
    }

    /// Bag peaks into MMR root
    fn bag_peaks(peaks: &[Hash]) -> Hash {
        if peaks.is_empty() {
            return Hash::default();
        }
        if peaks.len() == 1 {
            return peaks[0];
        }

        let mut root = peaks[peaks.len() - 1];
        for i in (0..peaks.len() - 1).rev() {
            let mut hasher = Sha256::new();
            hasher.update(b"MMR_BAG");
            hasher.update(peaks[i].as_bytes());
            hasher.update(root.as_bytes());
            root = Hash::from_bytes(hasher.finalize().into());
        }
        root
    }

    /// Get proof size in bytes
    pub fn size_bytes(&self) -> usize {
        32 + 8 + 8 + // leaf_hash, leaf_index, mmr_size
        self.auth_path.len() * 33 + // (hash + bool)
        8 + // peak_index
        self.peaks.len() * 32 // peaks
    }
}

// =============================================================================
// FLYCLIENT PROOF
// =============================================================================

/// FlyClient proof for super-light verification
/// O(log n) proof size instead of O(n) for full header chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlyClientProof {
    /// Genesis hash for chain identification
    pub genesis_hash: Hash,
    /// Chain tip header
    pub tip_header: BlockHeader,
    /// MMR root at chain tip
    pub mmr_root: Hash,
    /// Sampled headers with proofs
    pub samples: Vec<SampledBlock>,
    /// Total chain work
    pub total_work: u128,
    /// Security parameter used
    pub security_param: usize,
}

/// A sampled block in a FlyClient proof
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SampledBlock {
    /// Block header
    pub header: BlockHeader,
    /// MMR proof for this header
    pub mmr_proof: MMRProof,
    /// Sampling weight
    pub weight: f64,
}

impl FlyClientProof {
    /// Verify the FlyClient proof
    pub fn verify(&self) -> Result<bool, VerifyError> {
        // Check tip is valid
        if self.tip_header.height == 0 {
            return Err(VerifyError::InvalidTip);
        }

        // Verify all sampled headers have valid MMR proofs
        for sample in &self.samples {
            if !sample.mmr_proof.verify(&self.mmr_root) {
                return Err(VerifyError::InvalidMMRProof(sample.header.height));
            }
        }

        // Check sufficient samples
        let min_samples = self.security_param.min(self.tip_header.height as usize);
        if self.samples.len() < min_samples {
            return Err(VerifyError::InsufficientSamples);
        }

        Ok(true)
    }

    /// Get proof size in bytes
    pub fn size_bytes(&self) -> usize {
        32 + // genesis_hash
        92 + // tip_header
        32 + // mmr_root
        self.samples.iter().map(|s| 92 + s.mmr_proof.size_bytes() + 8).sum::<usize>() +
        16 + 8 // total_work + security_param
    }
}

// =============================================================================
// TRANSACTION PROOF (SPV)
// =============================================================================

/// Merkle proof for transaction inclusion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxProof {
    /// Transaction hash
    pub tx_hash: Hash,
    /// Block header containing the transaction
    pub block_header: BlockHeader,
    /// Merkle proof path
    pub merkle_path: Vec<(Hash, bool)>,
    /// Transaction index in block
    pub tx_index: u32,
}

impl TxProof {
    /// Verify transaction is in block
    pub fn verify(&self) -> bool {
        let mut current = self.tx_hash;

        for (sibling, is_right) in &self.merkle_path {
            let mut hasher = Sha256::new();
            hasher.update(b"MERKLE_NODE");
            
            if *is_right {
                hasher.update(current.as_bytes());
                hasher.update(sibling.as_bytes());
            } else {
                hasher.update(sibling.as_bytes());
                hasher.update(current.as_bytes());
            }
            
            current = Hash::from_bytes(hasher.finalize().into());
        }

        current == self.block_header.merkle_root
    }
}

// =============================================================================
// LIGHT CLIENT
// =============================================================================

/// Mobile light client verifier
/// Stateful verifier that tracks the verified chain state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightClient {
    /// Known genesis hash
    genesis_hash: Hash,
    /// Current verified tip
    verified_tip: Option<BlockHeader>,
    /// Current verified MMR root
    verified_mmr_root: Option<Hash>,
    /// Verified chain height
    verified_height: u64,
    /// Total verifications performed
    verification_count: u64,
}

impl LightClient {
    /// Create new light client
    pub fn new(genesis_hash: Hash) -> Self {
        LightClient {
            genesis_hash,
            verified_tip: None,
            verified_mmr_root: None,
            verified_height: 0,
            verification_count: 0,
        }
    }

    /// Create from hex genesis hash
    pub fn from_genesis_hex(hex: &str) -> Result<Self, ParseError> {
        let genesis_hash = Hash::from_hex(hex)?;
        Ok(Self::new(genesis_hash))
    }

    /// Verify FlyClient proof and update state
    pub fn verify_flyclient(&mut self, proof: &FlyClientProof) -> Result<(), VerifyError> {
        // Check genesis matches
        if proof.genesis_hash != self.genesis_hash {
            return Err(VerifyError::GenesisMismatch);
        }

        // Verify the proof
        proof.verify()?;

        // Update state if this extends our chain
        if proof.tip_header.height > self.verified_height {
            self.verified_tip = Some(proof.tip_header.clone());
            self.verified_mmr_root = Some(proof.mmr_root);
            self.verified_height = proof.tip_header.height;
        }
        
        self.verification_count += 1;
        Ok(())
    }

    /// Verify a block is in the chain
    pub fn verify_block(&self, proof: &MMRProof) -> Result<bool, VerifyError> {
        let mmr_root = self.verified_mmr_root
            .ok_or(VerifyError::NoVerifiedState)?;
        Ok(proof.verify(&mmr_root))
    }

    /// Verify a transaction is in the chain
    pub fn verify_transaction(
        &self,
        tx_proof: &TxProof,
        block_proof: &MMRProof,
    ) -> Result<bool, VerifyError> {
        // First verify block is in chain
        if !self.verify_block(block_proof)? {
            return Ok(false);
        }

        // Then verify tx is in block
        Ok(tx_proof.verify())
    }

    // === Getters ===

    pub fn genesis_hash(&self) -> Hash {
        self.genesis_hash
    }

    pub fn verified_height(&self) -> u64 {
        self.verified_height
    }

    pub fn verified_mmr_root(&self) -> Option<Hash> {
        self.verified_mmr_root
    }

    pub fn verified_tip(&self) -> Option<&BlockHeader> {
        self.verified_tip.as_ref()
    }

    pub fn verification_count(&self) -> u64 {
        self.verification_count
    }

    // === Serialization ===

    /// Export to JSON
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// Import from JSON
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// Verification errors
#[derive(Debug, Clone)]
pub enum VerifyError {
    GenesisMismatch,
    InvalidTip,
    InvalidMMRProof(u64),
    InsufficientSamples,
    NoVerifiedState,
}

impl fmt::Display for VerifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VerifyError::GenesisMismatch => write!(f, "Genesis hash mismatch"),
            VerifyError::InvalidTip => write!(f, "Invalid chain tip"),
            VerifyError::InvalidMMRProof(h) => write!(f, "Invalid MMR proof for block {}", h),
            VerifyError::InsufficientSamples => write!(f, "Insufficient proof samples"),
            VerifyError::NoVerifiedState => write!(f, "No verified state available"),
        }
    }
}

impl std::error::Error for VerifyError {}

// =============================================================================
// C-FFI INTERFACE
// =============================================================================

#[cfg(feature = "ffi")]
pub mod ffi {
    use super::*;
    use std::ffi::{CStr, CString};
    use std::os::raw::c_char;

    /// Opaque handle for FFI
    pub type LightClientHandle = *mut LightClient;

    /// Create new light client (FFI)
    #[no_mangle]
    pub extern "C" fn coinject_light_client_new(genesis_hex: *const c_char) -> LightClientHandle {
        if genesis_hex.is_null() {
            return std::ptr::null_mut();
        }

        let hex = unsafe {
            match CStr::from_ptr(genesis_hex).to_str() {
                Ok(s) => s,
                Err(_) => return std::ptr::null_mut(),
            }
        };

        match LightClient::from_genesis_hex(hex) {
            Ok(client) => Box::into_raw(Box::new(client)),
            Err(_) => std::ptr::null_mut(),
        }
    }

    /// Free light client (FFI)
    #[no_mangle]
    pub extern "C" fn coinject_light_client_free(handle: LightClientHandle) {
        if !handle.is_null() {
            unsafe { drop(Box::from_raw(handle)) }
        }
    }

    /// Get verified height (FFI)
    #[no_mangle]
    pub extern "C" fn coinject_light_client_height(handle: LightClientHandle) -> u64 {
        if handle.is_null() { return 0; }
        unsafe { (*handle).verified_height() }
    }

    /// Verify MMR proof from JSON (FFI)
    /// Returns 0 for success, negative for error
    #[no_mangle]
    pub extern "C" fn coinject_verify_mmr_proof(
        handle: LightClientHandle,
        proof_json: *const c_char,
    ) -> i32 {
        if handle.is_null() || proof_json.is_null() {
            return -1;
        }

        let json = unsafe {
            match CStr::from_ptr(proof_json).to_str() {
                Ok(s) => s,
                Err(_) => return -2,
            }
        };

        let proof: MMRProof = match serde_json::from_str(json) {
            Ok(p) => p,
            Err(_) => return -3,
        };

        let client = unsafe { &*handle };
        match client.verify_block(&proof) {
            Ok(true) => 0,
            Ok(false) => 1,
            Err(_) => -4,
        }
    }

    /// Export state to JSON (FFI)
    /// Caller must free with coinject_free_string
    #[no_mangle]
    pub extern "C" fn coinject_light_client_export(handle: LightClientHandle) -> *mut c_char {
        if handle.is_null() {
            return std::ptr::null_mut();
        }

        let client = unsafe { &*handle };
        let json = client.to_json();
        
        match CString::new(json) {
            Ok(cstr) => cstr.into_raw(),
            Err(_) => std::ptr::null_mut(),
        }
    }

    /// Free string allocated by FFI
    #[no_mangle]
    pub extern "C" fn coinject_free_string(s: *mut c_char) {
        if !s.is_null() {
            unsafe { drop(CString::from_raw(s)) }
        }
    }
}

// =============================================================================
// WASM INTERFACE
// =============================================================================

#[cfg(all(target_arch = "wasm32", feature = "wasm"))]
pub mod wasm {
    use super::*;
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen(start)]
    pub fn init() {
        #[cfg(feature = "console_error_panic_hook")]
        console_error_panic_hook::set_once();
    }

    /// WASM-exposed light client
    #[wasm_bindgen]
    pub struct WasmLightClient {
        inner: LightClient,
    }

    #[wasm_bindgen]
    impl WasmLightClient {
        /// Create new light client
        #[wasm_bindgen(constructor)]
        pub fn new(genesis_hex: &str) -> Result<WasmLightClient, JsError> {
            LightClient::from_genesis_hex(genesis_hex)
                .map(|inner| WasmLightClient { inner })
                .map_err(|e| JsError::new(&e.to_string()))
        }

        /// Get verified height
        #[wasm_bindgen(getter)]
        pub fn height(&self) -> u64 {
            self.inner.verified_height()
        }

        /// Get genesis hash as hex
        #[wasm_bindgen(getter)]
        pub fn genesis(&self) -> String {
            self.inner.genesis_hash().to_hex()
        }

        /// Get MMR root as hex (or empty if not verified)
        #[wasm_bindgen(getter)]
        pub fn mmr_root(&self) -> String {
            self.inner.verified_mmr_root()
                .map(|h| h.to_hex())
                .unwrap_or_default()
        }

        /// Export state to JSON
        #[wasm_bindgen]
        pub fn export_json(&self) -> String {
            self.inner.to_json()
        }

        /// Import state from JSON
        #[wasm_bindgen]
        pub fn import_json(json: &str) -> Result<WasmLightClient, JsError> {
            LightClient::from_json(json)
                .map(|inner| WasmLightClient { inner })
                .map_err(|e| JsError::new(&e.to_string()))
        }

        /// Verify MMR proof from JSON
        #[wasm_bindgen]
        pub fn verify_mmr_proof(&self, proof_json: &str) -> Result<bool, JsError> {
            let proof: MMRProof = serde_json::from_str(proof_json)
                .map_err(|e| JsError::new(&format!("Invalid proof JSON: {}", e)))?;
            
            self.inner.verify_block(&proof)
                .map_err(|e| JsError::new(&e.to_string()))
        }

        /// Verify FlyClient proof from JSON
        #[wasm_bindgen]
        pub fn verify_flyclient_proof(&mut self, proof_json: &str) -> Result<bool, JsError> {
            let proof: FlyClientProof = serde_json::from_str(proof_json)
                .map_err(|e| JsError::new(&format!("Invalid proof JSON: {}", e)))?;
            
            self.inner.verify_flyclient(&proof)
                .map(|_| true)
                .map_err(|e| JsError::new(&e.to_string()))
        }
    }

    /// Hash a hex string using double SHA256
    #[wasm_bindgen]
    pub fn hash_double_sha256(data_hex: &str) -> Result<String, JsError> {
        let bytes = hex::decode(data_hex)
            .map_err(|e| JsError::new(&format!("Invalid hex: {}", e)))?;
        Ok(Hash::double_sha256(&bytes).to_hex())
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_roundtrip() {
        let hash = Hash::from_bytes([0x42; 32]);
        let hex = hash.to_hex();
        let recovered = Hash::from_hex(&hex).unwrap();
        assert_eq!(hash, recovered);
    }

    #[test]
    fn test_hash_sha256() {
        let data = b"hello world";
        let hash = Hash::sha256(data);
        // Known SHA256 of "hello world"
        assert_eq!(
            hash.to_hex(),
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn test_block_header_hash_deterministic() {
        let header = BlockHeader {
            version: 1,
            height: 100,
            timestamp: 1700000000,
            parent_hash: Hash::from_bytes([1u8; 32]),
            merkle_root: Hash::from_bytes([2u8; 32]),
            nonce: 12345,
            difficulty: 4,
            work_score: 100,
        };

        let hash1 = header.hash();
        let hash2 = header.hash();
        assert_eq!(hash1, hash2);
        assert!(!hash1.is_zero());
    }

    #[test]
    fn test_light_client_creation() {
        let genesis = Hash::from_bytes([0u8; 32]);
        let client = LightClient::new(genesis);
        
        assert_eq!(client.verified_height(), 0);
        assert!(client.verified_tip().is_none());
        assert!(client.verified_mmr_root().is_none());
    }

    #[test]
    fn test_light_client_json_roundtrip() {
        let genesis = Hash::from_bytes([0xAB; 32]);
        let client = LightClient::new(genesis);
        
        let json = client.to_json();
        let recovered = LightClient::from_json(&json).unwrap();
        
        assert_eq!(client.genesis_hash(), recovered.genesis_hash());
        assert_eq!(client.verified_height(), recovered.verified_height());
    }

    #[test]
    fn test_mmr_proof_size() {
        let proof = MMRProof {
            leaf_hash: Hash::default(),
            leaf_index: 10000,
            mmr_size: 19999,
            auth_path: vec![(Hash::default(), true); 14], // log2(10000) ≈ 14
            peak_index: 0,
            peaks: vec![Hash::default(); 4],
        };

        // Proof for height 10000 should be < 1KB
        assert!(proof.size_bytes() < 1024);
    }

    #[test]
    fn test_verify_error_display() {
        let err = VerifyError::GenesisMismatch;
        assert_eq!(format!("{}", err), "Genesis hash mismatch");

        let err = VerifyError::InvalidMMRProof(100);
        assert_eq!(format!("{}", err), "Invalid MMR proof for block 100");
    }
}
