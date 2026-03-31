// =============================================================================
// Node Identity Management
// =============================================================================
//
// Each node has a persistent Ed25519 keypair. On first run, a new keypair is
// generated and saved to disk. On subsequent runs, it's loaded from the file.
// The NodeId is the SHA-256 hash of the public key (32 bytes, hex-displayed).

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use std::path::Path;

use super::error::NetworkError;

/// A 32-byte node identifier derived from SHA-256(public_key).
///
/// Used throughout the protocol to identify peers. Displayed as hex.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub [u8; 32]);

impl NodeId {
    /// Derive a NodeId from an Ed25519 public key.
    pub fn from_public_key(pk: &VerifyingKey) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(pk.as_bytes());
        let hash = hasher.finalize();
        let mut id = [0u8; 32];
        id.copy_from_slice(&hash);
        NodeId(id)
    }

    /// Return the raw bytes.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Short hex representation (first 8 chars) for log readability.
    pub fn short(&self) -> String {
        hex::encode(&self.0[..4])
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(self.0))
    }
}

impl fmt::Debug for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NodeId({})", self.short())
    }
}

impl PartialOrd for NodeId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for NodeId {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

/// Holds the node's Ed25519 keypair and derived identity.
///
/// The keypair is used for signing messages and authenticating during handshake.
/// It is persisted to disk so the node keeps the same identity across restarts.
pub struct Keypair {
    signing_key: SigningKey,
    verifying_key: VerifyingKey,
    node_id: NodeId,
}

impl Keypair {
    /// Generate a brand new random keypair.
    pub fn generate() -> Self {
        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();
        let node_id = NodeId::from_public_key(&verifying_key);
        Self {
            signing_key,
            verifying_key,
            node_id,
        }
    }

    /// Load a keypair from disk, or generate and save a new one if the file
    /// doesn't exist. The file stores the 32-byte Ed25519 secret seed.
    pub fn load_or_generate(data_dir: &Path) -> Result<Self, NetworkError> {
        let key_path = data_dir.join("mesh_keypair.bin");

        if key_path.exists() {
            let bytes = std::fs::read(&key_path).map_err(|e| {
                NetworkError::Crypto(format!(
                    "failed to read keypair from {}: {}",
                    key_path.display(),
                    e
                ))
            })?;
            if bytes.len() != 32 {
                return Err(NetworkError::Crypto(format!(
                    "keypair file has wrong size: {} (expected 32)",
                    bytes.len()
                )));
            }
            let mut seed = [0u8; 32];
            seed.copy_from_slice(&bytes);
            let signing_key = SigningKey::from_bytes(&seed);
            let verifying_key = signing_key.verifying_key();
            let node_id = NodeId::from_public_key(&verifying_key);
            Ok(Self {
                signing_key,
                verifying_key,
                node_id,
            })
        } else {
            let keypair = Self::generate();
            std::fs::create_dir_all(data_dir).map_err(|e| {
                NetworkError::Crypto(format!(
                    "failed to create data dir {}: {}",
                    data_dir.display(),
                    e
                ))
            })?;
            std::fs::write(&key_path, keypair.signing_key.to_bytes()).map_err(|e| {
                NetworkError::Crypto(format!(
                    "failed to write keypair to {}: {}",
                    key_path.display(),
                    e
                ))
            })?;
            Ok(keypair)
        }
    }

    /// The node's unique identity.
    pub fn node_id(&self) -> &NodeId {
        &self.node_id
    }

    /// The Ed25519 public key bytes (for sending in handshake).
    pub fn public_key_bytes(&self) -> Vec<u8> {
        self.verifying_key.to_bytes().to_vec()
    }

    /// The verifying (public) key.
    pub fn verifying_key(&self) -> &VerifyingKey {
        &self.verifying_key
    }

    /// Sign arbitrary data with the node's private key.
    pub fn sign(&self, data: &[u8]) -> Vec<u8> {
        let sig: Signature = self.signing_key.sign(data);
        sig.to_bytes().to_vec()
    }
}

/// Verify an Ed25519 signature given the public key bytes, the data, and signature bytes.
pub fn verify_signature(
    public_key_bytes: &[u8],
    data: &[u8],
    signature_bytes: &[u8],
) -> Result<(), NetworkError> {
    let pk_array: [u8; 32] = public_key_bytes
        .try_into()
        .map_err(|_| NetworkError::Crypto("public key must be 32 bytes".into()))?;
    let verifying_key = VerifyingKey::from_bytes(&pk_array)
        .map_err(|e| NetworkError::Crypto(format!("invalid public key: {}", e)))?;

    let sig_array: [u8; 64] = signature_bytes
        .try_into()
        .map_err(|_| NetworkError::Crypto("signature must be 64 bytes".into()))?;
    let signature = Signature::from_bytes(&sig_array);

    verifying_key
        .verify(data, &signature)
        .map_err(|e| NetworkError::Crypto(format!("signature verification failed: {}", e)))
}

/// Verify a signature using a NodeId to confirm it matches the expected sender.
/// Returns the VerifyingKey on success so the caller can cache it.
pub fn verify_signature_for_node(
    expected_node_id: &NodeId,
    public_key_bytes: &[u8],
    data: &[u8],
    signature_bytes: &[u8],
) -> Result<VerifyingKey, NetworkError> {
    let pk_array: [u8; 32] = public_key_bytes
        .try_into()
        .map_err(|_| NetworkError::Crypto("public key must be 32 bytes".into()))?;
    let verifying_key = VerifyingKey::from_bytes(&pk_array)
        .map_err(|e| NetworkError::Crypto(format!("invalid public key: {}", e)))?;

    // Verify the public key matches the claimed NodeId
    let derived_id = NodeId::from_public_key(&verifying_key);
    if derived_id != *expected_node_id {
        return Err(NetworkError::Crypto(format!(
            "public key doesn't match node ID: expected {}, got {}",
            expected_node_id.short(),
            derived_id.short()
        )));
    }

    let sig_array: [u8; 64] = signature_bytes
        .try_into()
        .map_err(|_| NetworkError::Crypto("signature must be 64 bytes".into()))?;
    let signature = Signature::from_bytes(&sig_array);

    verifying_key
        .verify(data, &signature)
        .map_err(|e| NetworkError::Crypto(format!("signature verification failed: {}", e)))?;

    Ok(verifying_key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_id_from_public_key_deterministic() {
        let kp = Keypair::generate();
        let id1 = NodeId::from_public_key(kp.verifying_key());
        let id2 = NodeId::from_public_key(kp.verifying_key());
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_node_id_display_and_short() {
        let id = NodeId([0xAB; 32]);
        let display = format!("{}", id);
        assert_eq!(display.len(), 64); // 32 bytes = 64 hex chars
        assert!(display.starts_with("abab"));
        assert_eq!(id.short(), "abababab");
    }

    #[test]
    fn test_node_id_ordering() {
        let a = NodeId([0x00; 32]);
        let b = NodeId([0xFF; 32]);
        assert!(a < b);
    }

    #[test]
    fn test_keypair_sign_verify() {
        let kp = Keypair::generate();
        let data = b"hello mesh network";
        let sig = kp.sign(data);
        assert!(verify_signature(&kp.public_key_bytes(), data, &sig).is_ok());
    }

    #[test]
    fn test_signature_verification_rejects_tampered_data() {
        let kp = Keypair::generate();
        let data = b"original data";
        let sig = kp.sign(data);
        let result = verify_signature(&kp.public_key_bytes(), b"tampered data", &sig);
        assert!(result.is_err());
    }

    #[test]
    fn test_signature_verification_rejects_wrong_key() {
        let kp1 = Keypair::generate();
        let kp2 = Keypair::generate();
        let data = b"test data";
        let sig = kp1.sign(data);
        let result = verify_signature(&kp2.public_key_bytes(), data, &sig);
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_signature_for_node_checks_id() {
        let kp = Keypair::generate();
        let data = b"node verification test";
        let sig = kp.sign(data);

        // Should succeed with correct node ID
        let result = verify_signature_for_node(kp.node_id(), &kp.public_key_bytes(), data, &sig);
        assert!(result.is_ok());

        // Should fail with wrong node ID
        let wrong_id = NodeId([0xFF; 32]);
        let result = verify_signature_for_node(&wrong_id, &kp.public_key_bytes(), data, &sig);
        assert!(result.is_err());
    }

    #[test]
    fn test_keypair_persistence() {
        let dir = std::env::temp_dir().join(format!("coinject-test-{}", rand::random::<u32>()));
        let _ = std::fs::remove_dir_all(&dir);

        // Generate and save
        let kp1 = Keypair::load_or_generate(&dir).expect("generate");
        let id1 = *kp1.node_id();

        // Load from disk — should be the same identity
        let kp2 = Keypair::load_or_generate(&dir).expect("load");
        let id2 = *kp2.node_id();

        assert_eq!(id1, id2);

        // Cleanup
        let _ = std::fs::remove_dir_all(&dir);
    }
}
