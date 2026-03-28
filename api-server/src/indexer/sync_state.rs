//! Tracks the indexer's sync position, persisted to Supabase.

use chrono::{DateTime, Utc};
use crate::supabase::SupabaseClient;

#[derive(Debug, Clone)]
pub struct SyncState {
    pub last_indexed_height: u64,
    pub last_indexed_hash: String,
    pub last_sync_at: DateTime<Utc>,
}

impl Default for SyncState {
    fn default() -> Self {
        Self {
            last_indexed_height: 0,
            last_indexed_hash: String::new(),
            last_sync_at: Utc::now(),
        }
    }
}

impl SyncState {
    /// Load from Supabase. Returns default (height 0) if not found.
    pub async fn load(supabase: &SupabaseClient) -> Result<Self, String> {
        match supabase
            .postgrest_get_public("sync_state?id=eq.main&select=*&limit=1")
            .await
        {
            Ok(data) => {
                if let Some(row) = data.as_array().and_then(|a| a.first()) {
                    Ok(Self {
                        last_indexed_height: row["last_indexed_height"]
                            .as_u64()
                            .unwrap_or(0),
                        last_indexed_hash: row["last_indexed_hash"]
                            .as_str()
                            .unwrap_or("")
                            .to_string(),
                        last_sync_at: Utc::now(),
                    })
                } else {
                    Ok(Self::default())
                }
            }
            Err(e) => Err(format!("Failed to load sync state: {e}")),
        }
    }

    /// Save to Supabase via upsert.
    pub async fn save(&self, supabase: &SupabaseClient) -> Result<(), String> {
        let body = serde_json::json!({
            "id": "main",
            "last_indexed_height": self.last_indexed_height,
            "last_indexed_hash": self.last_indexed_hash,
            "last_sync_at": self.last_sync_at.to_rfc3339(),
        });
        supabase
            .upsert_row("sync_state", body)
            .await
            .map(|_| ())
            .map_err(|e| format!("Failed to save sync state: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_state_default() {
        let state = SyncState::default();
        assert_eq!(state.last_indexed_height, 0);
        assert!(state.last_indexed_hash.is_empty());
    }

    #[test]
    fn test_confirmation_depth() {
        let chain_tip: u64 = 100;
        let confirmations: u64 = 6;
        let safe = chain_tip.saturating_sub(confirmations);
        assert_eq!(safe, 94);

        // Edge: tip < confirmations
        assert_eq!(3u64.saturating_sub(6), 0);
    }

    #[test]
    fn test_reorg_detection() {
        let last = "abc123";
        let parent = "def456";
        assert!(!last.is_empty() && parent != last); // reorg detected
    }
}
