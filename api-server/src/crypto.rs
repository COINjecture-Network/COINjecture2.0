//! Ed25519 signature verification for SIWB auth.
//!
//! Uses `ed25519-dalek` v2 directly (same version as `coinject-core`).
//! Kept as a standalone module to avoid pulling in the full core crate.

use ed25519_dalek::{Signature, Verifier, VerifyingKey};

/// Verify an Ed25519 signature over `message` using `public_key_bytes`.
///
/// Returns `Ok(true)` on valid signature, `Ok(false)` on invalid,
/// or `Err` if the key/signature bytes are malformed.
pub fn verify_ed25519_signature(
    public_key_bytes: &[u8],
    message: &[u8],
    signature_bytes: &[u8],
) -> Result<bool, String> {
    let pubkey_array: [u8; 32] = public_key_bytes
        .try_into()
        .map_err(|_| "Invalid public key length (expected 32 bytes)".to_string())?;

    let pubkey = VerifyingKey::from_bytes(&pubkey_array)
        .map_err(|e| format!("Invalid public key: {e}"))?;

    let sig_array: [u8; 64] = signature_bytes
        .try_into()
        .map_err(|_| "Invalid signature length (expected 64 bytes)".to_string())?;

    let sig = Signature::from_bytes(&sig_array);

    Ok(pubkey.verify(message, &sig).is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    #[test]
    fn roundtrip_sign_verify() {
        let signing_key = SigningKey::generate(&mut rand::rngs::OsRng);
        let message = b"hello coinjecture";
        let sig = signing_key.sign(message);

        let valid = verify_ed25519_signature(
            signing_key.verifying_key().as_bytes(),
            message,
            &sig.to_bytes(),
        )
        .unwrap();
        assert!(valid);
    }

    #[test]
    fn wrong_message_fails() {
        let signing_key = SigningKey::generate(&mut rand::rngs::OsRng);
        let sig = signing_key.sign(b"original");

        let valid = verify_ed25519_signature(
            signing_key.verifying_key().as_bytes(),
            b"tampered",
            &sig.to_bytes(),
        )
        .unwrap();
        assert!(!valid);
    }

    #[test]
    fn bad_key_length_errors() {
        let result = verify_ed25519_signature(&[0u8; 16], b"msg", &[0u8; 64]);
        assert!(result.is_err());
    }
}
