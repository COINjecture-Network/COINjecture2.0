// Exponential Dimensional Distribution
// η = 1/√2 = 0.7071067811865476

use serde::{Deserialize, Serialize};

/// The critical constant η = 1/√2
pub const ETA: f64 = 0.7071067811865476;

/// Exponential dimension with lock period
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Dimension {
    pub index: u8,
    pub scale: f64,          // D_n = e^(-ηt_n)
    pub allocation: f64,     // Normalized percentage
    pub lock_days: u64,      // Lock period in days
    pub function: String,    // Economic function
}

impl Dimension {
    /// Calculate dimension scale: D_n = e^(-ηt_n)
    pub fn calculate_scale(n: u8) -> f64 {
        let t_n = n as f64;
        (-ETA * t_n).exp()
    }

    /// Get all 8 dimensions
    pub fn all() -> Vec<Dimension> {
        vec![
            Dimension {
                index: 1,
                scale: 1.000,
                allocation: 56.1,
                lock_days: 0,
                function: "Immediate liquidity (block rewards)".to_string(),
            },
            Dimension {
                index: 2,
                scale: 0.867,
                allocation: 48.6,
                lock_days: 7,
                function: "Short-term staking".to_string(),
            },
            Dimension {
                index: 3,
                scale: 0.750,
                allocation: 42.1,
                lock_days: 14,
                function: "Problem bounty pool".to_string(),
            },
            Dimension {
                index: 4,
                scale: 0.618,
                allocation: 34.7,
                lock_days: 24,
                function: "Treasury reserve".to_string(),
            },
            Dimension {
                index: 5,
                scale: 0.500,
                allocation: 28.1,
                lock_days: 35,
                function: "Development fund".to_string(),
            },
            Dimension {
                index: 6,
                scale: 0.382,
                allocation: 21.4,
                lock_days: 48,
                function: "Long-term vesting".to_string(),
            },
            Dimension {
                index: 7,
                scale: 0.250,
                allocation: 14.0,
                lock_days: 69,
                function: "Strategic reserve".to_string(),
            },
            Dimension {
                index: 8,
                scale: 0.146,
                allocation: 8.2,
                lock_days: 96,
                function: "Foundation endowment".to_string(),
            },
        ]
    }

    /// Calculate unlock curve: U_n(t) = 1 - e^(-η(t-t_n)) for t ≥ t_n
    pub fn unlock_at_time(&self, elapsed_days: u64) -> f64 {
        if elapsed_days < self.lock_days {
            return 0.0;
        }
        let delta = (elapsed_days - self.lock_days) as f64;
        1.0 - (-ETA * delta).exp()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eta_constant() {
        assert!((ETA - (2.0_f64.sqrt().recip())).abs() < 1e-10);
    }

    #[test]
    fn test_dimension_scales() {
        let dims = Dimension::all();
        assert_eq!(dims.len(), 8);
        assert_eq!(dims[0].scale, 1.000);
        assert!(dims[3].scale > 0.6 && dims[3].scale < 0.7); // Golden ratio region
    }

    #[test]
    fn test_unlock_curve() {
        let d1 = &Dimension::all()[0];
        assert_eq!(d1.unlock_at_time(0), 0.0); // Locked
        assert!(d1.unlock_at_time(1) > 0.0); // Unlocking
    }
}
