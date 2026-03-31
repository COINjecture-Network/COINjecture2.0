//! Peer identity verification for Noise-authenticated connections.

use sha2::{Digest, Sha256};

/// A verified peer identity derived from a Noise static public key.
#[derive(Debug, Clone)]
pub struct PeerIdentity {
    pub noise_public_key: [u8; 32],
    /// SHA-256(noise_public_key)[..20] as hex (40 chars).
    pub peer_id: String,
}

impl PeerIdentity {
    pub fn from_noise_key(key: &[u8; 32]) -> Self {
        let hash = Sha256::digest(key);
        Self {
            noise_public_key: *key,
            peer_id: hex::encode(&hash[..20]),
        }
    }

    /// Check that a peer_id matches a given public key.
    pub fn verify_peer_id(peer_id: &str, public_key: &[u8; 32]) -> bool {
        let expected = Self::from_noise_key(public_key);
        expected.peer_id == peer_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::noise::NoiseKeypair;

    #[test]
    fn test_identity_from_key() {
        let kp = NoiseKeypair::generate();
        let id = PeerIdentity::from_noise_key(&kp.public_key);
        assert_eq!(id.peer_id.len(), 40);
        assert_eq!(id.noise_public_key, kp.public_key);
    }

    #[test]
    fn test_verify_peer_id() {
        let kp = NoiseKeypair::generate();
        let id = PeerIdentity::from_noise_key(&kp.public_key);
        assert!(PeerIdentity::verify_peer_id(&id.peer_id, &kp.public_key));

        let other = NoiseKeypair::generate();
        assert!(!PeerIdentity::verify_peer_id(
            &id.peer_id,
            &other.public_key
        ));
    }

    #[test]
    fn test_deterministic() {
        let kp = NoiseKeypair::generate();
        let id1 = PeerIdentity::from_noise_key(&kp.public_key);
        let id2 = PeerIdentity::from_noise_key(&kp.public_key);
        assert_eq!(id1.peer_id, id2.peer_id);
    }
}
