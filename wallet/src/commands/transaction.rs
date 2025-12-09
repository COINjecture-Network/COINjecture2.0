// Transaction commands

use crate::keystore::Keystore;
use crate::rpc_client::RpcClient;
use anyhow::{anyhow, Result};
use coinject_core::{Address, Balance, Ed25519Signature, PublicKey, Transaction, TransferTransaction, TimeLockTransaction, EscrowTransaction, EscrowType, ChannelTransaction, ChannelType, Hash};
use colored::*;
use ed25519_dalek::Signer;

/// Send tokens to an address
pub async fn send_tokens(
    from: &str,
    to_address: &str,
    amount: Balance,
    client: &RpcClient,
) -> Result<()> {
    println!("{}", "Preparing transaction...".dimmed());

    // Load sender account from keystore
    let keystore = Keystore::new()?;
    let sender = keystore.get_account(from)?;

    // Parse recipient address
    let to_address = to_address.trim_start_matches("0x");
    let to_bytes = hex::decode(to_address)
        .map_err(|e| anyhow!("Invalid recipient address: {}", e))?;

    if to_bytes.len() != 32 {
        return Err(anyhow!("Recipient address must be 32 bytes (64 hex chars)"));
    }

    let mut to_addr_bytes = [0u8; 32];
    to_addr_bytes.copy_from_slice(&to_bytes);
    let to = Address::from_bytes(to_addr_bytes);

    // Parse sender address
    let from_bytes = hex::decode(&sender.address)?;
    let mut from_addr_bytes = [0u8; 32];
    from_addr_bytes.copy_from_slice(&from_bytes);
    let from_addr = Address::from_bytes(from_addr_bytes);

    // Parse public key
    let public_key_bytes = hex::decode(&sender.public_key)?;
    let mut pk_bytes = [0u8; 32];
    pk_bytes.copy_from_slice(&public_key_bytes);
    let public_key = PublicKey::from_bytes(pk_bytes);

    // Get current nonce
    let nonce = client.get_nonce(&sender.address).await?;

    // Fee calculation for EIP-1559 style fee market
    // Default: 1000 base fee + 500 priority fee = 1500 total
    // TODO: Query actual base fee from RPC and make priority fee configurable
    let fee = 1500u128; // base_fee (1000) + priority_fee (500)

    println!();
    println!("{}", "Transaction Details".cyan().bold());
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("From:   {} ({})", sender.name.cyan(), sender.address);
    println!("To:     {}", to_address);
    println!("Amount: {} tokens", format_balance(amount));
    println!("Fee:    {} tokens", format_balance(fee));
    println!("Nonce:  {}", nonce);
    println!();

    // Create unsigned transaction for signing
    let signing_message = create_signing_message(&from_addr, &to, amount, fee, nonce, &public_key);

    // Sign transaction
    println!("{}", "Signing transaction...".dimmed());
    let signing_key = keystore.get_signing_key(from)?;
    let sig = signing_key.sign(&signing_message);
    let sig_bytes = sig.to_bytes();

    // Create signed transaction
    let signed_tx = Transaction::Transfer(TransferTransaction {
        from: from_addr,
        to,
        amount,
        fee,
        nonce,
        public_key,
        signature: Ed25519Signature::from_bytes(sig_bytes),
    });

    // Serialize and encode
    let signed_tx_bytes = bincode::serialize(&signed_tx)?;
    let tx_hex = hex::encode(&signed_tx_bytes);

    // Submit to network
    println!("{}", "Broadcasting transaction...".dimmed());

    match client.submit_transaction(&tx_hex).await {
        Ok(tx_hash) => {
            println!();
            println!("{}", "✅ Transaction Submitted Successfully".green().bold());
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("Transaction Hash: {}", tx_hash.green());
            println!();
            println!("Check status with:");
            println!("  coinject-wallet transaction status {}", tx_hash);
        }
        Err(e) => {
            println!();
            println!("{}", "❌ Transaction Failed".red().bold());
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("Error: {}", e);
        }
    }

    Ok(())
}

/// Create a time-locked transaction
pub async fn create_timelock(
    from: &str,
    recipient_address: &str,
    amount: Balance,
    unlock_in_seconds: i64,
    client: &RpcClient,
) -> Result<()> {
    println!("{}", "Preparing time-lock transaction...".dimmed());

    // Load sender account from keystore
    let keystore = Keystore::new()?;
    let sender = keystore.get_account(from)?;

    // Parse recipient address
    let recipient_address = recipient_address.trim_start_matches("0x");
    let recipient_bytes = hex::decode(recipient_address)
        .map_err(|e| anyhow!("Invalid recipient address: {}", e))?;

    if recipient_bytes.len() != 32 {
        return Err(anyhow!("Recipient address must be 32 bytes (64 hex chars)"));
    }

    let mut recipient_addr_bytes = [0u8; 32];
    recipient_addr_bytes.copy_from_slice(&recipient_bytes);
    let recipient = Address::from_bytes(recipient_addr_bytes);

    // Parse sender address
    let from_bytes = hex::decode(&sender.address)?;
    let mut from_addr_bytes = [0u8; 32];
    from_addr_bytes.copy_from_slice(&from_bytes);
    let from_addr = Address::from_bytes(from_addr_bytes);

    // Parse public key
    let public_key_bytes = hex::decode(&sender.public_key)?;
    let mut pk_bytes = [0u8; 32];
    pk_bytes.copy_from_slice(&public_key_bytes);
    let public_key = PublicKey::from_bytes(pk_bytes);

    // Calculate unlock time
    let current_time = chrono::Utc::now().timestamp();
    let unlock_time = current_time + unlock_in_seconds;

    // Get current nonce
    let nonce = client.get_nonce(&sender.address).await?;

    // Fee calculation
    let fee = 1500u128;

    println!();
    println!("{}", "Time-Lock Transaction Details".cyan().bold());
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("From:       {} ({})", sender.name.cyan(), sender.address);
    println!("Recipient:  {}", recipient_address);
    println!("Amount:     {} tokens", format_balance(amount));
    println!("Fee:        {} tokens", format_balance(fee));
    println!("Unlock In:  {} seconds", unlock_in_seconds);
    println!("Unlock At:  {}", chrono::DateTime::from_timestamp(unlock_time, 0)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| "Invalid timestamp".to_string()));
    println!("Nonce:      {}", nonce);
    println!();

    // Create signing message for time-lock
    let signing_message = create_timelock_signing_message(
        &from_addr,
        &recipient,
        amount,
        unlock_time,
        fee,
        nonce,
        &public_key,
    );

    // Sign transaction
    println!("{}", "Signing transaction...".dimmed());
    let signing_key = keystore.get_signing_key(from)?;
    let sig = signing_key.sign(&signing_message);
    let sig_bytes = sig.to_bytes();

    // Create signed transaction
    let signed_tx = Transaction::TimeLock(TimeLockTransaction {
        from: from_addr,
        recipient,
        amount,
        unlock_time,
        fee,
        nonce,
        public_key,
        signature: Ed25519Signature::from_bytes(sig_bytes),
    });

    // Serialize and encode
    let signed_tx_bytes = bincode::serialize(&signed_tx)?;
    let tx_hex = hex::encode(&signed_tx_bytes);

    // Submit to network
    println!("{}", "Broadcasting transaction...".dimmed());

    match client.submit_transaction(&tx_hex).await {
        Ok(tx_hash) => {
            println!();
            println!("{}", "✅ Time-Lock Transaction Submitted Successfully".green().bold());
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("Transaction Hash: {}", tx_hash.green());
            println!();
            println!("Funds are now locked until: {}",
                chrono::DateTime::from_timestamp(unlock_time, 0)
                    .map(|dt| dt.to_rfc3339())
                    .unwrap_or_else(|| "Invalid timestamp".to_string()).yellow());
            println!();
            println!("Check status with:");
            println!("  coinject-wallet transaction status {}", tx_hash);
        }
        Err(e) => {
            println!();
            println!("{}", "❌ Transaction Failed".red().bold());
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("Error: {}", e);
        }
    }

    Ok(())
}

/// Create an escrow
pub async fn create_escrow(
    from: &str,
    recipient_address: &str,
    arbiter_address: Option<&str>,
    amount: Balance,
    timeout_in_seconds: i64,
    conditions: &str,
    client: &RpcClient,
) -> Result<()> {
    println!("{}", "Preparing escrow creation...".dimmed());

    // Load sender account from keystore
    let keystore = Keystore::new()?;
    let sender = keystore.get_account(from)?;

    // Parse recipient address
    let recipient_address = recipient_address.trim_start_matches("0x");
    let recipient_bytes = hex::decode(recipient_address)
        .map_err(|e| anyhow!("Invalid recipient address: {}", e))?;
    if recipient_bytes.len() != 32 {
        return Err(anyhow!("Recipient address must be 32 bytes (64 hex chars)"));
    }
    let mut recipient_addr_bytes = [0u8; 32];
    recipient_addr_bytes.copy_from_slice(&recipient_bytes);
    let recipient = Address::from_bytes(recipient_addr_bytes);

    // Parse arbiter address (if provided)
    let arbiter = if let Some(arb_addr) = arbiter_address {
        let arb_addr = arb_addr.trim_start_matches("0x");
        let arb_bytes = hex::decode(arb_addr)
            .map_err(|e| anyhow!("Invalid arbiter address: {}", e))?;
        if arb_bytes.len() != 32 {
            return Err(anyhow!("Arbiter address must be 32 bytes (64 hex chars)"));
        }
        let mut arb_addr_bytes = [0u8; 32];
        arb_addr_bytes.copy_from_slice(&arb_bytes);
        Some(Address::from_bytes(arb_addr_bytes))
    } else {
        None
    };

    // Parse sender address
    let from_bytes = hex::decode(&sender.address)?;
    let mut from_addr_bytes = [0u8; 32];
    from_addr_bytes.copy_from_slice(&from_bytes);
    let from_addr = Address::from_bytes(from_addr_bytes);

    // Parse public key
    let public_key_bytes = hex::decode(&sender.public_key)?;
    let mut pk_bytes = [0u8; 32];
    pk_bytes.copy_from_slice(&public_key_bytes);
    let public_key = PublicKey::from_bytes(pk_bytes);

    // Calculate timeout
    let current_time = chrono::Utc::now().timestamp();
    let timeout = current_time + timeout_in_seconds;

    // Generate escrow ID (hash of transaction details)
    let escrow_id_bytes = blake3::hash(
        format!("{}-{}-{}-{}", sender.address, recipient_address, amount, current_time).as_bytes()
    ).as_bytes().to_owned();
    let escrow_id = Hash::from_bytes(escrow_id_bytes);

    // Hash the conditions
    let conditions_hash = Hash::from_bytes(*blake3::hash(conditions.as_bytes()).as_bytes());

    // Get current nonce
    let nonce = client.get_nonce(&sender.address).await?;

    // Fee calculation
    let fee = 1500u128;

    println!();
    println!("{}", "Escrow Creation Details".cyan().bold());
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("From:       {} ({})", sender.name.cyan(), sender.address);
    println!("Recipient:  {}", recipient_address);
    if let Some(arb) = arbiter_address {
        println!("Arbiter:    {}", arb);
    } else {
        println!("Arbiter:    {}", "None".dimmed());
    }
    println!("Amount:     {} tokens", format_balance(amount));
    println!("Fee:        {} tokens", format_balance(fee));
    println!("Timeout In: {} seconds", timeout_in_seconds);
    println!("Timeout At: {}", chrono::DateTime::from_timestamp(timeout, 0)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| "Invalid timestamp".to_string()));
    println!("Conditions: {}", conditions);
    println!("Escrow ID:  {}", hex::encode(escrow_id.as_bytes()));
    println!("Nonce:      {}", nonce);
    println!();

    // Create signing message for escrow creation
    let signing_message = create_escrow_create_signing_message(
        &escrow_id,
        &from_addr,
        &recipient,
        amount,
        timeout,
        fee,
        nonce,
        &public_key,
    );

    // Sign transaction
    println!("{}", "Signing transaction...".dimmed());
    let signing_key = keystore.get_signing_key(from)?;
    let sig = signing_key.sign(&signing_message);
    let sig_bytes = sig.to_bytes();

    // Create signed transaction
    let signed_tx = Transaction::Escrow(EscrowTransaction {
        escrow_type: EscrowType::Create {
            recipient,
            arbiter,
            amount,
            timeout,
            conditions_hash,
        },
        escrow_id,
        from: from_addr,
        fee,
        nonce,
        public_key,
        signature: Ed25519Signature::from_bytes(sig_bytes),
        additional_signatures: vec![],
    });

    // Serialize and encode
    let signed_tx_bytes = bincode::serialize(&signed_tx)?;
    let tx_hex = hex::encode(&signed_tx_bytes);

    // Submit to network
    println!("{}", "Broadcasting transaction...".dimmed());

    match client.submit_transaction(&tx_hex).await {
        Ok(tx_hash) => {
            println!();
            println!("{}", "✅ Escrow Created Successfully".green().bold());
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("Transaction Hash: {}", tx_hash.green());
            println!("Escrow ID:        {}", hex::encode(escrow_id.as_bytes()).green());
            println!();
            println!("Funds are now escrowed until: {}",
                chrono::DateTime::from_timestamp(timeout, 0)
                    .map(|dt| dt.to_rfc3339())
                    .unwrap_or_else(|| "Invalid timestamp".to_string()).yellow());
            println!();
            println!("Check status with:");
            println!("  coinject-wallet transaction status {}", tx_hash);
        }
        Err(e) => {
            println!();
            println!("{}", "❌ Transaction Failed".red().bold());
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("Error: {}", e);
        }
    }

    Ok(())
}

/// Release escrowed funds
pub async fn release_escrow(
    from: &str,
    escrow_id: &str,
    client: &RpcClient,
) -> Result<()> {
    println!("{}", "Preparing escrow release...".dimmed());

    // Load account from keystore
    let keystore = Keystore::new()?;
    let sender = keystore.get_account(from)?;

    // Parse escrow ID
    let escrow_id_str = escrow_id.trim_start_matches("0x");
    let escrow_id_bytes = hex::decode(escrow_id_str)
        .map_err(|e| anyhow!("Invalid escrow ID: {}", e))?;
    if escrow_id_bytes.len() != 32 {
        return Err(anyhow!("Escrow ID must be 32 bytes (64 hex chars)"));
    }
    let mut esc_id_bytes = [0u8; 32];
    esc_id_bytes.copy_from_slice(&escrow_id_bytes);
    let esc_id = Hash::from_bytes(esc_id_bytes);

    // Parse sender address
    let from_bytes = hex::decode(&sender.address)?;
    let mut from_addr_bytes = [0u8; 32];
    from_addr_bytes.copy_from_slice(&from_bytes);
    let from_addr = Address::from_bytes(from_addr_bytes);

    // Parse public key
    let public_key_bytes = hex::decode(&sender.public_key)?;
    let mut pk_bytes = [0u8; 32];
    pk_bytes.copy_from_slice(&public_key_bytes);
    let public_key = PublicKey::from_bytes(pk_bytes);

    // Get current nonce
    let nonce = client.get_nonce(&sender.address).await?;

    // Fee calculation
    let fee = 1500u128;

    println!();
    println!("{}", "Escrow Release Details".cyan().bold());
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("From:      {} ({})", sender.name.cyan(), sender.address);
    println!("Escrow ID: {}", escrow_id_str);
    println!("Fee:       {} tokens", format_balance(fee));
    println!("Nonce:     {}", nonce);
    println!();

    // Create signing message for escrow release
    let signing_message = create_escrow_release_refund_signing_message(
        &esc_id,
        &from_addr,
        fee,
        nonce,
        &public_key,
    );

    // Sign transaction
    println!("{}", "Signing transaction...".dimmed());
    let signing_key = keystore.get_signing_key(from)?;
    let sig = signing_key.sign(&signing_message);
    let sig_bytes = sig.to_bytes();

    // Create signed transaction
    let signed_tx = Transaction::Escrow(EscrowTransaction {
        escrow_type: EscrowType::Release,
        escrow_id: esc_id,
        from: from_addr,
        fee,
        nonce,
        public_key,
        signature: Ed25519Signature::from_bytes(sig_bytes),
        additional_signatures: vec![],
    });

    // Serialize and encode
    let signed_tx_bytes = bincode::serialize(&signed_tx)?;
    let tx_hex = hex::encode(&signed_tx_bytes);

    // Submit to network
    println!("{}", "Broadcasting transaction...".dimmed());

    match client.submit_transaction(&tx_hex).await {
        Ok(tx_hash) => {
            println!();
            println!("{}", "✅ Escrow Released Successfully".green().bold());
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("Transaction Hash: {}", tx_hash.green());
            println!();
            println!("Check status with:");
            println!("  coinject-wallet transaction status {}", tx_hash);
        }
        Err(e) => {
            println!();
            println!("{}", "❌ Transaction Failed".red().bold());
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("Error: {}", e);
        }
    }

    Ok(())
}

/// Refund escrowed funds
pub async fn refund_escrow(
    from: &str,
    escrow_id: &str,
    client: &RpcClient,
) -> Result<()> {
    println!("{}", "Preparing escrow refund...".dimmed());

    // Load account from keystore
    let keystore = Keystore::new()?;
    let sender = keystore.get_account(from)?;

    // Parse escrow ID
    let escrow_id_str = escrow_id.trim_start_matches("0x");
    let escrow_id_bytes = hex::decode(escrow_id_str)
        .map_err(|e| anyhow!("Invalid escrow ID: {}", e))?;
    if escrow_id_bytes.len() != 32 {
        return Err(anyhow!("Escrow ID must be 32 bytes (64 hex chars)"));
    }
    let mut esc_id_bytes = [0u8; 32];
    esc_id_bytes.copy_from_slice(&escrow_id_bytes);
    let esc_id = Hash::from_bytes(esc_id_bytes);

    // Parse sender address
    let from_bytes = hex::decode(&sender.address)?;
    let mut from_addr_bytes = [0u8; 32];
    from_addr_bytes.copy_from_slice(&from_bytes);
    let from_addr = Address::from_bytes(from_addr_bytes);

    // Parse public key
    let public_key_bytes = hex::decode(&sender.public_key)?;
    let mut pk_bytes = [0u8; 32];
    pk_bytes.copy_from_slice(&public_key_bytes);
    let public_key = PublicKey::from_bytes(pk_bytes);

    // Get current nonce
    let nonce = client.get_nonce(&sender.address).await?;

    // Fee calculation
    let fee = 1500u128;

    println!();
    println!("{}", "Escrow Refund Details".cyan().bold());
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("From:      {} ({})", sender.name.cyan(), sender.address);
    println!("Escrow ID: {}", escrow_id_str);
    println!("Fee:       {} tokens", format_balance(fee));
    println!("Nonce:     {}", nonce);
    println!();

    // Create signing message for escrow refund
    let signing_message = create_escrow_release_refund_signing_message(
        &esc_id,
        &from_addr,
        fee,
        nonce,
        &public_key,
    );

    // Sign transaction
    println!("{}", "Signing transaction...".dimmed());
    let signing_key = keystore.get_signing_key(from)?;
    let sig = signing_key.sign(&signing_message);
    let sig_bytes = sig.to_bytes();

    // Create signed transaction
    let signed_tx = Transaction::Escrow(EscrowTransaction {
        escrow_type: EscrowType::Refund,
        escrow_id: esc_id,
        from: from_addr,
        fee,
        nonce,
        public_key,
        signature: Ed25519Signature::from_bytes(sig_bytes),
        additional_signatures: vec![],
    });

    // Serialize and encode
    let signed_tx_bytes = bincode::serialize(&signed_tx)?;
    let tx_hex = hex::encode(&signed_tx_bytes);

    // Submit to network
    println!("{}", "Broadcasting transaction...".dimmed());

    match client.submit_transaction(&tx_hex).await {
        Ok(tx_hash) => {
            println!();
            println!("{}", "✅ Escrow Refunded Successfully".green().bold());
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("Transaction Hash: {}", tx_hash.green());
            println!();
            println!("Check status with:");
            println!("  coinject-wallet transaction status {}", tx_hash);
        }
        Err(e) => {
            println!();
            println!("{}", "❌ Transaction Failed".red().bold());
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("Error: {}", e);
        }
    }

    Ok(())
}

/// Open a new payment channel
pub async fn open_channel(
    from: &str,
    participant_b_address: &str,
    deposit_a: Balance,
    deposit_b: Balance,
    timeout_seconds: i64,
    client: &RpcClient,
) -> Result<()> {
    println!("{}", "Preparing channel open...".dimmed());

    // Load sender account from keystore (participant A)
    let keystore = Keystore::new()?;
    let sender = keystore.get_account(from)?;

    // Parse participant B address
    let participant_b_str = participant_b_address.trim_start_matches("0x");
    let participant_b_bytes = hex::decode(participant_b_str)
        .map_err(|e| anyhow!("Invalid participant B address: {}", e))?;
    if participant_b_bytes.len() != 32 {
        return Err(anyhow!("Participant B address must be 32 bytes (64 hex chars)"));
    }
    let mut participant_b_addr_bytes = [0u8; 32];
    participant_b_addr_bytes.copy_from_slice(&participant_b_bytes);
    let participant_b = Address::from_bytes(participant_b_addr_bytes);

    // Parse sender address (participant A)
    let from_bytes = hex::decode(&sender.address)?;
    let mut from_addr_bytes = [0u8; 32];
    from_addr_bytes.copy_from_slice(&from_bytes);
    let from_addr = Address::from_bytes(from_addr_bytes);

    // Parse public key
    let public_key_bytes = hex::decode(&sender.public_key)?;
    let mut pk_bytes = [0u8; 32];
    pk_bytes.copy_from_slice(&public_key_bytes);
    let public_key = PublicKey::from_bytes(pk_bytes);

    // Get current nonce
    let nonce = client.get_nonce(&sender.address).await?;

    // Fee calculation
    let fee = 2000u128;

    // Generate channel ID using blake3 hash
    let current_time = chrono::Utc::now().timestamp();
    let channel_id_bytes = blake3::hash(
        format!("{}-{}-{}-{}", sender.address, participant_b_address, deposit_a + deposit_b, current_time)
            .as_bytes(),
    )
    .as_bytes()
    .to_owned();
    let channel_id = Hash::from_bytes(channel_id_bytes);

    println!();
    println!("{}", "Channel Open Details".cyan().bold());
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("From:          {} ({})", sender.name.cyan(), sender.address);
    println!("Participant A: {}", sender.address);
    println!("Participant B: {}", participant_b_str);
    println!("Deposit A:     {} tokens", format_balance(deposit_a));
    println!("Deposit B:     {} tokens", format_balance(deposit_b));
    println!("Total:         {} tokens", format_balance(deposit_a + deposit_b));
    println!("Timeout:       {} seconds", timeout_seconds);
    println!("Fee:           {} tokens", format_balance(fee));
    println!("Nonce:         {}", nonce);
    println!("Channel ID:    {}", hex::encode(channel_id.as_bytes()).yellow());
    println!();

    // Create signing message for channel open
    let signing_message = create_channel_open_signing_message(
        &channel_id,
        &from_addr,
        deposit_a,
        deposit_b,
        fee,
        nonce,
        &public_key,
    );

    // Sign transaction
    println!("{}", "Signing transaction...".dimmed());
    let signing_key = keystore.get_signing_key(from)?;
    let sig = signing_key.sign(&signing_message);
    let sig_bytes = sig.to_bytes();

    // Create signed transaction
    let signed_tx = Transaction::Channel(ChannelTransaction {
        channel_type: ChannelType::Open {
            participant_a: from_addr,
            participant_b,
            deposit_a,
            deposit_b,
            timeout: timeout_seconds,
        },
        channel_id,
        from: from_addr,
        fee,
        nonce,
        public_key,
        signature: Ed25519Signature::from_bytes(sig_bytes),
        additional_signatures: vec![],
    });

    // Serialize and encode
    let signed_tx_bytes = bincode::serialize(&signed_tx)?;
    let tx_hex = hex::encode(&signed_tx_bytes);

    // Submit to network
    println!("{}", "Broadcasting transaction...".dimmed());

    match client.submit_transaction(&tx_hex).await {
        Ok(tx_hash) => {
            println!();
            println!("{}", "✅ Channel Opened Successfully".green().bold());
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("Transaction Hash: {}", tx_hash.green());
            println!("Channel ID:       {}", hex::encode(channel_id.as_bytes()).green());
            println!();
            println!("Check status with:");
            println!("  coinject-wallet transaction status {}", tx_hash);
        }
        Err(e) => {
            println!();
            println!("{}", "❌ Transaction Failed".red().bold());
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("Error: {}", e);
        }
    }

    Ok(())
}

/// Update channel state (typically for off-chain state updates)
pub async fn update_channel(
    from: &str,
    channel_id: &str,
    sequence: u64,
    balance_a: Balance,
    balance_b: Balance,
    client: &RpcClient,
) -> Result<()> {
    println!("{}", "Preparing channel update...".dimmed());

    // Load account from keystore
    let keystore = Keystore::new()?;
    let sender = keystore.get_account(from)?;

    // Parse channel ID
    let channel_id_str = channel_id.trim_start_matches("0x");
    let channel_id_bytes = hex::decode(channel_id_str)
        .map_err(|e| anyhow!("Invalid channel ID: {}", e))?;
    if channel_id_bytes.len() != 32 {
        return Err(anyhow!("Channel ID must be 32 bytes (64 hex chars)"));
    }
    let mut chan_id_bytes = [0u8; 32];
    chan_id_bytes.copy_from_slice(&channel_id_bytes);
    let chan_id = Hash::from_bytes(chan_id_bytes);

    // Parse sender address
    let from_bytes = hex::decode(&sender.address)?;
    let mut from_addr_bytes = [0u8; 32];
    from_addr_bytes.copy_from_slice(&from_bytes);
    let from_addr = Address::from_bytes(from_addr_bytes);

    // Parse public key
    let public_key_bytes = hex::decode(&sender.public_key)?;
    let mut pk_bytes = [0u8; 32];
    pk_bytes.copy_from_slice(&public_key_bytes);
    let public_key = PublicKey::from_bytes(pk_bytes);

    // Get current nonce
    let nonce = client.get_nonce(&sender.address).await?;

    // Fee calculation
    let fee = 1000u128;

    println!();
    println!("{}", "Channel Update Details".cyan().bold());
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("From:       {} ({})", sender.name.cyan(), sender.address);
    println!("Channel ID: {}", channel_id_str);
    println!("Sequence:   {}", sequence);
    println!("Balance A:  {} tokens", format_balance(balance_a));
    println!("Balance B:  {} tokens", format_balance(balance_b));
    println!("Fee:        {} tokens", format_balance(fee));
    println!("Nonce:      {}", nonce);
    println!();

    // Create signing message for channel update
    let signing_message = create_channel_update_signing_message(
        &chan_id,
        &from_addr,
        fee,
        nonce,
        &public_key,
    );

    // Sign transaction
    println!("{}", "Signing transaction...".dimmed());
    let signing_key = keystore.get_signing_key(from)?;
    let sig = signing_key.sign(&signing_message);
    let sig_bytes = sig.to_bytes();

    // Create signed transaction
    let signed_tx = Transaction::Channel(ChannelTransaction {
        channel_type: ChannelType::Update {
            sequence,
            balance_a,
            balance_b,
        },
        channel_id: chan_id,
        from: from_addr,
        fee,
        nonce,
        public_key,
        signature: Ed25519Signature::from_bytes(sig_bytes),
        additional_signatures: vec![],
    });

    // Serialize and encode
    let signed_tx_bytes = bincode::serialize(&signed_tx)?;
    let tx_hex = hex::encode(&signed_tx_bytes);

    // Submit to network
    println!("{}", "Broadcasting transaction...".dimmed());

    match client.submit_transaction(&tx_hex).await {
        Ok(tx_hash) => {
            println!();
            println!("{}", "✅ Channel Updated Successfully".green().bold());
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("Transaction Hash: {}", tx_hash.green());
            println!();
            println!("Check status with:");
            println!("  coinject-wallet transaction status {}", tx_hash);
        }
        Err(e) => {
            println!();
            println!("{}", "❌ Transaction Failed".red().bold());
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("Error: {}", e);
        }
    }

    Ok(())
}

/// Close a payment channel (cooperative or unilateral)
pub async fn close_channel(
    from: &str,
    channel_id: &str,
    final_balance_a: Balance,
    final_balance_b: Balance,
    client: &RpcClient,
) -> Result<()> {
    println!("{}", "Preparing channel close (cooperative)...".dimmed());

    // Load account from keystore
    let keystore = Keystore::new()?;
    let sender = keystore.get_account(from)?;

    // Parse channel ID
    let channel_id_str = channel_id.trim_start_matches("0x");
    let channel_id_bytes = hex::decode(channel_id_str)
        .map_err(|e| anyhow!("Invalid channel ID: {}", e))?;
    if channel_id_bytes.len() != 32 {
        return Err(anyhow!("Channel ID must be 32 bytes (64 hex chars)"));
    }
    let mut chan_id_bytes = [0u8; 32];
    chan_id_bytes.copy_from_slice(&channel_id_bytes);
    let chan_id = Hash::from_bytes(chan_id_bytes);

    // Parse sender address
    let from_bytes = hex::decode(&sender.address)?;
    let mut from_addr_bytes = [0u8; 32];
    from_addr_bytes.copy_from_slice(&from_bytes);
    let from_addr = Address::from_bytes(from_addr_bytes);

    // Parse public key
    let public_key_bytes = hex::decode(&sender.public_key)?;
    let mut pk_bytes = [0u8; 32];
    pk_bytes.copy_from_slice(&public_key_bytes);
    let public_key = PublicKey::from_bytes(pk_bytes);

    // Get current nonce
    let nonce = client.get_nonce(&sender.address).await?;

    // Fee calculation
    let fee = 1500u128;

    println!();
    println!("{}", "Channel Close Details".cyan().bold());
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("From:            {} ({})", sender.name.cyan(), sender.address);
    println!("Channel ID:      {}", channel_id_str);
    println!("Final Balance A: {} tokens", format_balance(final_balance_a));
    println!("Final Balance B: {} tokens", format_balance(final_balance_b));
    println!("Fee:             {} tokens", format_balance(fee));
    println!("Nonce:           {}", nonce);
    println!();

    // Create signing message for channel cooperative close
    let signing_message = create_channel_close_signing_message(
        &chan_id,
        &from_addr,
        final_balance_a,
        final_balance_b,
        fee,
        nonce,
        &public_key,
    );

    // Sign transaction
    println!("{}", "Signing transaction...".dimmed());
    let signing_key = keystore.get_signing_key(from)?;
    let sig = signing_key.sign(&signing_message);
    let sig_bytes = sig.to_bytes();

    // Create signed transaction
    let signed_tx = Transaction::Channel(ChannelTransaction {
        channel_type: ChannelType::CooperativeClose {
            final_balance_a,
            final_balance_b,
        },
        channel_id: chan_id,
        from: from_addr,
        fee,
        nonce,
        public_key,
        signature: Ed25519Signature::from_bytes(sig_bytes),
        additional_signatures: vec![],
    });

    // Serialize and encode
    let signed_tx_bytes = bincode::serialize(&signed_tx)?;
    let tx_hex = hex::encode(&signed_tx_bytes);

    // Submit to network
    println!("{}", "Broadcasting transaction...".dimmed());

    match client.submit_transaction(&tx_hex).await {
        Ok(tx_hash) => {
            println!();
            println!("{}", "✅ Channel Closed Successfully".green().bold());
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("Transaction Hash: {}", tx_hash.green());
            println!();
            println!("Check status with:");
            println!("  coinject-wallet transaction status {}", tx_hash);
        }
        Err(e) => {
            println!();
            println!("{}", "❌ Transaction Failed".red().bold());
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("Error: {}", e);
        }
    }

    Ok(())
}

/// Get transaction status
pub async fn get_transaction_status(tx_hash: &str, client: &RpcClient) -> Result<()> {
    let tx_hash = tx_hash.trim_start_matches("0x");

    println!("{}", "Querying transaction status...".dimmed());

    match client.get_transaction_status(tx_hash).await {
        Ok(status) => {
            println!();
            println!("{}", "Transaction Status".cyan().bold());
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("TX Hash: {}", status.tx_hash);

            let status_text = match status.status.as_str() {
                "pending" => status.status.yellow(),
                "confirmed" => status.status.green(),
                "failed" => status.status.red(),
                _ => status.status.normal(),
            };
            println!("Status:  {}", status_text.bold());

            if let Some(height) = status.block_height {
                println!("Block:   #{}", height);
            }
        }
        Err(e) => {
            println!("{}", format!("❌ Failed to get transaction status: {}", e).red());
        }
    }

    Ok(())
}

// Helper functions

/// Create the signing message (matches TransferTransaction::signing_message in core)
fn create_signing_message(
    from: &Address,
    to: &Address,
    amount: Balance,
    fee: Balance,
    nonce: u64,
    public_key: &PublicKey,
) -> Vec<u8> {
    let mut msg = Vec::new();
    msg.extend_from_slice(from.as_bytes());
    msg.extend_from_slice(to.as_bytes());
    msg.extend_from_slice(&amount.to_le_bytes());
    msg.extend_from_slice(&fee.to_le_bytes());
    msg.extend_from_slice(&nonce.to_le_bytes());
    msg.extend_from_slice(public_key.as_bytes());
    msg
}

/// Create the signing message for TimeLock (matches TimeLockTransaction::signing_message in core)
fn create_timelock_signing_message(
    from: &Address,
    recipient: &Address,
    amount: Balance,
    unlock_time: i64,
    fee: Balance,
    nonce: u64,
    public_key: &PublicKey,
) -> Vec<u8> {
    let mut msg = Vec::new();
    msg.extend_from_slice(from.as_bytes());
    msg.extend_from_slice(recipient.as_bytes());
    msg.extend_from_slice(&amount.to_le_bytes());
    msg.extend_from_slice(&unlock_time.to_le_bytes());
    msg.extend_from_slice(&fee.to_le_bytes());
    msg.extend_from_slice(&nonce.to_le_bytes());
    msg.extend_from_slice(public_key.as_bytes());
    msg
}

/// Create the signing message for Escrow Create (matches EscrowTransaction::signing_message in core)
fn create_escrow_create_signing_message(
    escrow_id: &Hash,
    from: &Address,
    recipient: &Address,
    amount: Balance,
    timeout: i64,
    fee: Balance,
    nonce: u64,
    public_key: &PublicKey,
) -> Vec<u8> {
    let mut msg = Vec::new();
    msg.extend_from_slice(escrow_id.as_bytes());
    msg.extend_from_slice(from.as_bytes());
    msg.extend_from_slice(&fee.to_le_bytes());
    msg.extend_from_slice(&nonce.to_le_bytes());
    msg.extend_from_slice(public_key.as_bytes());
    // Include escrow type specific data for Create
    msg.extend_from_slice(recipient.as_bytes());
    msg.extend_from_slice(&amount.to_le_bytes());
    msg.extend_from_slice(&timeout.to_le_bytes());
    msg
}

/// Create the signing message for Escrow Release/Refund (matches EscrowTransaction::signing_message in core)
fn create_escrow_release_refund_signing_message(
    escrow_id: &Hash,
    from: &Address,
    fee: Balance,
    nonce: u64,
    public_key: &PublicKey,
) -> Vec<u8> {
    let mut msg = Vec::new();
    msg.extend_from_slice(escrow_id.as_bytes());
    msg.extend_from_slice(from.as_bytes());
    msg.extend_from_slice(&fee.to_le_bytes());
    msg.extend_from_slice(&nonce.to_le_bytes());
    msg.extend_from_slice(public_key.as_bytes());
    // Release/Refund don't include additional data
    msg
}

/// Create the signing message for Channel Open (matches ChannelTransaction::signing_message in core)
fn create_channel_open_signing_message(
    channel_id: &Hash,
    from: &Address,
    deposit_a: Balance,
    deposit_b: Balance,
    fee: Balance,
    nonce: u64,
    public_key: &PublicKey,
) -> Vec<u8> {
    let mut msg = Vec::new();
    msg.extend_from_slice(channel_id.as_bytes());
    msg.extend_from_slice(from.as_bytes());
    msg.extend_from_slice(&fee.to_le_bytes());
    msg.extend_from_slice(&nonce.to_le_bytes());
    msg.extend_from_slice(public_key.as_bytes());
    // Include channel type specific data for Open
    msg.extend_from_slice(&deposit_a.to_le_bytes());
    msg.extend_from_slice(&deposit_b.to_le_bytes());
    msg
}

/// Create the signing message for Channel Update (matches ChannelTransaction::signing_message in core)
fn create_channel_update_signing_message(
    channel_id: &Hash,
    from: &Address,
    fee: Balance,
    nonce: u64,
    public_key: &PublicKey,
) -> Vec<u8> {
    let mut msg = Vec::new();
    msg.extend_from_slice(channel_id.as_bytes());
    msg.extend_from_slice(from.as_bytes());
    msg.extend_from_slice(&fee.to_le_bytes());
    msg.extend_from_slice(&nonce.to_le_bytes());
    msg.extend_from_slice(public_key.as_bytes());
    // Update doesn't include additional data in signing message
    msg
}

/// Create the signing message for Channel CooperativeClose (matches ChannelTransaction::signing_message in core)
fn create_channel_close_signing_message(
    channel_id: &Hash,
    from: &Address,
    final_balance_a: Balance,
    final_balance_b: Balance,
    fee: Balance,
    nonce: u64,
    public_key: &PublicKey,
) -> Vec<u8> {
    let mut msg = Vec::new();
    msg.extend_from_slice(channel_id.as_bytes());
    msg.extend_from_slice(from.as_bytes());
    msg.extend_from_slice(&fee.to_le_bytes());
    msg.extend_from_slice(&nonce.to_le_bytes());
    msg.extend_from_slice(public_key.as_bytes());
    // Include channel type specific data for CooperativeClose
    msg.extend_from_slice(&final_balance_a.to_le_bytes());
    msg.extend_from_slice(&final_balance_b.to_le_bytes());
    msg
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
