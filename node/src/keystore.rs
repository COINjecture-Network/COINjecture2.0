// Validator Keystore
// Manages persistent validator keys for mining rewards
// SEPARATE from user wallet keys

use coinject_core::Address;
use ed25519_dalek::{SigningKey, VerifyingKey, Signer};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Validator key information
#[derive(Serialize, Deserialize, Clone)]
pub struct ValidatorKey {
    /// Validator address (derived from public key)
    pub address: Address,
    /// Public key (32 bytes)
    pub public_key: [u8; 32],
    /// Private key (32 bytes) - SENSITIVE!
    secret_key: [u8; 32],
}

impl ValidatorKey {
    /// Generate a new random validator key
    pub fn generate() -> Self {
        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();

        let public_key = verifying_key.to_bytes();
        let secret_key = signing_key.to_bytes();

        // Derive address from public key hash
        let address_hash = blake3::hash(&public_key);
        let mut address_bytes = [0u8; 32];
        address_bytes.copy_from_slice(address_hash.as_bytes());
        let address = Address::from_bytes(address_bytes);

        ValidatorKey {
            address,
            public_key,
            secret_key,
        }
    }

    /// Load validator key from file
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let data = fs::read(path.as_ref())
            .map_err(|e| format!("Failed to read validator key: {}", e))?;

        bincode::deserialize(&data)
            .map_err(|e| format!("Failed to deserialize validator key: {}", e))
    }

    /// Save validator key to file
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), String> {
        let data = bincode::serialize(self)
            .map_err(|e| format!("Failed to serialize validator key: {}", e))?;

        // Create parent directory if needed
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create key directory: {}", e))?;
        }

        fs::write(path.as_ref(), data)
            .map_err(|e| format!("Failed to write validator key: {}", e))?;

        // Set restrictive permissions on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(path.as_ref())
                .map_err(|e| format!("Failed to get metadata: {}", e))?
                .permissions();
            perms.set_mode(0o600); // Read/write for owner only
            fs::set_permissions(path.as_ref(), perms)
                .map_err(|e| format!("Failed to set permissions: {}", e))?;
        }

        Ok(())
    }

    /// Get the signing key
    pub fn signing_key(&self) -> Result<SigningKey, String> {
        Ok(SigningKey::from_bytes(&self.secret_key))
    }

    /// Get validator address
    pub fn address(&self) -> Address {
        self.address
    }
}

/// Validator keystore manager
pub struct ValidatorKeystore {
    key_file: PathBuf,
}

impl ValidatorKeystore {
    /// Create new keystore manager
    pub fn new(data_dir: &Path) -> Self {
        let key_file = data_dir.join("validator_key.bin");
        ValidatorKeystore { key_file }
    }

    /// Get or create validator key
    /// If key exists, loads it. Otherwise generates and saves a new one.
    pub fn get_or_create_key(&self) -> Result<ValidatorKey, String> {
        if self.key_file.exists() {
            println!("= Loading existing validator key from: {}", self.key_file.display());
            ValidatorKey::load(&self.key_file)
        } else {
            println!("= Generating new validator key...");
            let key = ValidatorKey::generate();
            key.save(&self.key_file)?;
            println!("   Validator address: {}", hex::encode(key.address.as_bytes()));
            println!("   Key saved to: {}", self.key_file.display());
            Ok(key)
        }
    }

    /// Import validator key from hex private key
    pub fn import_key(&self, secret_key_hex: &str) -> Result<ValidatorKey, String> {
        let secret_bytes = hex::decode(secret_key_hex)
            .map_err(|e| format!("Invalid hex: {}", e))?;

        if secret_bytes.len() != 32 {
            return Err(format!("Secret key must be 32 bytes, got {}", secret_bytes.len()));
        }

        let mut secret_key = [0u8; 32];
        secret_key.copy_from_slice(&secret_bytes);

        let signing_key = SigningKey::from_bytes(&secret_key);
        let verifying_key = signing_key.verifying_key();
        let public_key = verifying_key.to_bytes();

        // Derive address
        let address_hash = blake3::hash(&public_key);
        let mut address_bytes = [0u8; 32];
        address_bytes.copy_from_slice(address_hash.as_bytes());
        let address = Address::from_bytes(address_bytes);

        let key = ValidatorKey {
            address,
            public_key,
            secret_key,
        };

        key.save(&self.key_file)?;
        println!(" Validator key imported");
        println!("   Address: {}", hex::encode(address.as_bytes()));

        Ok(key)
    }

    /// Export validator private key (hex)
    pub fn export_key(&self) -> Result<String, String> {
        let key = ValidatorKey::load(&self.key_file)?;
        Ok(hex::encode(key.secret_key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_generate_key() {
        let key = ValidatorKey::generate();
        assert_eq!(key.public_key.len(), 32);
        assert_eq!(key.secret_key.len(), 32);
    }

    #[test]
    fn test_save_load_key() {
        let dir = tempdir().unwrap();
        let key_path = dir.path().join("test_key.bin");

        let key1 = ValidatorKey::generate();
        key1.save(&key_path).unwrap();

        let key2 = ValidatorKey::load(&key_path).unwrap();

        assert_eq!(key1.address, key2.address);
        assert_eq!(key1.public_key, key2.public_key);
        assert_eq!(key1.secret_key, key2.secret_key);
    }

    #[test]
    fn test_get_or_create() {
        let dir = tempdir().unwrap();
        let keystore = ValidatorKeystore::new(dir.path());

        // First call creates
        let key1 = keystore.get_or_create_key().unwrap();

        // Second call loads
        let key2 = keystore.get_or_create_key().unwrap();

        assert_eq!(key1.address, key2.address);
    }
}
