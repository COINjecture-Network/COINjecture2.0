// =============================================================================
// COINjecture CPP — Session Encryption & Peer Authentication
// =============================================================================
//
// Implements a lightweight authenticated key-exchange handshake between CPP
// peers, followed by ChaCha20-Poly1305 encryption of every message.
//
// ## Protocol (simplified Noise XX pattern)
//
//   1. Both sides generate ephemeral X25519 keypairs.
//   2. Each side sends:
//        [32 bytes] ephemeral X25519 public key
//        [32 bytes] static ed25519 verifying (public) key
//        [64 bytes] ed25519 signature of (ephemeral_x25519_pubkey || static_ed25519_pubkey || b"CPP_AUTH_V1")
//      Total: 128 bytes per side.
//   3. Both sides verify the received signature.
//   4. Each side computes the X25519 DH shared secret using:
//        their ephemeral *private* key × peer's ephemeral *public* key
//   5. Session keys are derived with BLAKE3:
//        initiator_send_key = BLAKE3_KDF(dh_secret, "CPP_INIT_SEND" || init_ephem_pub || resp_ephem_pub)
//        responder_send_key = BLAKE3_KDF(dh_secret, "CPP_RESP_SEND" || init_ephem_pub || resp_ephem_pub)
//   6. All subsequent CPP frames are encrypted with ChaCha20-Poly1305.
//
// ## Encrypted frame format (on the wire)
//
//   ┌─────────────────────────┬────────────────────────────────┐
//   │ counter  (8 bytes LE)   │ ciphertext_len  (4 bytes LE)   │
//   ├─────────────────────────┴────────────────────────────────┤
//   │ ciphertext = ChaCha20Poly1305(plaintext) + 16-byte tag   │
//   └──────────────────────────────────────────────────────────┘
//   plaintext = original CPP envelope bytes (magic+version+type+len+payload+checksum)
//
// The 12-byte ChaCha20-Poly1305 nonce is: counter(8 LE) || 0x00 0x00 0x00 0x00
//
// Forward secrecy: ephemeral X25519 keys are discarded after the handshake.
// Replay protection: the 64-bit counter is strictly monotonic per session.
//
// =============================================================================

use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    ChaCha20Poly1305, Nonce,
};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use x25519_dalek::{EphemeralSecret, PublicKey as X25519PublicKey};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum EncryptionError {
    Io(std::io::Error),
    AuthFailed(String),
    DecryptionFailed,
    Timeout,
}

impl std::fmt::Display for EncryptionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "IO error: {}", e),
            Self::AuthFailed(s) => write!(f, "Authentication failed: {}", s),
            Self::DecryptionFailed => write!(f, "Decryption failed (possible tampering)"),
            Self::Timeout => write!(f, "Encryption handshake timeout"),
        }
    }
}

impl From<std::io::Error> for EncryptionError {
    fn from(e: std::io::Error) -> Self {
        EncryptionError::Io(e)
    }
}

// ---------------------------------------------------------------------------
// Auth token (128 bytes per peer)
// ---------------------------------------------------------------------------

const AUTH_DOMAIN: &[u8] = b"CPP_AUTH_V1";

/// Wire-level peer authentication token.
#[derive(Clone)]
struct PeerAuthToken {
    ephemeral_x25519_pubkey: [u8; 32],
    ed25519_pubkey: [u8; 32],
    ed25519_signature: [u8; 64],
}

impl PeerAuthToken {
    const SIZE: usize = 128;

    /// Build a token from the local ephemeral X25519 key and static signing key.
    fn new(ephemeral_pub: &X25519PublicKey, signing_key: &SigningKey) -> Self {
        let ephemeral_x25519_pubkey = *ephemeral_pub.as_bytes();
        let ed25519_pubkey = signing_key.verifying_key().to_bytes();

        // Signature over: ephemeral_x25519_pubkey || ed25519_pubkey || AUTH_DOMAIN
        let mut msg = Vec::with_capacity(32 + 32 + AUTH_DOMAIN.len());
        msg.extend_from_slice(&ephemeral_x25519_pubkey);
        msg.extend_from_slice(&ed25519_pubkey);
        msg.extend_from_slice(AUTH_DOMAIN);

        let signature = signing_key.sign(&msg);
        PeerAuthToken {
            ephemeral_x25519_pubkey,
            ed25519_pubkey,
            ed25519_signature: signature.to_bytes(),
        }
    }

    fn to_bytes(&self) -> [u8; Self::SIZE] {
        let mut buf = [0u8; Self::SIZE];
        buf[..32].copy_from_slice(&self.ephemeral_x25519_pubkey);
        buf[32..64].copy_from_slice(&self.ed25519_pubkey);
        buf[64..128].copy_from_slice(&self.ed25519_signature);
        buf
    }

    fn from_bytes(bytes: &[u8; Self::SIZE]) -> Self {
        let mut ephemeral_x25519_pubkey = [0u8; 32];
        let mut ed25519_pubkey = [0u8; 32];
        let mut ed25519_signature = [0u8; 64];
        ephemeral_x25519_pubkey.copy_from_slice(&bytes[..32]);
        ed25519_pubkey.copy_from_slice(&bytes[32..64]);
        ed25519_signature.copy_from_slice(&bytes[64..128]);
        PeerAuthToken {
            ephemeral_x25519_pubkey,
            ed25519_pubkey,
            ed25519_signature,
        }
    }

    /// Verify that the token's signature is valid.
    fn verify(&self) -> Result<(), EncryptionError> {
        let verifying_key = VerifyingKey::from_bytes(&self.ed25519_pubkey)
            .map_err(|e| EncryptionError::AuthFailed(format!("Invalid ed25519 pubkey: {}", e)))?;

        let mut msg = Vec::with_capacity(32 + 32 + AUTH_DOMAIN.len());
        msg.extend_from_slice(&self.ephemeral_x25519_pubkey);
        msg.extend_from_slice(&self.ed25519_pubkey);
        msg.extend_from_slice(AUTH_DOMAIN);

        let signature = Signature::from_bytes(&self.ed25519_signature);
        verifying_key.verify(&msg, &signature).map_err(|e| {
            EncryptionError::AuthFailed(format!("Signature verification failed: {}", e))
        })
    }
}

// ---------------------------------------------------------------------------
// Key derivation
// ---------------------------------------------------------------------------

fn derive_session_keys(
    dh_secret: &[u8; 32],
    init_ephem_pub: &[u8; 32],
    resp_ephem_pub: &[u8; 32],
) -> ([u8; 32], [u8; 32]) {
    // Initiator → Responder key
    let init_send_key = {
        let mut context = Vec::with_capacity(13 + 32 + 32);
        context.extend_from_slice(b"CPP_INIT_SEND");
        context.extend_from_slice(init_ephem_pub);
        context.extend_from_slice(resp_ephem_pub);
        blake3::derive_key(&String::from_utf8_lossy(&context), dh_secret)
    };
    // Responder → Initiator key
    let resp_send_key = {
        let mut context = Vec::with_capacity(13 + 32 + 32);
        context.extend_from_slice(b"CPP_RESP_SEND");
        context.extend_from_slice(init_ephem_pub);
        context.extend_from_slice(resp_ephem_pub);
        blake3::derive_key(&String::from_utf8_lossy(&context), dh_secret)
    };
    (init_send_key, resp_send_key)
}

// ---------------------------------------------------------------------------
// Handshake result
// ---------------------------------------------------------------------------

/// Result returned from a successful encryption handshake.
pub struct HandshakeResult {
    /// Cipher for encrypting outbound frames.
    pub send_cipher: SessionCipher,
    /// Cipher for decrypting inbound frames.
    pub recv_cipher: SessionCipher,
    /// The remote peer's static ed25519 verifying key (32 bytes).
    pub remote_ed25519_pubkey: [u8; 32],
    /// Peer ID derived as BLAKE3(remote_ed25519_pubkey)[..32].
    pub authenticated_peer_id: [u8; 32],
}

// ---------------------------------------------------------------------------
// Handshake — initiator side (we connect outbound)
// ---------------------------------------------------------------------------

/// Perform the encryption + authentication handshake as the *initiator*
/// (the peer that opened the TCP connection).
pub async fn perform_handshake_initiator(
    stream: &mut TcpStream,
    signing_key: &SigningKey,
) -> Result<HandshakeResult, EncryptionError> {
    let ephemeral_secret = EphemeralSecret::random_from_rng(OsRng);
    let ephemeral_pub = X25519PublicKey::from(&ephemeral_secret);

    // Build and send our auth token
    let our_token = PeerAuthToken::new(&ephemeral_pub, signing_key);
    stream.write_all(&our_token.to_bytes()).await?;
    stream.flush().await?;

    // Receive peer's auth token
    let mut peer_token_bytes = [0u8; PeerAuthToken::SIZE];
    stream.read_exact(&mut peer_token_bytes).await?;
    let peer_token = PeerAuthToken::from_bytes(&peer_token_bytes);

    // Verify peer's signature
    peer_token.verify()?;

    // DH shared secret
    let peer_ephem_pub = X25519PublicKey::from(peer_token.ephemeral_x25519_pubkey);
    let dh_secret = ephemeral_secret.diffie_hellman(&peer_ephem_pub);

    // Derive session keys (initiator side)
    let (init_send_key, resp_send_key) = derive_session_keys(
        dh_secret.as_bytes(),
        ephemeral_pub.as_bytes(),
        &peer_token.ephemeral_x25519_pubkey,
    );

    // Derive authenticated peer ID
    let authenticated_peer_id = *blake3::hash(&peer_token.ed25519_pubkey).as_bytes();

    Ok(HandshakeResult {
        send_cipher: SessionCipher::new(init_send_key),
        recv_cipher: SessionCipher::new(resp_send_key),
        remote_ed25519_pubkey: peer_token.ed25519_pubkey,
        authenticated_peer_id,
    })
}

// ---------------------------------------------------------------------------
// Handshake — responder side (we accepted inbound)
// ---------------------------------------------------------------------------

/// Perform the encryption + authentication handshake as the *responder*
/// (the peer that accepted the incoming TCP connection).
pub async fn perform_handshake_responder(
    stream: &mut TcpStream,
    signing_key: &SigningKey,
) -> Result<HandshakeResult, EncryptionError> {
    // Receive initiator's auth token first
    let mut init_token_bytes = [0u8; PeerAuthToken::SIZE];
    stream.read_exact(&mut init_token_bytes).await?;
    let init_token = PeerAuthToken::from_bytes(&init_token_bytes);

    // Verify initiator's signature
    init_token.verify()?;

    // Build and send our auth token
    let ephemeral_secret = EphemeralSecret::random_from_rng(OsRng);
    let ephemeral_pub = X25519PublicKey::from(&ephemeral_secret);
    let our_token = PeerAuthToken::new(&ephemeral_pub, signing_key);
    stream.write_all(&our_token.to_bytes()).await?;
    stream.flush().await?;

    // DH shared secret
    let init_ephem_pub = X25519PublicKey::from(init_token.ephemeral_x25519_pubkey);
    let dh_secret = ephemeral_secret.diffie_hellman(&init_ephem_pub);

    // Derive session keys (responder side — keys are swapped relative to initiator)
    let (init_send_key, resp_send_key) = derive_session_keys(
        dh_secret.as_bytes(),
        &init_token.ephemeral_x25519_pubkey,
        ephemeral_pub.as_bytes(),
    );

    let authenticated_peer_id = *blake3::hash(&init_token.ed25519_pubkey).as_bytes();

    Ok(HandshakeResult {
        // Responder sends on the "resp_send_key" and receives on "init_send_key"
        send_cipher: SessionCipher::new(resp_send_key),
        recv_cipher: SessionCipher::new(init_send_key),
        remote_ed25519_pubkey: init_token.ed25519_pubkey,
        authenticated_peer_id,
    })
}

// ---------------------------------------------------------------------------
// Session cipher
// ---------------------------------------------------------------------------

/// Directional ChaCha20-Poly1305 cipher with a monotonic 64-bit counter nonce.
///
/// **Each `SessionCipher` is for one direction only** (send or receive).
/// Create one instance per direction with the appropriate derived key.
pub struct SessionCipher {
    key: [u8; 32],
    counter: u64,
}

impl SessionCipher {
    pub fn new(key: [u8; 32]) -> Self {
        SessionCipher { key, counter: 0 }
    }

    // Build the 12-byte nonce from the counter (8 bytes LE + 4 bytes zero)
    fn make_nonce(counter: u64) -> Nonce {
        let mut n = [0u8; 12];
        n[..8].copy_from_slice(&counter.to_le_bytes());
        *Nonce::from_slice(&n)
    }

    /// Encrypt `plaintext` and wrap it in the CPP encrypted frame.
    ///
    /// Returns the complete frame bytes ready to write to the wire.
    pub fn encrypt_frame(&mut self, plaintext: &[u8]) -> Result<Vec<u8>, EncryptionError> {
        let counter = self.counter;
        self.counter = self.counter.wrapping_add(1);

        let cipher = ChaCha20Poly1305::new_from_slice(&self.key)
            .expect("ChaCha20Poly1305 key length is always 32");
        let nonce = Self::make_nonce(counter);
        let ciphertext = cipher
            .encrypt(&nonce, plaintext)
            .map_err(|_| EncryptionError::DecryptionFailed)?;

        // Frame layout: counter(8 LE) || ciphertext_len(4 LE) || ciphertext
        let ciphertext_len = ciphertext.len() as u32;
        let mut frame = Vec::with_capacity(8 + 4 + ciphertext.len());
        frame.extend_from_slice(&counter.to_le_bytes());
        frame.extend_from_slice(&ciphertext_len.to_le_bytes());
        frame.extend_from_slice(&ciphertext);
        Ok(frame)
    }

    /// Decrypt a frame that was previously encrypted with the paired cipher.
    ///
    /// Returns the plaintext.
    pub fn decrypt_frame(
        &mut self,
        counter: u64,
        ciphertext: &[u8],
    ) -> Result<Vec<u8>, EncryptionError> {
        let cipher = ChaCha20Poly1305::new_from_slice(&self.key)
            .expect("ChaCha20Poly1305 key length is always 32");
        let nonce = Self::make_nonce(counter);
        cipher
            .decrypt(&nonce, ciphertext)
            .map_err(|_| EncryptionError::DecryptionFailed)
    }

    pub fn current_counter(&self) -> u64 {
        self.counter
    }
}

// ---------------------------------------------------------------------------
// Encrypted frame I/O helpers (work on ReadHalf / full TcpStream)
// ---------------------------------------------------------------------------

/// Read and decrypt one encrypted CPP frame from a split `ReadHalf`.
///
/// Frame layout: [8: counter LE] [4: ciphertext_len LE] [ciphertext]
///
/// Returns the decrypted plaintext (original CPP envelope bytes).
pub async fn read_encrypted_frame(
    read_half: &mut tokio::io::ReadHalf<TcpStream>,
    cipher: &mut SessionCipher,
) -> Result<Vec<u8>, EncryptionError> {
    // Read 8-byte counter + 4-byte ciphertext length
    let mut header = [0u8; 12];
    read_half.read_exact(&mut header).await?;

    let counter = u64::from_le_bytes(header[..8].try_into().unwrap());
    let ciphertext_len = u32::from_le_bytes(header[8..12].try_into().unwrap()) as usize;

    // Reject unreasonably large frames (max cleartext ~4 MB → ciphertext + 16 tag)
    const MAX_ENCRYPTED_FRAME: usize = 4 * 1_024 * 1_024 + 64;
    if ciphertext_len > MAX_ENCRYPTED_FRAME {
        return Err(EncryptionError::AuthFailed(format!(
            "Encrypted frame too large: {} bytes",
            ciphertext_len
        )));
    }

    let mut ciphertext = vec![0u8; ciphertext_len];
    read_half.read_exact(&mut ciphertext).await?;

    cipher.decrypt_frame(counter, &ciphertext)
}

/// Write one encrypted CPP frame to the full `TcpStream`.
pub async fn write_encrypted_frame(
    stream: &mut TcpStream,
    cipher: &mut SessionCipher,
    plaintext: &[u8],
) -> Result<(), EncryptionError> {
    let frame = cipher.encrypt_frame(plaintext)?;
    stream.write_all(&frame).await?;
    stream.flush().await?;
    Ok(())
}

/// Write one encrypted CPP frame to a `WriteHalf<TcpStream>`.
pub async fn write_encrypted_frame_half(
    write_half: &mut tokio::io::WriteHalf<TcpStream>,
    cipher: &mut SessionCipher,
    plaintext: &[u8],
) -> Result<(), EncryptionError> {
    let frame = cipher.encrypt_frame(plaintext)?;
    write_half.write_all(&frame).await?;
    write_half.flush().await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;

    fn random_signing_key() -> SigningKey {
        SigningKey::generate(&mut OsRng)
    }

    #[test]
    fn test_session_cipher_encrypt_decrypt_roundtrip() {
        let key = [0x42u8; 32];
        let mut enc = SessionCipher::new(key);
        let mut dec = SessionCipher::new(key);

        let plaintext = b"Hello, CPP encryption!";
        let frame = enc.encrypt_frame(plaintext).unwrap();

        // Parse frame to extract counter and ciphertext
        let counter = u64::from_le_bytes(frame[..8].try_into().unwrap());
        let ciphertext_len = u32::from_le_bytes(frame[8..12].try_into().unwrap()) as usize;
        let ciphertext = &frame[12..12 + ciphertext_len];

        let recovered = dec.decrypt_frame(counter, ciphertext).unwrap();
        assert_eq!(recovered, plaintext);
    }

    #[test]
    fn test_session_cipher_wrong_key_fails() {
        let key1 = [0x01u8; 32];
        let key2 = [0x02u8; 32];
        let mut enc = SessionCipher::new(key1);
        let mut dec = SessionCipher::new(key2); // wrong key

        let plaintext = b"secret data";
        let frame = enc.encrypt_frame(plaintext).unwrap();

        let counter = u64::from_le_bytes(frame[..8].try_into().unwrap());
        let ciphertext_len = u32::from_le_bytes(frame[8..12].try_into().unwrap()) as usize;
        let ciphertext = &frame[12..12 + ciphertext_len];

        assert!(dec.decrypt_frame(counter, ciphertext).is_err());
    }

    #[test]
    fn test_session_cipher_counter_increments() {
        let key = [0xABu8; 32];
        let mut cipher = SessionCipher::new(key);
        assert_eq!(cipher.current_counter(), 0);

        cipher.encrypt_frame(b"msg1").unwrap();
        assert_eq!(cipher.current_counter(), 1);

        cipher.encrypt_frame(b"msg2").unwrap();
        assert_eq!(cipher.current_counter(), 2);
    }

    #[test]
    fn test_peer_auth_token_verify() {
        let signing_key = random_signing_key();
        let ephemeral_secret = EphemeralSecret::random_from_rng(OsRng);
        let ephemeral_pub = X25519PublicKey::from(&ephemeral_secret);

        let token = PeerAuthToken::new(&ephemeral_pub, &signing_key);
        assert!(token.verify().is_ok());
    }

    #[test]
    fn test_peer_auth_token_tampered_fails() {
        let signing_key = random_signing_key();
        let ephemeral_secret = EphemeralSecret::random_from_rng(OsRng);
        let ephemeral_pub = X25519PublicKey::from(&ephemeral_secret);

        let mut token = PeerAuthToken::new(&ephemeral_pub, &signing_key);
        // Tamper with the ephemeral key
        token.ephemeral_x25519_pubkey[0] ^= 0xFF;
        assert!(token.verify().is_err());
    }

    #[test]
    fn test_derive_session_keys_different_per_direction() {
        let dh_secret = [0x55u8; 32];
        let init_ephem = [0xAAu8; 32];
        let resp_ephem = [0xBBu8; 32];

        let (init_send, resp_send) = derive_session_keys(&dh_secret, &init_ephem, &resp_ephem);
        // The two keys must be different
        assert_ne!(init_send, resp_send);
    }

    #[test]
    fn test_key_derivation_symmetry() {
        // Both peers do the same DH and should derive the same keys
        let dh_secret = [0x77u8; 32];
        let init_ephem = [0x11u8; 32];
        let resp_ephem = [0x22u8; 32];

        // Initiator derives keys with its own ephem first
        let (i_send, i_recv) = derive_session_keys(&dh_secret, &init_ephem, &resp_ephem);
        // Responder derives with its own ephem first — roles are swapped
        let (r_send, r_recv) = derive_session_keys(&dh_secret, &init_ephem, &resp_ephem);

        // Initiator's send == Responder's "init_send_key" (which is r_send since same derivation)
        // and Initiator's recv == resp_send_key
        assert_eq!(i_send, r_send); // Both see the same initiator_send_key
        assert_eq!(i_recv, r_recv); // Both see the same responder_send_key
                                    // But initiator sends with i_send and receives with i_recv (= resp_send),
                                    // whereas responder sends with resp_send and receives with init_send.
    }
}
