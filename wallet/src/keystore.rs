// Keystore for managing Ed25519 keypairs
// Stores encrypted keys in ~/.coinject/wallets/

use anyhow::{anyhow, Result};
use coinject_core::Address;
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;

/// Account stored in keystore
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredAccount {
    pub name: String,
    pub address: String,
    pub public_key: String,
    pub private_key: String, // TODO: Encrypt this in production
    pub created_at: i64,
}

/// Keystore manager
pub struct Keystore {
    keystore_dir: PathBuf,
}

impl Keystore {
    /// Create a new keystore
    pub fn new() -> Result<Self> {
        let keystore_dir = Self::get_keystore_dir()?;
        fs::create_dir_all(&keystore_dir)?;

        Ok(Keystore { keystore_dir })
    }

    /// Get the keystore directory path
    fn get_keystore_dir() -> Result<PathBuf> {
        let home = dirs::home_dir().ok_or_else(|| anyhow!("Could not find home directory"))?;
        Ok(home.join(".coinject").join("wallets"))
    }

    /// Generate a new keypair
    pub fn generate_keypair(&self, name: Option<String>) -> Result<StoredAccount> {
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        let verifying_key = signing_key.verifying_key();

        // Derive address from public key (use SHA256 of public key)
        let address = Self::derive_address(&verifying_key);

        let account_name = name.unwrap_or_else(|| {
            format!("account-{}", &hex::encode(&address.as_bytes())[0..8])
        });

        let account = StoredAccount {
            name: account_name.clone(),
            address: hex::encode(address.as_bytes()),
            public_key: hex::encode(verifying_key.as_bytes()),
            private_key: hex::encode(signing_key.to_bytes()),
            created_at: chrono::Utc::now().timestamp(),
        };

        // Save to disk
        self.save_account(&account)?;

        Ok(account)
    }

    /// Derive an address from a public key
    fn derive_address(public_key: &VerifyingKey) -> Address {
        let mut hasher = Sha256::new();
        hasher.update(public_key.as_bytes());
        let hash = hasher.finalize();

        let mut addr_bytes = [0u8; 32];
        addr_bytes.copy_from_slice(&hash[..32]);
        Address::from_bytes(addr_bytes)
    }

    /// Import a keypair from private key
    pub fn import_keypair(&self, private_key_hex: &str, name: Option<String>) -> Result<StoredAccount> {
        let private_key_bytes = hex::decode(private_key_hex)
            .map_err(|e| anyhow!("Invalid private key hex: {}", e))?;

        if private_key_bytes.len() != 32 {
            return Err(anyhow!("Private key must be 32 bytes"));
        }

        let mut key_bytes = [0u8; 32];
        key_bytes.copy_from_slice(&private_key_bytes);

        let signing_key = SigningKey::from_bytes(&key_bytes);
        let verifying_key = signing_key.verifying_key();
        let address = Self::derive_address(&verifying_key);

        let account_name = name.unwrap_or_else(|| {
            format!("imported-{}", &hex::encode(&address.as_bytes())[0..8])
        });

        let account = StoredAccount {
            name: account_name.clone(),
            address: hex::encode(address.as_bytes()),
            public_key: hex::encode(verifying_key.as_bytes()),
            private_key: hex::encode(signing_key.to_bytes()),
            created_at: chrono::Utc::now().timestamp(),
        };

        // Save to disk
        self.save_account(&account)?;

        Ok(account)
    }

    /// Save account to disk
    fn save_account(&self, account: &StoredAccount) -> Result<()> {
        let filename = format!("{}.json", account.name);
        let path = self.keystore_dir.join(filename);

        let json = serde_json::to_string_pretty(account)?;
        fs::write(path, json)?;

        Ok(())
    }

    /// List all accounts in keystore
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
                let account: StoredAccount = serde_json::from_str(&json)?;
                accounts.push(account);
            }
        }

        Ok(accounts)
    }

    /// Get account by name or address
    pub fn get_account(&self, name_or_address: &str) -> Result<StoredAccount> {
        let accounts = self.list_accounts()?;

        for account in accounts {
            if account.name == name_or_address || account.address == name_or_address {
                return Ok(account);
            }
        }

        Err(anyhow!("Account '{}' not found in keystore", name_or_address))
    }

    /// Get signing key for an account
    pub fn get_signing_key(&self, name_or_address: &str) -> Result<SigningKey> {
        let account = self.get_account(name_or_address)?;
        let private_key_bytes = hex::decode(&account.private_key)?;

        let mut key_bytes = [0u8; 32];
        key_bytes.copy_from_slice(&private_key_bytes);

        Ok(SigningKey::from_bytes(&key_bytes))
    }

    /// Sign a message with an account's private key
    pub fn sign(&self, name_or_address: &str, message: &[u8]) -> Result<Signature> {
        let signing_key = self.get_signing_key(name_or_address)?;
        Ok(signing_key.sign(message))
    }

    /// Delete an account from keystore
    pub fn delete_account(&self, name: &str) -> Result<()> {
        let filename = format!("{}.json", name);
        let path = self.keystore_dir.join(filename);

        if !path.exists() {
            return Err(anyhow!("Account '{}' not found", name));
        }

        fs::remove_file(path)?;
        Ok(())
    }
}

impl Default for Keystore {
    fn default() -> Self {
        Self::new().expect("Failed to create keystore")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keypair_generation() {
        let keystore = Keystore::new().unwrap();
        let account = keystore.generate_keypair(Some("test-account".to_string())).unwrap();

        assert_eq!(account.name, "test-account");
        assert_eq!(account.address.len(), 64); // 32 bytes in hex
        assert_eq!(account.public_key.len(), 64);
        assert_eq!(account.private_key.len(), 64);

        // Clean up
        let _ = keystore.delete_account("test-account");
    }

    #[test]
    fn test_import_keypair() {
        let keystore = Keystore::new().unwrap();

        // Generate a keypair to get a valid private key
        let original = keystore.generate_keypair(Some("original".to_string())).unwrap();
        let private_key = original.private_key.clone();

        // Import it with a new name
        let imported = keystore.import_keypair(&private_key, Some("imported".to_string())).unwrap();

        // Should have same address and keys
        assert_eq!(imported.address, original.address);
        assert_eq!(imported.public_key, original.public_key);
        assert_eq!(imported.private_key, original.private_key);

        // Clean up
        let _ = keystore.delete_account("original");
        let _ = keystore.delete_account("imported");
    }
}
