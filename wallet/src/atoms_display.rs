//! Display ledger **atoms** as **BEANS** using the same fixed-point as chain rewards.

use coinject_core::validation::{ATOMS_PER_DISPLAY_BEAN, MIN_FEE_BOUNTY_SUBMISSION};

/// Default fee (atoms) for CLI-built transfers, time-locks, escrows, channels, etc.
/// Matches mempool default `min_fee` / bounty submission floor.
pub const DEFAULT_PAID_TX_FEE_ATOMS: u128 = MIN_FEE_BOUNTY_SUBMISSION;

fn add_thousands_sep_uint(digits: String) -> String {
    let mut result = String::new();
    for (count, c) in digits.chars().rev().enumerate() {
        if count > 0 && count % 3 == 0 {
            result.insert(0, ',');
        }
        result.insert(0, c);
    }
    result
}

/// Format smallest-unit balance as display BEANS (trim fractional zeros).
pub fn format_atoms_as_beans(atoms: u128) -> String {
    let whole = atoms / ATOMS_PER_DISPLAY_BEAN;
    let frac = atoms % ATOMS_PER_DISPLAY_BEAN;
    if frac == 0 {
        return add_thousands_sep_uint(whole.to_string());
    }
    let mut frac_str = format!("{:012}", frac);
    while frac_str.ends_with('0') {
        frac_str.pop();
    }
    format!("{}.{frac_str}", add_thousands_sep_uint(whole.to_string()))
}
