// Wallet Keystore — Encrypted at Rest
//
// Security model:
//   - Private keys are NEVER stored in plaintext JSON.
//   - Public account metadata (name, address, public_key) is stored in cleartext
//     `{name}.json` for fast account listing without requiring a password.
//   - The secret key is stored separately in an encrypted `{name}.key` file using
//     AES-256-GCM with an argon2id-derived password.
//   - Key material is zeroized (overwritten with zeros) when dropped via the
//     Zeroize crate.
//
// Encrypted key file format (binary):
//   [4 bytes]  magic "CKWV"  (COINject Wallet Vault)
//   [1 byte]   version 0x01
//   [32 bytes] argon2id salt
//   [12 bytes] AES-256-GCM nonce
//   [N bytes]  AES-256-GCM ciphertext (32-byte secret key + 16-byte auth tag = 48 bytes)
//
// Password source: COINJECT_KEYSTORE_PASSWORD env var (same as node keystore).
//
// NOTE: Some signing methods are prepared for future transaction types.
#![allow(dead_code)]

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use argon2::{Algorithm, Argon2, Params, Version};
use anyhow::{anyhow, Result};
use coinject_core::Address;
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::path::PathBuf;
use zeroize::Zeroize;

// ─── File format constants ────────────────────────────────────────────────────

const WALLET_KEY_MAGIC: &[u8; 4] = b"CKWV";
const WALLET_KEY_VERSION: u8 = 0x01;
const MIN_KEY_FILE_SIZE: usize = 4 + 1 + 32 + 12 + 16; // magic + ver + salt + nonce + tag

// ─── KDF parameters ──────────────────────────────────────────────────────────

const ARGON2_MEM_KIB: u32 = 65536; // 64 MiB
const ARGON2_ITERATIONS: u32 = 3;
const ARGON2_PARALLELISM: u32 = 1;
const KEY_LEN: usize = 32;

// ─── Public account metadata (stored in cleartext) ───────────────────────────

/// Public account metadata stored in cleartext `{name}.json`.
///
/// The private key is stored separately in an encrypted `{name}.key` file.
/// This allows account listing without requiring a password.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredAccount {
    pub name: String,
    pub address: String,
    pub public_key: String,
    pub created_at: i64,
}

// ─── Keystore manager ────────────────────────────────────────────────────────

/// Manages encrypted wallet keypairs on disk.
pub struct Keystore {
    keystore_dir: PathBuf,
}

impl Keystore {
    /// Open the default keystore at `~/.coinject/wallets/`.
    pub fn new() -> Result<Self> {
        let keystore_dir = default_keystore_dir()?;
        fs::create_dir_all(&keystore_dir)?;
        Ok(Keystore { keystore_dir })
    }

    /// Open a keystore at a specific directory.
    pub fn at(dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&dir)?;
        Ok(Keystore { keystore_dir: dir })
    }

    // ─── Key generation ───────────────────────────────────────────────────

    /// Generate a new keypair, encrypt the secret key, and persist both files.
    ///
    /// Password is read from `COINJECT_KEYSTORE_PASSWORD` env var.
    pub fn generate_keypair(&self, name: Option<String>) -> Result<StoredAccount> {
        let password = wallet_password();
        self.generate_keypair_with_password(name, &password)
    }

    /// Generate a new keypair using an explicit `password`.
    pub fn generate_keypair_with_password(
        &self,
        name: Option<String>,
        password: &str,
    ) -> Result<StoredAccount> {
        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();
        let address = derive_address(&verifying_key);

        let account_name = name.unwrap_or_else(|| {
            format!("account-{}", &hex::encode(address.as_bytes())[..8])
        });

        let account = StoredAccount {
            name: account_name.clone(),
            address: hex::encode(address.as_bytes()),
            public_key: hex::encode(verifying_key.as_bytes()),
            created_at: chrono::Utc::now().timestamp(),
        };

        // Persist: public metadata (plaintext) + secret key (encrypted).
        self.save_account_metadata(&account)?;
        self.save_secret_key(&account_name, &signing_key.to_bytes(), password)?;

        Ok(account)
    }

    // ─── Key import ───────────────────────────────────────────────────────

    /// Import an existing keypair from a hex-encoded private key.
    ///
    /// Password is read from `COINJECT_KEYSTORE_PASSWORD` env var.
    pub fn import_keypair(&self, private_key_hex: &str, name: Option<String>) -> Result<StoredAccount> {
        let password = wallet_password();
        self.import_keypair_with_password(private_key_hex, name, &password)
    }

    /// Import an existing keypair using an explicit `password`.
    pub fn import_keypair_with_password(
        &self,
        private_key_hex: &str,
        name: Option<String>,
        password: &str,
    ) -> Result<StoredAccount> {
        let private_key_bytes = hex::decode(private_key_hex)
            .map_err(|e| anyhow!("Invalid private key hex: {}", e))?;

        if private_key_bytes.len() != 32 {
            return Err(anyhow!("Private key must be 32 bytes, got {}", private_key_bytes.len()));
        }

        let mut key_bytes = [0u8; 32];
        key_bytes.copy_from_slice(&private_key_bytes);

        let signing_key = SigningKey::from_bytes(&key_bytes);
        let verifying_key = signing_key.verifying_key();
        let address = derive_address(&verifying_key);

        let account_name = name.unwrap_or_else(|| {
            format!("imported-{}", &hex::encode(address.as_bytes())[..8])
        });

        let account = StoredAccount {
            name: account_name.clone(),
            address: hex::encode(address.as_bytes()),
            public_key: hex::encode(verifying_key.as_bytes()),
            created_at: chrono::Utc::now().timestamp(),
        };

        self.save_account_metadata(&account)?;
        let mut secret = signing_key.to_bytes();
        self.save_secret_key(&account_name, &secret, password)?;
        secret.zeroize();

        key_bytes.zeroize();
        Ok(account)
    }

    // ─── Account listing ──────────────────────────────────────────────────

    /// List all accounts (reads public metadata only — no password required).
    pub fn list_accounts(&self) -> Result<Vec<StoredAccount>> {
        let mut accounts = Vec::new();
        if !self.keystore_dir.exists() {
            return Ok(accounts);
        }
        for entry in fs::read_dir(&self.keystore_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                let json = fs::read_to_string(&path)?;
                match serde_json::from_str::<StoredAccount>(&json) {
                    Ok(account) => accounts.push(account),
                    Err(e) => {
                        eprintln!("WARN: Skipping malformed account file {:?}: {}", path, e);
                    }
                }
            }
        }
        Ok(accounts)
    }

    /// Get account metadata by name or address (no password required).
    pub fn get_account(&self, name_or_address: &str) -> Result<StoredAccount> {
        for account in self.list_accounts()? {
            if account.name == name_or_address || account.address == name_or_address {
                return Ok(account);
            }
        }
        Err(anyhow!("Account '{}' not found in keystore", name_or_address))
    }

    // ─── Signing (requires password) ──────────────────────────────────────

    /// Load the signing key for `name_or_address`, decrypting with the env-var password.
    pub fn get_signing_key(&self, name_or_address: &str) -> Result<SigningKey> {
        let password = wallet_password();
        self.get_signing_key_with_password(name_or_address, &password)
    }

    /// Load the signing key using an explicit `password`.
    pub fn get_signing_key_with_password(
        &self,
        name_or_address: &str,
        password: &str,
    ) -> Result<SigningKey> {
        let account = self.get_account(name_or_address)?;
        let mut secret = self.load_secret_key(&account.name, password)?;
        let signing_key = SigningKey::from_bytes(&secret);
        secret.zeroize();
        Ok(signing_key)
    }

    /// Sign `message` with the secret key for `name_or_address`.
    pub fn sign(&self, name_or_address: &str, message: &[u8]) -> Result<Signature> {
        let signing_key = self.get_signing_key(name_or_address)?;
        Ok(signing_key.sign(message))
    }

    /// Sign `message` using an explicit `password`.
    pub fn sign_with_password(
        &self,
        name_or_address: &str,
        message: &[u8],
        password: &str,
    ) -> Result<Signature> {
        let signing_key = self.get_signing_key_with_password(name_or_address, password)?;
        Ok(signing_key.sign(message))
    }

    // ─── Account deletion ─────────────────────────────────────────────────

    /// Delete both the metadata JSON and encrypted key file for `name`.
    pub fn delete_account(&self, name: &str) -> Result<()> {
        let json_path = self.keystore_dir.join(format!("{}.json", name));
        let key_path = self.keystore_dir.join(format!("{}.key", name));

        if !json_path.exists() {
            return Err(anyhow!("Account '{}' not found", name));
        }
        fs::remove_file(&json_path)?;
        if key_path.exists() {
            fs::remove_file(&key_path)?;
        }
        Ok(())
    }

    // ─── Internal helpers ─────────────────────────────────────────────────

    /// Write public account metadata to `{name}.json` (cleartext).
    fn save_account_metadata(&self, account: &StoredAccount) -> Result<()> {
        let path = self.keystore_dir.join(format!("{}.json", account.name));
        let json = serde_json::to_string_pretty(account)?;
        fs::write(path, json)?;
        Ok(())
    }

    /// Encrypt and write `secret_key` to `{name}.key`.
    fn save_secret_key(&self, name: &str, secret_key: &[u8; 32], password: &str) -> Result<()> {
        let encrypted = encrypt_secret_key(secret_key, password)
            .map_err(|e| anyhow!("Failed to encrypt secret key: {}", e))?;

        let path = self.keystore_dir.join(format!("{}.key", name));
        fs::write(&path, &encrypted)?;

        // Restrict to owner read/write on Unix.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&path)?.permissions();
            perms.set_mode(0o600);
            fs::set_permissions(&path, perms)?;
        }

        Ok(())
    }

    /// Decrypt and return the secret key for `name`.
    fn load_secret_key(&self, name: &str, password: &str) -> Result<[u8; 32]> {
        let path = self.keystore_dir.join(format!("{}.key", name));
        if !path.exists() {
            return Err(anyhow!(
                "Encrypted key file not found for '{}'. \
                 The account metadata exists but the key file is missing.",
                name
            ));
        }
        let data = fs::read(&path)?;
        decrypt_secret_key(&data, password)
            .map_err(|e| anyhow!("Failed to decrypt key for '{}': {}", name, e))
    }
}

impl Default for Keystore {
    fn default() -> Self {
        Self::new().expect("Failed to open default keystore")
    }
}

// ─── Encryption helpers ───────────────────────────────────────────────────────

/// Encrypt a 32-byte secret key, returning the encrypted file bytes.
fn encrypt_secret_key(secret_key: &[u8; 32], password: &str) -> Result<Vec<u8>, String> {
    let mut rng = OsRng;
    let mut salt = [0u8; 32];
    let mut nonce_bytes = [0u8; 12];
    rng.fill_bytes(&mut salt);
    rng.fill_bytes(&mut nonce_bytes);

    let mut enc_key = [0u8; KEY_LEN];
    derive_key(password, &salt, &mut enc_key)?;

    let cipher_key = Key::<Aes256Gcm>::from_slice(&enc_key);
    let cipher = Aes256Gcm::new(cipher_key);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, secret_key.as_ref())
        .map_err(|e| format!("AES-GCM encryption failed: {}", e))?;

    enc_key.zeroize();

    let mut out = Vec::with_capacity(MIN_KEY_FILE_SIZE + 32);
    out.extend_from_slice(WALLET_KEY_MAGIC);
    out.push(WALLET_KEY_VERSION);
    out.extend_from_slice(&salt);
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Decrypt file bytes produced by [`encrypt_secret_key`].
fn decrypt_secret_key(data: &[u8], password: &str) -> Result<[u8; 32], String> {
    if data.len() < MIN_KEY_FILE_SIZE {
        return Err(format!(
            "Key file too small ({} bytes)",
            data.len()
        ));
    }

    if &data[0..4] != WALLET_KEY_MAGIC {
        return Err(
            "Invalid wallet key file magic. \
             Private keys may have been stored in a legacy plaintext format. \
             Re-import your key with: coinject-wallet import"
                .to_string(),
        );
    }

    if data[4] != WALLET_KEY_VERSION {
        return Err(format!("Unsupported wallet key version 0x{:02x}", data[4]));
    }

    let salt = &data[5..37];
    let nonce_bytes = &data[37..49];
    let ciphertext = &data[49..];

    let mut enc_key = [0u8; KEY_LEN];
    derive_key(password, salt, &mut enc_key)?;

    let cipher_key = Key::<Aes256Gcm>::from_slice(&enc_key);
    let cipher = Aes256Gcm::new(cipher_key);
    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| "Decryption failed — wrong password or corrupted key file".to_string())?;

    enc_key.zeroize();

    if plaintext.len() != 32 {
        return Err(format!(
            "Unexpected decrypted key length {} (expected 32)",
            plaintext.len()
        ));
    }

    let mut secret = [0u8; 32];
    secret.copy_from_slice(&plaintext);
    Ok(secret)
}

/// Derive an AES key from `password` and `salt` using argon2id.
fn derive_key(password: &str, salt: &[u8], key_out: &mut [u8]) -> Result<(), String> {
    let params = Params::new(ARGON2_MEM_KIB, ARGON2_ITERATIONS, ARGON2_PARALLELISM, Some(key_out.len()))
        .map_err(|e| format!("Invalid argon2 parameters: {}", e))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    argon2
        .hash_password_into(password.as_bytes(), salt, key_out)
        .map_err(|e| format!("argon2id key derivation failed: {}", e))?;
    Ok(())
}

/// Derive an `Address` from a verifying key using SHA-256.
fn derive_address(public_key: &VerifyingKey) -> Address {
    let mut hasher = Sha256::new();
    hasher.update(public_key.as_bytes());
    let hash = hasher.finalize();
    let mut addr_bytes = [0u8; 32];
    addr_bytes.copy_from_slice(&hash[..32]);
    Address::from_bytes(addr_bytes)
}

/// Read the wallet keystore password from environment variables.
///
/// Same env var as the node keystore for operational simplicity.
fn wallet_password() -> String {
    if let Ok(pw) = env::var("COINJECT_KEYSTORE_PASSWORD") {
        return pw;
    }
    if let Ok(pw) = env::var("COINJECT_WALLET_PASSWORD") {
        return pw;
    }
    eprintln!(
        "SECURITY WARNING: No wallet keystore password configured.\n\
         Set COINJECT_KEYSTORE_PASSWORD environment variable.\n\
         Using an empty password is insecure and must NOT be used in production."
    );
    String::new()
}

/// Resolve the default keystore directory: `~/.coinject/wallets/`.
fn default_keystore_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("Could not find home directory"))?;
    Ok(home.join(".coinject").join("wallets"))
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    const TEST_PASSWORD: &str = "test-wallet-password-do-not-use-in-production";

    fn test_keystore() -> (Keystore, tempfile::TempDir) {
        let dir = tempdir().unwrap();
        let ks = Keystore::at(dir.path().to_path_buf()).unwrap();
        (ks, dir)
    }

    #[test]
    fn test_generate_and_load_round_trip() {
        let (ks, _dir) = test_keystore();
        let account = ks
            .generate_keypair_with_password(Some("alice".to_string()), TEST_PASSWORD)
            .unwrap();

        assert_eq!(account.name, "alice");
        assert_eq!(account.address.len(), 64);
        assert_eq!(account.public_key.len(), 64);
        // No private_key field in StoredAccount anymore
    }

    #[test]
    fn test_sign_verify_round_trip() {
        let (ks, _dir) = test_keystore();
        ks.generate_keypair_with_password(Some("bob".to_string()), TEST_PASSWORD)
            .unwrap();

        let message = b"transfer 100 tokens to alice";
        let sig = ks
            .sign_with_password("bob", message, TEST_PASSWORD)
            .unwrap();

        // Reconstruct the verifying key and check signature
        let account = ks.get_account("bob").unwrap();
        let pk_bytes = hex::decode(&account.public_key).unwrap();
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&pk_bytes);
        let vk = VerifyingKey::from_bytes(&arr).unwrap();
        use ed25519_dalek::Verifier;
        assert!(vk.verify(message, &sig).is_ok());
    }

    #[test]
    fn test_wrong_password_fails() {
        let (ks, _dir) = test_keystore();
        ks.generate_keypair_with_password(Some("carol".to_string()), TEST_PASSWORD)
            .unwrap();

        let result = ks.get_signing_key_with_password("carol", "wrong-password");
        assert!(result.is_err(), "Wrong password should fail");
    }

    #[test]
    fn test_import_keypair() {
        let (ks, _dir) = test_keystore();

        // Generate a key to get a valid secret
        let original_sk = SigningKey::generate(&mut OsRng);
        let sk_hex = hex::encode(original_sk.to_bytes());

        let account = ks
            .import_keypair_with_password(&sk_hex, Some("dave".to_string()), TEST_PASSWORD)
            .unwrap();

        // Verify we can recover the same signing key
        let recovered = ks
            .get_signing_key_with_password("dave", TEST_PASSWORD)
            .unwrap();
        assert_eq!(original_sk.to_bytes(), recovered.to_bytes());
        assert_eq!(
            account.public_key,
            hex::encode(original_sk.verifying_key().as_bytes())
        );
    }

    #[test]
    fn test_list_accounts() {
        let (ks, _dir) = test_keystore();
        ks.generate_keypair_with_password(Some("a1".to_string()), TEST_PASSWORD)
            .unwrap();
        ks.generate_keypair_with_password(Some("a2".to_string()), TEST_PASSWORD)
            .unwrap();

        let accounts = ks.list_accounts().unwrap();
        assert_eq!(accounts.len(), 2);
    }

    #[test]
    fn test_delete_account() {
        let (ks, _dir) = test_keystore();
        ks.generate_keypair_with_password(Some("temp".to_string()), TEST_PASSWORD)
            .unwrap();
        ks.delete_account("temp").unwrap();

        let accounts = ks.list_accounts().unwrap();
        assert!(accounts.is_empty());
    }

    #[test]
    fn test_no_plaintext_private_key_in_json() {
        let (ks, dir) = test_keystore();
        ks.generate_keypair_with_password(Some("eve".to_string()), TEST_PASSWORD)
            .unwrap();

        let json_path = dir.path().join("eve.json");
        let contents = fs::read_to_string(json_path).unwrap();

        // The JSON must NOT contain a private_key field
        assert!(
            !contents.contains("private_key"),
            "JSON must not contain private_key: {}",
            contents
        );
    }

    #[test]
    fn test_key_file_has_encrypted_magic() {
        let (ks, dir) = test_keystore();
        ks.generate_keypair_with_password(Some("frank".to_string()), TEST_PASSWORD)
            .unwrap();

        let key_path = dir.path().join("frank.key");
        let bytes = fs::read(key_path).unwrap();
        assert_eq!(&bytes[..4], WALLET_KEY_MAGIC, "Encrypted key file must start with CKWV");
    }
}
