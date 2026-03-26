// Validator Keystore — Encrypted at Rest
//
// Security model:
//   - Private key material is NEVER written to disk in plaintext.
//   - On disk: AES-256-GCM ciphertext with an argon2id-derived key.
//   - In memory: ValidatorKey implements Drop via zeroize to wipe secret_key.
//   - Password source: COINJECT_KEYSTORE_PASSWORD env var (required for production).
//
// Encrypted file format (binary):
//   [4 bytes]  magic "CIKV"  (COINject Key Vault)
//   [1 byte]   version 0x01
//   [32 bytes] argon2id salt
//   [12 bytes] AES-256-GCM nonce
//   [N bytes]  AES-256-GCM ciphertext (plaintext + 16-byte auth tag)
//
//   Plaintext inside ciphertext = [32 bytes public_key || 32 bytes secret_key || 32 bytes address]
//
// KDF parameters (argon2id):
//   m = 65536 KiB (64 MiB memory cost)
//   t = 3 iterations
//   p = 1 parallelism
//   output = 32 bytes
//
// These parameters are deliberately conservative for validator nodes
// that restart infrequently. Tune down only if cold-start latency is critical.

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use argon2::{Algorithm, Argon2, Params, Version};
use coinject_core::Address;
use ed25519_dalek::SigningKey;
use rand::{rngs::OsRng, RngCore};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use zeroize::Zeroize;

// ─── File format constants ────────────────────────────────────────────────────

const KEYSTORE_MAGIC: &[u8; 4] = b"CIKV";
const KEYSTORE_VERSION: u8 = 0x01;

// Offsets within the file
const MAGIC_END: usize = 4;
const VERSION_OFFSET: usize = 4;
const SALT_START: usize = 5;
const SALT_END: usize = 37;
const NONCE_START: usize = 37;
const NONCE_END: usize = 49;
const CIPHERTEXT_START: usize = 49;
const MIN_FILE_SIZE: usize = CIPHERTEXT_START + 16; // ciphertext is at least one auth tag

// ─── KDF parameters ──────────────────────────────────────────────────────────

/// argon2id memory cost in KiB (64 MiB)
const ARGON2_MEM_KIB: u32 = 65536;
/// argon2id iteration count
const ARGON2_ITERATIONS: u32 = 3;
/// argon2id degree of parallelism
const ARGON2_PARALLELISM: u32 = 1;
/// Derived key length (matches AES-256 key size)
const KEY_LEN: usize = 32;

// ─── ValidatorKey ─────────────────────────────────────────────────────────────

/// Validator key material, held in memory only.
///
/// The `secret_key` field is zeroized (overwritten with zeros) when this struct
/// is dropped, preventing key material from lingering in freed memory.
#[derive(Clone)]
pub struct ValidatorKey {
    /// Validator address derived from public key via BLAKE3
    pub address: Address,
    /// Ed25519 verifying key (32 bytes)
    pub public_key: [u8; 32],
    /// Ed25519 signing key (32 bytes) — NEVER written to disk in plaintext
    secret_key: [u8; 32],
}

impl Drop for ValidatorKey {
    fn drop(&mut self) {
        self.secret_key.zeroize();
    }
}

/// Never print the secret key in Debug output.
impl std::fmt::Debug for ValidatorKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ValidatorKey")
            .field("address", &hex::encode(self.address.as_bytes()))
            .field("public_key", &hex::encode(self.public_key))
            .field("secret_key", &"[REDACTED]")
            .finish()
    }
}

#[allow(dead_code)]
impl ValidatorKey {
    /// Generate a fresh random validator key.
    pub fn generate() -> Self {
        let signing_key = SigningKey::generate(&mut OsRng);
        let public_key = signing_key.verifying_key().to_bytes();
        let secret_key = signing_key.to_bytes();

        let address_hash = blake3::hash(&public_key);
        let mut address_bytes = [0u8; 32];
        address_bytes.copy_from_slice(address_hash.as_bytes());

        ValidatorKey {
            address: Address::from_bytes(address_bytes),
            public_key,
            secret_key,
        }
    }

    /// Load and decrypt a validator key from `path`.
    ///
    /// Password is read from `COINJECT_KEYSTORE_PASSWORD` or `COINJECT_VALIDATOR_PASSWORD`
    /// environment variables. A missing password logs a warning and uses an empty string
    /// (insecure; only acceptable for local development).
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let password = keystore_password();
        Self::load_with_password(path, &password)
    }

    /// Load and decrypt a validator key using an explicit `password`.
    pub fn load_with_password<P: AsRef<Path>>(path: P, password: &str) -> Result<Self, String> {
        let data = fs::read(path.as_ref())
            .map_err(|e| format!("Failed to read keystore: {}", e))?;
        decrypt_validator_key(&data, password)
    }

    /// Encrypt and write this validator key to `path`.
    ///
    /// Password is read from environment (see [`load`]).
    /// On Unix, the file is written with mode `0600`.
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), String> {
        let password = keystore_password();
        self.save_with_password(path, &password)
    }

    /// Encrypt and write this validator key using an explicit `password`.
    pub fn save_with_password<P: AsRef<Path>>(&self, path: P, password: &str) -> Result<(), String> {
        let encrypted = encrypt_validator_key(self, password)?;

        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create keystore directory: {}", e))?;
        }

        fs::write(path.as_ref(), &encrypted)
            .map_err(|e| format!("Failed to write keystore file: {}", e))?;

        // Restrict to owner read/write on Unix.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(path.as_ref())
                .map_err(|e| format!("Failed to stat keystore: {}", e))?
                .permissions();
            perms.set_mode(0o600);
            fs::set_permissions(path.as_ref(), perms)
                .map_err(|e| format!("Failed to chmod keystore: {}", e))?;
        }

        Ok(())
    }

    /// Return an ed25519 [`SigningKey`] from the secret material.
    pub fn signing_key(&self) -> SigningKey {
        SigningKey::from_bytes(&self.secret_key)
    }

    /// Return the validator's on-chain address.
    pub fn address(&self) -> Address {
        self.address
    }

    /// Return the verifying-key bytes (public key).
    pub fn public_key_bytes(&self) -> &[u8; 32] {
        &self.public_key
    }
}

// ─── Encryption helpers ───────────────────────────────────────────────────────

/// Serialize and encrypt `key` under `password`, returning the file bytes.
fn encrypt_validator_key(key: &ValidatorKey, password: &str) -> Result<Vec<u8>, String> {
    let mut rng = OsRng;

    // Random salt and nonce — unique per file write.
    let mut salt = [0u8; 32];
    let mut nonce_bytes = [0u8; 12];
    rng.fill_bytes(&mut salt);
    rng.fill_bytes(&mut nonce_bytes);

    // Derive a 256-bit encryption key from the password + salt.
    let mut enc_key = [0u8; KEY_LEN];
    derive_key(password, &salt, &mut enc_key)?;

    // Plaintext = public_key (32) || secret_key (32) || address (32) = 96 bytes.
    let mut plaintext = [0u8; 96];
    plaintext[0..32].copy_from_slice(&key.public_key);
    plaintext[32..64].copy_from_slice(&key.secret_key);
    plaintext[64..96].copy_from_slice(key.address.as_bytes());

    // Encrypt with AES-256-GCM.
    let cipher_key = Key::<Aes256Gcm>::from_slice(&enc_key);
    let cipher = Aes256Gcm::new(cipher_key);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_ref())
        .map_err(|e| format!("AES-GCM encryption failed: {}", e))?;

    // Wipe sensitive in-memory buffers.
    enc_key.zeroize();
    plaintext.zeroize();

    // Assemble the file.
    let mut out = Vec::with_capacity(CIPHERTEXT_START + ciphertext.len());
    out.extend_from_slice(KEYSTORE_MAGIC);
    out.push(KEYSTORE_VERSION);
    out.extend_from_slice(&salt);
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);

    Ok(out)
}

/// Decrypt file bytes produced by [`encrypt_validator_key`].
fn decrypt_validator_key(data: &[u8], password: &str) -> Result<ValidatorKey, String> {
    if data.len() < MIN_FILE_SIZE {
        return Err(format!(
            "Keystore file too small ({} bytes); expected at least {}",
            data.len(),
            MIN_FILE_SIZE
        ));
    }

    if &data[0..MAGIC_END] != KEYSTORE_MAGIC {
        return Err(
            "Invalid keystore magic. \
             This file may be an unencrypted legacy key. \
             Regenerate with: coinject keygen"
                .to_string(),
        );
    }

    if data[VERSION_OFFSET] != KEYSTORE_VERSION {
        return Err(format!(
            "Unsupported keystore version 0x{:02x}",
            data[VERSION_OFFSET]
        ));
    }

    let salt = &data[SALT_START..SALT_END];
    let nonce_bytes = &data[NONCE_START..NONCE_END];
    let ciphertext = &data[CIPHERTEXT_START..];

    // Derive encryption key.
    let mut enc_key = [0u8; KEY_LEN];
    derive_key(password, salt, &mut enc_key)?;

    // Decrypt.
    let cipher_key = Key::<Aes256Gcm>::from_slice(&enc_key);
    let cipher = Aes256Gcm::new(cipher_key);
    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| "Decryption failed — wrong password or corrupted keystore".to_string())?;

    enc_key.zeroize();

    // Plaintext should be exactly 96 bytes.
    if plaintext.len() != 96 {
        return Err(format!(
            "Unexpected plaintext length {} (expected 96)",
            plaintext.len()
        ));
    }

    let mut public_key = [0u8; 32];
    let mut secret_key = [0u8; 32];
    let mut address_bytes = [0u8; 32];
    public_key.copy_from_slice(&plaintext[0..32]);
    secret_key.copy_from_slice(&plaintext[32..64]);
    address_bytes.copy_from_slice(&plaintext[64..96]);

    Ok(ValidatorKey {
        address: Address::from_bytes(address_bytes),
        public_key,
        secret_key,
    })
}

/// Derive a symmetric key from `password` and `salt` using argon2id.
fn derive_key(password: &str, salt: &[u8], key_out: &mut [u8]) -> Result<(), String> {
    let params = Params::new(ARGON2_MEM_KIB, ARGON2_ITERATIONS, ARGON2_PARALLELISM, Some(key_out.len()))
        .map_err(|e| format!("Invalid argon2 parameters: {}", e))?;

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    argon2
        .hash_password_into(password.as_bytes(), salt, key_out)
        .map_err(|e| format!("argon2id key derivation failed: {}", e))?;

    Ok(())
}

/// Read the keystore password from environment variables.
///
/// Priority:
///   1. `COINJECT_KEYSTORE_PASSWORD`
///   2. `COINJECT_VALIDATOR_PASSWORD`
///   3. Empty string — logs a security warning; do not use in production.
fn keystore_password() -> String {
    if let Ok(pw) = env::var("COINJECT_KEYSTORE_PASSWORD") {
        return pw;
    }
    if let Ok(pw) = env::var("COINJECT_VALIDATOR_PASSWORD") {
        return pw;
    }
    eprintln!(
        "SECURITY WARNING: No keystore password configured.\n\
         Set COINJECT_KEYSTORE_PASSWORD environment variable before starting the node.\n\
         Using an empty password is insecure and must NOT be used in production."
    );
    String::new()
}

// ─── ValidatorKeystore ────────────────────────────────────────────────────────

/// Manages the on-disk validator key file for a node.
pub struct ValidatorKeystore {
    key_file: PathBuf,
}

impl ValidatorKeystore {
    /// Create a keystore manager rooted at `data_dir`.
    pub fn new(data_dir: &Path) -> Self {
        ValidatorKeystore {
            key_file: data_dir.join("validator_key.bin"),
        }
    }

    /// Load the existing validator key, or generate and persist a new one.
    ///
    /// Password is read from `COINJECT_KEYSTORE_PASSWORD` environment variable.
    pub fn get_or_create_key(&self) -> Result<ValidatorKey, String> {
        let password = keystore_password();
        self.get_or_create_with_password(&password)
    }

    /// Load or create a validator key using an explicit `password`.
    pub fn get_or_create_with_password(&self, password: &str) -> Result<ValidatorKey, String> {
        if self.key_file.exists() {
            println!(
                "= Loading validator key from: {}",
                self.key_file.display()
            );
            ValidatorKey::load_with_password(&self.key_file, password)
        } else {
            println!("= Generating new validator key (encrypted)…");
            let key = ValidatorKey::generate();
            key.save_with_password(&self.key_file, password)?;
            println!(
                "   Validator address : {}",
                hex::encode(key.address.as_bytes())
            );
            println!("   Key file          : {}", self.key_file.display());
            Ok(key)
        }
    }

    /// Import a validator key from a hex-encoded secret key.
    ///
    /// The key is immediately encrypted and written to disk.
    #[allow(dead_code)]
    pub fn import_key_with_password(
        &self,
        secret_key_hex: &str,
        password: &str,
    ) -> Result<ValidatorKey, String> {
        let secret_bytes = hex::decode(secret_key_hex)
            .map_err(|e| format!("Invalid hex: {}", e))?;

        if secret_bytes.len() != 32 {
            return Err(format!(
                "Secret key must be 32 bytes, got {}",
                secret_bytes.len()
            ));
        }

        let mut secret_key = [0u8; 32];
        secret_key.copy_from_slice(&secret_bytes);

        let signing_key = SigningKey::from_bytes(&secret_key);
        let public_key = signing_key.verifying_key().to_bytes();

        let address_hash = blake3::hash(&public_key);
        let mut address_bytes = [0u8; 32];
        address_bytes.copy_from_slice(address_hash.as_bytes());

        let key = ValidatorKey {
            address: Address::from_bytes(address_bytes),
            public_key,
            secret_key,
        };

        key.save_with_password(&self.key_file, password)?;
        println!(
            " Validator key imported (encrypted)\n   Address: {}",
            hex::encode(key.address.as_bytes())
        );

        Ok(key)
    }

    /// Export the secret key as a hex string.
    ///
    /// **WARNING:** This exposes the raw private key. Use only for migrations.
    #[allow(dead_code)]
    pub fn export_key_with_password(&self, password: &str) -> Result<String, String> {
        let key = ValidatorKey::load_with_password(&self.key_file, password)?;
        Ok(hex::encode(key.signing_key().to_bytes()))
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    const TEST_PASSWORD: &str = "test-password-do-not-use-in-production";

    #[test]
    fn test_generate_key() {
        let key = ValidatorKey::generate();
        assert_eq!(key.public_key.len(), 32);
        assert_eq!(key.secret_key.len(), 32);
    }

    #[test]
    fn test_encrypted_save_load_round_trip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test_validator_key.bin");

        let key1 = ValidatorKey::generate();
        key1.save_with_password(&path, TEST_PASSWORD).unwrap();

        // File must exist and be non-empty
        assert!(path.exists());
        let bytes = fs::read(&path).unwrap();
        assert!(&bytes[..4] == KEYSTORE_MAGIC, "Magic header missing");

        // Load and verify round-trip
        let key2 = ValidatorKey::load_with_password(&path, TEST_PASSWORD).unwrap();
        assert_eq!(key1.address, key2.address);
        assert_eq!(key1.public_key, key2.public_key);
        assert_eq!(key1.signing_key().to_bytes(), key2.signing_key().to_bytes());
    }

    #[test]
    fn test_wrong_password_fails() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("key.bin");

        let key = ValidatorKey::generate();
        key.save_with_password(&path, TEST_PASSWORD).unwrap();

        let result = ValidatorKey::load_with_password(&path, "wrong-password");
        assert!(result.is_err(), "Wrong password should fail decryption");
    }

    #[test]
    fn test_plaintext_file_rejected() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("plaintext.bin");

        // Write garbage that doesn't start with CIKV
        fs::write(&path, b"not a valid keystore file").unwrap();

        let result = ValidatorKey::load_with_password(&path, TEST_PASSWORD);
        assert!(result.is_err());
        let msg = result.unwrap_err();
        assert!(
            msg.contains("magic") || msg.contains("small"),
            "Error should mention magic or size: {}",
            msg
        );
    }

    #[test]
    fn test_get_or_create_key() {
        let dir = tempdir().unwrap();
        let keystore = ValidatorKeystore::new(dir.path());

        // First call: creates a new key
        let key1 = keystore.get_or_create_with_password(TEST_PASSWORD).unwrap();

        // Second call: loads the same key
        let key2 = keystore.get_or_create_with_password(TEST_PASSWORD).unwrap();

        assert_eq!(key1.address, key2.address);
        assert_eq!(key1.public_key, key2.public_key);
    }

    #[test]
    fn test_different_passwords_produce_different_ciphertexts() {
        let dir = tempdir().unwrap();
        let path1 = dir.path().join("key1.bin");
        let path2 = dir.path().join("key2.bin");

        // Same key, different passwords
        let key = ValidatorKey::generate();
        key.save_with_password(&path1, "password-alpha").unwrap();
        key.save_with_password(&path2, "password-beta").unwrap();

        let bytes1 = fs::read(&path1).unwrap();
        let bytes2 = fs::read(&path2).unwrap();

        // Different salts/nonces should produce different ciphertexts
        assert_ne!(bytes1, bytes2);
    }

    #[test]
    fn test_signing_key_works() {
        let key = ValidatorKey::generate();
        let sk = key.signing_key();
        let vk = sk.verifying_key();
        assert_eq!(vk.to_bytes(), key.public_key);
    }

    #[test]
    fn test_debug_redacts_secret() {
        let key = ValidatorKey::generate();
        let debug_str = format!("{:?}", key);
        assert!(debug_str.contains("REDACTED"), "secret_key must be redacted");
        assert!(!debug_str.contains(&hex::encode(key.signing_key().to_bytes())));
    }
}
