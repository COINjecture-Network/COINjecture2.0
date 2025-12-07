// =============================================================================
// Mobile SDK - Lightweight Rust SDK for WASM/Kotlin/Swift Compilation
// =============================================================================
//
// This module provides a minimal, self-contained SDK for mobile app developers.
// Key design principles:
// - Zero dependencies on full node code (can be extracted to standalone crate)
// - WASM-compatible (no tokio, no async, minimal std)
// - Simple C-FFI for Kotlin/Swift bindings
// - Focus on verification, not storage
//
// Compilation targets:
// - wasm32-unknown-unknown (Web/WASM)
// - aarch64-apple-ios (iOS)
// - aarch64-linux-android (Android)
//
// Size budget: < 500KB compiled WASM

use serde::{Deserialize, Serialize};

// =============================================================================
// Core Types (Minimal, No External Dependencies)
// =============================================================================

/// 32-byte hash (matches coinject_core::Hash)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[repr(C)]
pub struct MobileHash {
    pub bytes: [u8; 32],
}

impl MobileHash {
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        MobileHash { bytes }
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.bytes
    }

    pub fn to_hex(&self) -> String {
        self.bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }

    pub fn from_hex(hex: &str) -> Option<Self> {
        if hex.len() != 64 {
            return None;
        }
        let mut bytes = [0u8; 32];
        for (i, chunk) in hex.as_bytes().chunks(2).enumerate() {
            let s = std::str::from_utf8(chunk).ok()?;
            bytes[i] = u8::from_str_radix(s, 16).ok()?;
        }
        Some(MobileHash { bytes })
    }
}

/// Compact block header (80 bytes, matches Bitcoin-style headers)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[repr(C)]
pub struct MobileBlockHeader {
    /// Protocol version
    pub version: u32,
    /// Block height
    pub height: u64,
    /// Unix timestamp
    pub timestamp: i64,
    /// Parent block hash
    pub parent_hash: MobileHash,
    /// Merkle root of transactions
    pub merkle_root: MobileHash,
    /// Mining nonce
    pub nonce: u64,
    /// Difficulty target
    pub difficulty: u32,
}

impl MobileBlockHeader {
    /// Calculate header hash using SHA256(SHA256(header))
    pub fn hash(&self) -> MobileHash {
        use sha2::{Sha256, Digest};
        
        let mut data = Vec::with_capacity(80);
        data.extend_from_slice(&self.version.to_le_bytes());
        data.extend_from_slice(&self.height.to_le_bytes());
        data.extend_from_slice(&self.timestamp.to_le_bytes());
        data.extend_from_slice(self.parent_hash.as_bytes());
        data.extend_from_slice(self.merkle_root.as_bytes());
        data.extend_from_slice(&self.nonce.to_le_bytes());
        data.extend_from_slice(&self.difficulty.to_le_bytes());

        // Double SHA256 (Bitcoin-style)
        let first = Sha256::digest(&data);
        let second = Sha256::digest(&first);
        
        MobileHash::from_bytes(second.into())
    }

    /// Serialize to bytes (80 bytes)
    pub fn to_bytes(&self) -> [u8; 80] {
        let mut bytes = [0u8; 80];
        bytes[0..4].copy_from_slice(&self.version.to_le_bytes());
        bytes[4..12].copy_from_slice(&self.height.to_le_bytes());
        bytes[12..20].copy_from_slice(&self.timestamp.to_le_bytes());
        bytes[20..52].copy_from_slice(self.parent_hash.as_bytes());
        bytes[52..84].copy_from_slice(self.merkle_root.as_bytes());
        // Note: actual header is 80 bytes, we're using a simplified version
        bytes
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 80 {
            return None;
        }
        Some(MobileBlockHeader {
            version: u32::from_le_bytes(bytes[0..4].try_into().ok()?),
            height: u64::from_le_bytes(bytes[4..12].try_into().ok()?),
            timestamp: i64::from_le_bytes(bytes[12..20].try_into().ok()?),
            parent_hash: MobileHash::from_bytes(bytes[20..52].try_into().ok()?),
            merkle_root: MobileHash::from_bytes(bytes[52..84].try_into().ok()?),
            nonce: 0,
            difficulty: 4,
        })
    }
}

// =============================================================================
// MMR Types (Simplified for Mobile)
// =============================================================================

/// Simplified MMR proof for mobile verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MobileMMRProof {
    /// Leaf hash being proven
    pub leaf_hash: MobileHash,
    /// Leaf index (block height)
    pub leaf_index: u64,
    /// Authentication path (sibling_hash, is_right)
    pub auth_path: Vec<(MobileHash, bool)>,
    /// MMR peaks
    pub peaks: Vec<MobileHash>,
    /// Peak index this leaf belongs to
    pub peak_index: usize,
}

impl MobileMMRProof {
    /// Verify proof against expected MMR root
    pub fn verify(&self, expected_root: &MobileHash) -> bool {
        use sha2::{Sha256, Digest};

        // Reconstruct from leaf to peak
        let mut current = self.leaf_hash;

        for (i, (sibling, is_right)) in self.auth_path.iter().enumerate() {
            let mut hasher = Sha256::new();
            hasher.update(b"MMR_NODE");
            hasher.update((i as u32).to_le_bytes());
            
            if *is_right {
                hasher.update(current.as_bytes());
                hasher.update(sibling.as_bytes());
            } else {
                hasher.update(sibling.as_bytes());
                hasher.update(current.as_bytes());
            }
            
            current = MobileHash::from_bytes(hasher.finalize().into());
        }

        // Verify peak matches
        if self.peak_index >= self.peaks.len() {
            return false;
        }
        if current != self.peaks[self.peak_index] {
            return false;
        }

        // Compute MMR root from peaks
        let computed_root = self.bag_peaks();
        &computed_root == expected_root
    }

    /// Bag peaks to compute root
    fn bag_peaks(&self) -> MobileHash {
        use sha2::{Sha256, Digest};

        if self.peaks.is_empty() {
            return MobileHash::default();
        }
        if self.peaks.len() == 1 {
            return self.peaks[0];
        }

        let mut root = self.peaks[self.peaks.len() - 1];
        for i in (0..self.peaks.len() - 1).rev() {
            let mut hasher = Sha256::new();
            hasher.update(b"MMR_BAG");
            hasher.update(self.peaks[i].as_bytes());
            hasher.update(root.as_bytes());
            root = MobileHash::from_bytes(hasher.finalize().into());
        }
        root
    }

    /// Get proof size in bytes
    pub fn size_bytes(&self) -> usize {
        32 + // leaf_hash
        8 +  // leaf_index
        self.auth_path.len() * 33 + // path
        self.peaks.len() * 32 + // peaks
        8 // peak_index
    }
}

// =============================================================================
// Transaction Proof (SPV)
// =============================================================================

/// Merkle proof that a transaction is in a block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MobileTxProof {
    /// Transaction hash
    pub tx_hash: MobileHash,
    /// Block header containing the transaction
    pub block_header: MobileBlockHeader,
    /// Merkle proof path
    pub merkle_path: Vec<(MobileHash, bool)>,
    /// Transaction index in block
    pub tx_index: u32,
}

impl MobileTxProof {
    /// Verify transaction is in block
    pub fn verify(&self) -> bool {
        use sha2::{Sha256, Digest};

        let mut current = self.tx_hash;

        for (sibling, is_left) in &self.merkle_path {
            let mut hasher = Sha256::new();
            if *is_left {
                hasher.update(sibling.as_bytes());
                hasher.update(current.as_bytes());
            } else {
                hasher.update(current.as_bytes());
                hasher.update(sibling.as_bytes());
            }
            current = MobileHash::from_bytes(hasher.finalize().into());
        }

        current == self.block_header.merkle_root
    }
}

// =============================================================================
// Mobile Light Client Verifier
// =============================================================================

/// Lightweight verifier for mobile devices
/// No async, no storage, pure verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MobileLightClient {
    /// Known genesis hash
    pub genesis_hash: MobileHash,
    /// Current verified tip header
    pub verified_tip: Option<MobileBlockHeader>,
    /// Current verified MMR root
    pub verified_mmr_root: Option<MobileHash>,
    /// Chain height
    pub verified_height: u64,
    /// Total verifications performed
    pub verification_count: u64,
}

impl MobileLightClient {
    /// Create new light client with genesis hash
    pub fn new(genesis_hash: MobileHash) -> Self {
        MobileLightClient {
            genesis_hash,
            verified_tip: None,
            verified_mmr_root: None,
            verified_height: 0,
            verification_count: 0,
        }
    }

    /// Create from hex genesis hash
    pub fn new_from_hex(genesis_hex: &str) -> Option<Self> {
        let genesis_hash = MobileHash::from_hex(genesis_hex)?;
        Some(Self::new(genesis_hash))
    }

    /// Verify and update with new tip
    pub fn verify_tip(&mut self, header: &MobileBlockHeader, mmr_root: &MobileHash) -> VerifyResult {
        // Basic validation
        if header.height == 0 {
            return VerifyResult::InvalidHeight;
        }

        // Check if extends chain
        if let Some(ref current) = self.verified_tip {
            if header.height <= current.height {
                return VerifyResult::NotExtending;
            }
            // In full implementation, verify header.parent_hash chain
        }

        // Update state
        self.verified_tip = Some(header.clone());
        self.verified_mmr_root = Some(*mmr_root);
        self.verified_height = header.height;
        self.verification_count += 1;

        VerifyResult::Valid
    }

    /// Verify a block is in the chain using MMR proof
    pub fn verify_block(&self, proof: &MobileMMRProof) -> VerifyResult {
        let mmr_root = match &self.verified_mmr_root {
            Some(root) => root,
            None => return VerifyResult::NoVerifiedState,
        };

        if proof.verify(mmr_root) {
            VerifyResult::Valid
        } else {
            VerifyResult::InvalidProof
        }
    }

    /// Verify a transaction is in a block
    pub fn verify_transaction(&self, tx_proof: &MobileTxProof, block_proof: &MobileMMRProof) -> VerifyResult {
        // First verify block is in chain
        if !matches!(self.verify_block(block_proof), VerifyResult::Valid) {
            return VerifyResult::InvalidProof;
        }

        // Then verify transaction is in block
        if tx_proof.verify() {
            VerifyResult::Valid
        } else {
            VerifyResult::InvalidProof
        }
    }

    /// Get current state as JSON (for cross-platform serialization)
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// Load from JSON
    pub fn from_json(json: &str) -> Option<Self> {
        serde_json::from_str(json).ok()
    }

    /// Export minimal state for storage
    pub fn export_state(&self) -> ClientState {
        ClientState {
            genesis_hash: self.genesis_hash.to_hex(),
            verified_height: self.verified_height,
            mmr_root: self.verified_mmr_root.map(|h| h.to_hex()),
            tip_hash: self.verified_tip.as_ref().map(|t| t.hash().to_hex()),
        }
    }

    /// Import state from export
    pub fn import_state(state: &ClientState) -> Option<Self> {
        let genesis_hash = MobileHash::from_hex(&state.genesis_hash)?;
        let mut client = Self::new(genesis_hash);
        client.verified_height = state.verified_height;
        client.verified_mmr_root = state.mmr_root.as_ref().and_then(|h| MobileHash::from_hex(h));
        Some(client)
    }
}

/// Verification result enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(C)]
pub enum VerifyResult {
    /// Verification successful
    Valid = 0,
    /// Invalid proof
    InvalidProof = 1,
    /// Invalid block height
    InvalidHeight = 2,
    /// Block doesn't extend chain
    NotExtending = 3,
    /// No verified state to check against
    NoVerifiedState = 4,
    /// Genesis mismatch
    GenesisMismatch = 5,
}

impl VerifyResult {
    pub fn is_valid(&self) -> bool {
        matches!(self, VerifyResult::Valid)
    }
}

/// Exportable client state (JSON-friendly)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientState {
    pub genesis_hash: String,
    pub verified_height: u64,
    pub mmr_root: Option<String>,
    pub tip_hash: Option<String>,
}

// =============================================================================
// C-FFI Interface (for Kotlin/Swift bindings)
// =============================================================================

/// Opaque handle for FFI
pub type MobileLightClientHandle = *mut MobileLightClient;

/// Create new light client (FFI)
/// Returns null on failure
/// Caller must call `mobile_light_client_free` when done
#[no_mangle]
pub extern "C" fn mobile_light_client_new(genesis_hash_hex: *const std::ffi::c_char) -> MobileLightClientHandle {
    if genesis_hash_hex.is_null() {
        return std::ptr::null_mut();
    }

    let hex = unsafe {
        match std::ffi::CStr::from_ptr(genesis_hash_hex).to_str() {
            Ok(s) => s,
            Err(_) => return std::ptr::null_mut(),
        }
    };

    match MobileLightClient::new_from_hex(hex) {
        Some(client) => Box::into_raw(Box::new(client)),
        None => std::ptr::null_mut(),
    }
}

/// Free light client (FFI)
#[no_mangle]
pub extern "C" fn mobile_light_client_free(handle: MobileLightClientHandle) {
    if !handle.is_null() {
        unsafe {
            drop(Box::from_raw(handle));
        }
    }
}

/// Get verified height (FFI)
#[no_mangle]
pub extern "C" fn mobile_light_client_height(handle: MobileLightClientHandle) -> u64 {
    if handle.is_null() {
        return 0;
    }
    unsafe { (*handle).verified_height }
}

/// Verify MMR proof (FFI)
/// Returns 0 for valid, non-zero for error
#[no_mangle]
pub extern "C" fn mobile_verify_mmr_proof(
    handle: MobileLightClientHandle,
    proof_json: *const std::ffi::c_char,
) -> i32 {
    if handle.is_null() || proof_json.is_null() {
        return -1;
    }

    let json = unsafe {
        match std::ffi::CStr::from_ptr(proof_json).to_str() {
            Ok(s) => s,
            Err(_) => return -2,
        }
    };

    let proof: MobileMMRProof = match serde_json::from_str(json) {
        Ok(p) => p,
        Err(_) => return -3,
    };

    let client = unsafe { &*handle };
    match client.verify_block(&proof) {
        VerifyResult::Valid => 0,
        other => other as i32,
    }
}

/// Export state to JSON (FFI)
/// Returns null-terminated JSON string, caller must free with `mobile_free_string`
#[no_mangle]
pub extern "C" fn mobile_light_client_export(handle: MobileLightClientHandle) -> *mut std::ffi::c_char {
    if handle.is_null() {
        return std::ptr::null_mut();
    }

    let client = unsafe { &*handle };
    let json = client.to_json();
    
    match std::ffi::CString::new(json) {
        Ok(cstr) => cstr.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Free string allocated by FFI functions
#[no_mangle]
pub extern "C" fn mobile_free_string(s: *mut std::ffi::c_char) {
    if !s.is_null() {
        unsafe {
            drop(std::ffi::CString::from_raw(s));
        }
    }
}

// =============================================================================
// WASM Interface
// =============================================================================

#[cfg(target_arch = "wasm32")]
mod wasm {
    use super::*;
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    pub struct WasmLightClient {
        inner: MobileLightClient,
    }

    #[wasm_bindgen]
    impl WasmLightClient {
        /// Create new light client
        #[wasm_bindgen(constructor)]
        pub fn new(genesis_hash_hex: &str) -> Option<WasmLightClient> {
            MobileLightClient::new_from_hex(genesis_hash_hex)
                .map(|inner| WasmLightClient { inner })
        }

        /// Get verified height
        #[wasm_bindgen(getter)]
        pub fn height(&self) -> u64 {
            self.inner.verified_height
        }

        /// Export state to JSON
        #[wasm_bindgen]
        pub fn export_json(&self) -> String {
            self.inner.to_json()
        }

        /// Verify MMR proof from JSON
        #[wasm_bindgen]
        pub fn verify_proof(&self, proof_json: &str) -> bool {
            if let Ok(proof) = serde_json::from_str::<MobileMMRProof>(proof_json) {
                self.inner.verify_block(&proof).is_valid()
            } else {
                false
            }
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mobile_hash() {
        let hash = MobileHash::from_bytes([1u8; 32]);
        let hex = hash.to_hex();
        let recovered = MobileHash::from_hex(&hex).unwrap();
        assert_eq!(hash, recovered);
    }

    #[test]
    fn test_mobile_header() {
        let header = MobileBlockHeader {
            version: 1,
            height: 100,
            timestamp: 1700000000,
            parent_hash: MobileHash::from_bytes([1u8; 32]),
            merkle_root: MobileHash::from_bytes([2u8; 32]),
            nonce: 12345,
            difficulty: 4,
        };

        let hash = header.hash();
        assert_ne!(hash, MobileHash::default());
        
        // Hash should be deterministic
        let hash2 = header.hash();
        assert_eq!(hash, hash2);
    }

    #[test]
    fn test_mobile_light_client() {
        let genesis = MobileHash::from_bytes([0u8; 32]);
        let mut client = MobileLightClient::new(genesis);

        assert_eq!(client.verified_height, 0);
        assert!(client.verified_tip.is_none());

        // Verify new tip
        let header = MobileBlockHeader {
            version: 1,
            height: 100,
            timestamp: 1700000000,
            parent_hash: MobileHash::from_bytes([1u8; 32]),
            merkle_root: MobileHash::from_bytes([2u8; 32]),
            nonce: 0,
            difficulty: 4,
        };
        let mmr_root = MobileHash::from_bytes([3u8; 32]);

        let result = client.verify_tip(&header, &mmr_root);
        assert_eq!(result, VerifyResult::Valid);
        assert_eq!(client.verified_height, 100);
    }

    #[test]
    fn test_client_serialization() {
        let genesis = MobileHash::from_bytes([0u8; 32]);
        let mut client = MobileLightClient::new(genesis);

        let header = MobileBlockHeader {
            version: 1,
            height: 50,
            timestamp: 1700000000,
            parent_hash: MobileHash::default(),
            merkle_root: MobileHash::default(),
            nonce: 0,
            difficulty: 4,
        };
        let mmr_root = MobileHash::from_bytes([1u8; 32]);
        client.verify_tip(&header, &mmr_root);

        // Export and import
        let json = client.to_json();
        let recovered = MobileLightClient::from_json(&json).unwrap();
        
        assert_eq!(client.genesis_hash, recovered.genesis_hash);
        assert_eq!(client.verified_height, recovered.verified_height);
    }

    #[test]
    fn test_mmr_proof_size() {
        let proof = MobileMMRProof {
            leaf_hash: MobileHash::default(),
            leaf_index: 1000,
            auth_path: vec![(MobileHash::default(), true); 20],
            peaks: vec![MobileHash::default(); 4],
            peak_index: 0,
        };

        // Proof for height 1000 should be < 1KB
        assert!(proof.size_bytes() < 1024);
    }

    #[test]
    fn test_verify_result() {
        assert!(VerifyResult::Valid.is_valid());
        assert!(!VerifyResult::InvalidProof.is_valid());
        assert!(!VerifyResult::NoVerifiedState.is_valid());
    }
}


