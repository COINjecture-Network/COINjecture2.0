// Account management commands

use crate::keystore::Keystore;
use crate::rpc_client::RpcClient;
use anyhow::Result;
use colored::*;

/// Create a new account
pub async fn new_account(name: Option<String>) -> Result<()> {
    let keystore = Keystore::new()?;
    let account = keystore.generate_keypair(name)?;

    println!("{}", "✅ New Account Created".green().bold());
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Name:        {}", account.name.cyan());
    println!("Address:     {}", account.address);
    println!("Public Key:  {}", account.public_key.dimmed());
    println!();
    println!("{}", "⚠️  IMPORTANT: Keep your private key safe!".yellow().bold());
    println!("Private Key: {}", account.private_key.red());
    println!();
    println!("Account saved to keystore at: ~/.coinject/wallets/");

    Ok(())
}

/// List all accounts in keystore
pub async fn list_accounts() -> Result<()> {
    let keystore = Keystore::new()?;
    let accounts = keystore.list_accounts()?;

    if accounts.is_empty() {
        println!("{}", "No accounts found in keystore".yellow());
        println!("Create a new account with: coinject-wallet account new");
        return Ok(());
    }

    println!("{}", format!("Found {} account(s)", accounts.len()).green().bold());
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();

    for (i, account) in accounts.iter().enumerate() {
        println!("{}. {}", i + 1, account.name.cyan().bold());
        println!("   Address:    {}", account.address);
        println!("   Public Key: {}", account.public_key.dimmed());
        println!("   Created:    {}", format_timestamp(account.created_at));
        println!();
    }

    Ok(())
}

/// Get account balance
pub async fn get_balance(address: &str, client: &RpcClient) -> Result<()> {
    // Normalize address (remove 0x prefix if present)
    let address = address.trim_start_matches("0x");

    println!("{}", "Querying balance...".dimmed());

    match client.get_balance(address).await {
        Ok(balance) => {
            println!();
            println!("{}", "Account Balance".green().bold());
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("Address: {}", address);
            println!("Balance: {} tokens", format_balance(balance));
        }
        Err(e) => {
            println!("{}", format!("❌ Failed to get balance: {}", e).red());
        }
    }

    Ok(())
}

/// Get full account information
pub async fn get_account_info(address: &str, client: &RpcClient) -> Result<()> {
    // Normalize address (remove 0x prefix if present)
    let address = address.trim_start_matches("0x");

    println!("{}", "Querying account information...".dimmed());

    match client.get_account_info(address).await {
        Ok(info) => {
            println!();
            println!("{}", "Account Information".green().bold());
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("Address: {}", info.address);
            println!("Balance: {} tokens", format_balance(info.balance));
            println!("Nonce:   {}", info.nonce);

            // Check if account is in keystore
            let keystore = Keystore::new()?;
            if let Ok(stored) = keystore.get_account(address) {
                println!();
                println!("{}", "Local Keystore Info".dimmed());
                println!("Name:    {}", stored.name.cyan());
                println!("Created: {}", format_timestamp(stored.created_at));
            }
        }
        Err(e) => {
            println!("{}", format!("❌ Failed to get account info: {}", e).red());
        }
    }

    Ok(())
}

/// Export account private key
pub async fn export_account(name_or_address: &str) -> Result<()> {
    let keystore = Keystore::new()?;

    match keystore.get_account(name_or_address) {
        Ok(account) => {
            println!("{}", "⚠️  EXPORTING PRIVATE KEY".red().bold());
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!();
            println!("Name:        {}", account.name.cyan());
            println!("Address:     {}", account.address);
            println!();
            println!("{}", "🔐 Private Key (keep this secret!):".yellow().bold());
            println!("{}", account.private_key.red());
            println!();
            println!("{}", "⚠️  Never share this key with anyone!".yellow().bold());
        }
        Err(e) => {
            println!("{}", format!("❌ Account not found: {}", e).red());
            println!();
            println!("Available accounts:");
            list_accounts().await?;
        }
    }

    Ok(())
}

/// Import account from private key
pub async fn import_account(private_key: &str, name: Option<String>) -> Result<()> {
    let keystore = Keystore::new()?;

    match keystore.import_keypair(private_key, name) {
        Ok(account) => {
            println!("{}", "✅ Account Imported Successfully".green().bold());
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("Name:       {}", account.name.cyan());
            println!("Address:    {}", account.address);
            println!("Public Key: {}", account.public_key.dimmed());
            println!();
            println!("Account saved to keystore.");
        }
        Err(e) => {
            println!("{}", format!("❌ Failed to import account: {}", e).red());
        }
    }

    Ok(())
}

// Helper functions

fn format_timestamp(timestamp: i64) -> String {
    use chrono::{DateTime, Utc};
    let dt = DateTime::<Utc>::from_timestamp(timestamp, 0)
        .unwrap_or_else(|| Utc::now());
    dt.format("%Y-%m-%d %H:%M:%S UTC").to_string()
}

fn format_balance(balance: u128) -> String {
    // Format with thousand separators
    let balance_str = balance.to_string();
    let mut result = String::new();
    let mut count = 0;

    for c in balance_str.chars().rev() {
        if count > 0 && count % 3 == 0 {
            result.insert(0, ',');
        }
        result.insert(0, c);
        count += 1;
    }

    result
}
