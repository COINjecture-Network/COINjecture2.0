// Testnet Faucet
// Rate-limited token distribution for testing

use coinject_core::Address;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// Faucet state tracker
#[derive(Clone)]
pub struct Faucet {
    /// Faucet configuration
    config: FaucetConfig,
    /// Last request time per address
    last_request: Arc<Mutex<HashMap<String, u64>>>,
}

#[derive(Clone, Debug)]
pub struct FaucetConfig {
    /// Whether faucet is enabled
    pub enabled: bool,
    /// Amount to distribute per request
    pub amount: u128,
    /// Cooldown period in seconds
    pub cooldown: u64,
}

impl Faucet {
    /// Create a new faucet
    pub fn new(config: FaucetConfig) -> Self {
        Faucet {
            config,
            last_request: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Check if faucet is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Request tokens from faucet
    pub fn request_tokens(&self, address: &Address) -> Result<u128, FaucetError> {
        if !self.config.enabled {
            return Err(FaucetError::Disabled);
        }

        let addr_str = hex::encode(address.as_bytes());
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut last_req = self.last_request.lock().unwrap();

        // Check if address has requested before
        if let Some(&last_time) = last_req.get(&addr_str) {
            let elapsed = now - last_time;
            if elapsed < self.config.cooldown {
                let remaining = self.config.cooldown - elapsed;
                return Err(FaucetError::Cooldown {
                    remaining_seconds: remaining,
                });
            }
        }

        // Update last request time
        last_req.insert(addr_str, now);

        Ok(self.config.amount)
    }

    /// Get faucet configuration
    pub fn config(&self) -> &FaucetConfig {
        &self.config
    }

    /// Get remaining cooldown for an address (in seconds)
    pub fn get_remaining_cooldown(&self, address: &Address) -> Option<u64> {
        let addr_str = hex::encode(address.as_bytes());
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let last_req = self.last_request.lock().unwrap();

        if let Some(&last_time) = last_req.get(&addr_str) {
            let elapsed = now - last_time;
            if elapsed < self.config.cooldown {
                return Some(self.config.cooldown - elapsed);
            }
        }

        None
    }
}

#[derive(Debug)]
pub enum FaucetError {
    /// Faucet is disabled
    Disabled,
    /// Address is on cooldown
    Cooldown { remaining_seconds: u64 },
}

impl std::fmt::Display for FaucetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FaucetError::Disabled => write!(f, "Faucet is not enabled on this node"),
            FaucetError::Cooldown { remaining_seconds } => {
                write!(
                    f,
                    "Faucet cooldown active. Try again in {} seconds",
                    remaining_seconds
                )
            }
        }
    }
}

impl std::error::Error for FaucetError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_faucet_disabled() {
        let config = FaucetConfig {
            enabled: false,
            amount: 10000,
            cooldown: 60,
        };
        let faucet = Faucet::new(config);

        let addr = Address::from_bytes([1u8; 32]);
        let result = faucet.request_tokens(&addr);

        assert!(matches!(result, Err(FaucetError::Disabled)));
    }

    #[test]
    fn test_faucet_success() {
        let config = FaucetConfig {
            enabled: true,
            amount: 10000,
            cooldown: 60,
        };
        let faucet = Faucet::new(config);

        let addr = Address::from_bytes([1u8; 32]);
        let result = faucet.request_tokens(&addr);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 10000);
    }

    #[test]
    fn test_faucet_cooldown() {
        let config = FaucetConfig {
            enabled: true,
            amount: 10000,
            cooldown: 60,
        };
        let faucet = Faucet::new(config);

        let addr = Address::from_bytes([1u8; 32]);

        // First request should succeed
        let result1 = faucet.request_tokens(&addr);
        assert!(result1.is_ok());

        // Second immediate request should fail
        let result2 = faucet.request_tokens(&addr);
        assert!(matches!(result2, Err(FaucetError::Cooldown { .. })));
    }

    #[test]
    fn test_faucet_different_addresses() {
        let config = FaucetConfig {
            enabled: true,
            amount: 10000,
            cooldown: 60,
        };
        let faucet = Faucet::new(config);

        let addr1 = Address::from_bytes([1u8; 32]);
        let addr2 = Address::from_bytes([2u8; 32]);

        // Both addresses should be able to request
        let result1 = faucet.request_tokens(&addr1);
        let result2 = faucet.request_tokens(&addr2);

        assert!(result1.is_ok());
        assert!(result2.is_ok());
    }
}
