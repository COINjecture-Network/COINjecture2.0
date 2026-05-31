//! JSON deserialization for ledger atom amounts (`Balance` / `u128`).
//!
//! Browser clients send large bounties as decimal strings (JS `Number` cannot represent
//! 10^12-scale atoms exactly). Accept string or integer JSON numbers.

use coinject_core::Balance;
use serde::de::{self, Deserializer, Visitor};
use std::fmt;

pub fn deserialize_atoms_balance<'de, D>(deserializer: D) -> Result<Balance, D::Error>
where
    D: Deserializer<'de>,
{
    struct AtomsVisitor;

    impl<'de> Visitor<'de> for AtomsVisitor {
        type Value = Balance;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a non-negative integer or decimal string (ledger atoms)")
        }

        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(value as Balance)
        }

        fn visit_u128<E>(self, value: u128) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(value)
        }

        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            if value < 0 {
                return Err(E::custom("balance must be non-negative"));
            }
            Ok(value as Balance)
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            let trimmed = value.trim();
            trimmed
                .parse::<Balance>()
                .map_err(|e| E::custom(format!("invalid balance string: {e}")))
        }

        fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            self.visit_str(&value)
        }
    }

    deserializer.deserialize_any(AtomsVisitor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Params {
        #[serde(deserialize_with = "deserialize_atoms_balance")]
        bounty: Balance,
    }

    #[test]
    fn accepts_decimal_string_atoms() {
        let p: Params = serde_json::from_str(r#"{"bounty":"50000000000000000"}"#).unwrap();
        assert_eq!(p.bounty, 50_000_000_000_000_000);
    }

    #[test]
    fn accepts_u64_json_number() {
        let p: Params = serde_json::from_str(r#"{"bounty":1000}"#).unwrap();
        assert_eq!(p.bounty, 1000);
    }

    /// Without `deserialize_atoms_balance`, quoted bounty strings fail serde_json u128 parsing.
    #[test]
    fn raw_u128_rejects_quoted_bounty_string() {
        #[derive(Debug, Deserialize)]
        struct RawBounty {
            bounty: Balance,
        }
        let err = serde_json::from_str::<RawBounty>(r#"{"bounty":"50000000000000000"}"#).unwrap_err();
        assert!(err.to_string().contains("invalid"));
    }
}
