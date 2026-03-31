//! Sign-In With BEANS (SIWB) — CAIP-122-style challenge message.
//!
//! Provides construction, serialisation, and parsing of SIWB challenge
//! messages used in the wallet authentication flow.

use chrono::{DateTime, Utc};
use std::fmt;

/// A structured SIWB challenge message.
pub struct SiwbMessage {
    pub domain: String,
    pub wallet_address: String,
    pub statement: String,
    pub uri: String,
    pub version: String,
    pub chain_id: String,
    pub nonce: String,
    pub issued_at: DateTime<Utc>,
    pub expiration_time: DateTime<Utc>,
}

#[derive(Debug)]
pub enum SiwbError {
    ParseError(String),
    Expired,
    InvalidFormat(String),
}

impl fmt::Display for SiwbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ParseError(msg) => write!(f, "Parse error: {msg}"),
            Self::Expired => write!(f, "Message expired"),
            Self::InvalidFormat(msg) => write!(f, "Invalid format: {msg}"),
        }
    }
}

impl SiwbMessage {
    /// Construct a new challenge message with a 300 s TTL.
    pub fn new(wallet_address: &str, nonce: &str, network: &str) -> Self {
        let now = Utc::now();
        Self {
            domain: "coinjecture.com".into(),
            wallet_address: wallet_address.into(),
            statement: format!("Sign in to COINjecture {network}"),
            uri: "https://coinjecture.com".into(),
            version: "1".into(),
            chain_id: format!("coinjecture:{network}"),
            nonce: nonce.into(),
            issued_at: now,
            expiration_time: now + chrono::Duration::seconds(300),
        }
    }

    /// Serialize to the human-readable CAIP-122 string format.
    pub fn to_message_string(&self) -> String {
        format!(
            "{domain} wants you to sign in with your COINjecture account:\n\
             {address}\n\
             \n\
             {statement}\n\
             \n\
             URI: {uri}\n\
             Version: {version}\n\
             Chain ID: {chain_id}\n\
             Nonce: {nonce}\n\
             Issued At: {issued_at}\n\
             Expiration Time: {expiration_time}",
            domain = self.domain,
            address = self.wallet_address,
            statement = self.statement,
            uri = self.uri,
            version = self.version,
            chain_id = self.chain_id,
            nonce = self.nonce,
            issued_at = self.issued_at.to_rfc3339(),
            expiration_time = self.expiration_time.to_rfc3339(),
        )
    }

    /// Parse a message string back into structured fields.
    pub fn from_message_string(message: &str) -> Result<Self, SiwbError> {
        let lines: Vec<&str> = message.lines().collect();

        // Line 0: "{domain} wants you to sign in with your COINjecture account:"
        let domain = lines
            .first()
            .and_then(|l| l.split(" wants you to sign in").next())
            .ok_or_else(|| SiwbError::ParseError("Missing domain line".into()))?
            .to_string();

        // Line 1: wallet address
        let wallet_address = lines
            .get(1)
            .ok_or_else(|| SiwbError::ParseError("Missing wallet address".into()))?
            .to_string();

        // Line 3: statement (line 2 is blank)
        let statement = lines
            .get(3)
            .ok_or_else(|| SiwbError::ParseError("Missing statement".into()))?
            .to_string();

        // Parse key-value fields from remaining lines
        fn extract_field<'a>(lines: &[&'a str], prefix: &str) -> Option<&'a str> {
            lines
                .iter()
                .find_map(|l| l.strip_prefix(prefix))
                .map(|s| s.trim())
        }

        let uri = extract_field(&lines, "URI: ")
            .ok_or_else(|| SiwbError::ParseError("Missing URI".into()))?
            .to_string();
        let version = extract_field(&lines, "Version: ")
            .ok_or_else(|| SiwbError::ParseError("Missing Version".into()))?
            .to_string();
        let chain_id = extract_field(&lines, "Chain ID: ")
            .ok_or_else(|| SiwbError::ParseError("Missing Chain ID".into()))?
            .to_string();
        let nonce = extract_field(&lines, "Nonce: ")
            .ok_or_else(|| SiwbError::ParseError("Missing Nonce".into()))?
            .to_string();
        let issued_at_str = extract_field(&lines, "Issued At: ")
            .ok_or_else(|| SiwbError::ParseError("Missing Issued At".into()))?;
        let expiration_str = extract_field(&lines, "Expiration Time: ")
            .ok_or_else(|| SiwbError::ParseError("Missing Expiration Time".into()))?;

        let issued_at = DateTime::parse_from_rfc3339(issued_at_str)
            .map_err(|e| SiwbError::ParseError(format!("Invalid Issued At: {e}")))?
            .with_timezone(&Utc);
        let expiration_time = DateTime::parse_from_rfc3339(expiration_str)
            .map_err(|e| SiwbError::ParseError(format!("Invalid Expiration Time: {e}")))?
            .with_timezone(&Utc);

        Ok(Self {
            domain,
            wallet_address,
            statement,
            uri,
            version,
            chain_id,
            nonce,
            issued_at,
            expiration_time,
        })
    }

    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expiration_time
    }

    pub fn validate(&self) -> Result<(), SiwbError> {
        if self.domain != "coinjecture.com" {
            return Err(SiwbError::InvalidFormat("Invalid domain".into()));
        }
        if self.wallet_address.is_empty() {
            return Err(SiwbError::InvalidFormat("Empty wallet address".into()));
        }
        if self.version != "1" {
            return Err(SiwbError::InvalidFormat("Unsupported version".into()));
        }
        if self.is_expired() {
            return Err(SiwbError::Expired);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_message() {
        let msg = SiwbMessage::new("abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234", "test-nonce", "testnet");
        let text = msg.to_message_string();
        let parsed = SiwbMessage::from_message_string(&text).unwrap();

        assert_eq!(parsed.domain, "coinjecture.com");
        assert_eq!(parsed.wallet_address, msg.wallet_address);
        assert_eq!(parsed.nonce, "test-nonce");
        assert_eq!(parsed.chain_id, "coinjecture:testnet");
        assert!(parsed.validate().is_ok());
    }
}
