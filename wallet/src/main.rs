// COINjecture Network B Wallet CLI
// Interactive command-line wallet for managing accounts, transactions, and marketplace interactions

// Signing-message builders are public APIs that must accept each field individually.
#![allow(clippy::too_many_arguments)]

use clap::{Parser, Subcommand};
use colored::*;

mod commands;
mod keystore;
mod rpc_client;

use commands::{account, marketplace, transaction};
use rpc_client::RpcClient;

/// COINjecture Network B Wallet CLI
#[derive(Parser)]
#[command(name = "coinject-wallet")]
#[command(version = "0.1.0")]
#[command(about = "Command-line wallet for COINjecture Network B", long_about = None)]
struct Cli {
    /// RPC endpoint URL
    #[arg(short, long, default_value = "http://127.0.0.1:9944")]
    rpc: String,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Account management commands
    #[command(subcommand)]
    Account(AccountCommands),

    /// Transaction commands
    #[command(subcommand)]
    Transaction(TransactionCommands),

    /// Marketplace commands
    #[command(subcommand)]
    Marketplace(MarketplaceCommands),

    /// Chain information commands
    #[command(subcommand)]
    Chain(ChainCommands),

    /// Testnet faucet commands
    #[command(subcommand)]
    Faucet(FaucetCommands),
}

#[derive(Subcommand)]
enum AccountCommands {
    /// Generate a new keypair
    New {
        /// Name for the account
        #[arg(short, long)]
        name: Option<String>,
    },

    /// List all accounts in keystore
    List,

    /// Get account balance
    Balance {
        /// Account address (hex)
        address: String,
    },

    /// Get full account information
    Info {
        /// Account address (hex)
        address: String,
    },

    /// Export account private key
    Export {
        /// Account name or address
        account: String,
    },

    /// Import account from private key
    Import {
        /// Private key (hex)
        #[arg(short, long)]
        key: String,

        /// Name for the account
        #[arg(short, long)]
        name: Option<String>,
    },
}

#[derive(Subcommand)]
enum TransactionCommands {
    /// Send tokens to an address
    Send {
        /// Sender account name or address
        #[arg(short, long)]
        from: String,

        /// Recipient address
        to: String,

        /// Amount to send
        amount: u128,
    },

    /// Create a time-locked transaction
    Timelock {
        /// Sender account name or address
        #[arg(short, long)]
        from: String,

        /// Recipient address (who can claim after unlock)
        to: String,

        /// Amount to lock
        amount: u128,

        /// Unlock time in seconds from now (e.g., 3600 for 1 hour)
        #[arg(short, long)]
        unlock_in: i64,
    },

    /// Get transaction status
    Status {
        /// Transaction hash (hex)
        tx_hash: String,
    },

    /// Create an escrow
    EscrowCreate {
        /// Sender account name or address
        #[arg(short, long)]
        from: String,

        /// Recipient address (who receives on release)
        to: String,

        /// Optional arbiter address
        #[arg(short, long)]
        arbiter: Option<String>,

        /// Amount to escrow
        amount: u128,

        /// Timeout in seconds from now
        #[arg(short = 't', long)]
        timeout: i64,

        /// Conditions description
        #[arg(short, long)]
        conditions: String,
    },

    /// Release escrowed funds
    EscrowRelease {
        /// Releaser account name or address (must be recipient or arbiter)
        #[arg(short, long)]
        from: String,

        /// Escrow ID (hex)
        escrow_id: String,
    },

    /// Refund escrowed funds
    EscrowRefund {
        /// Refunder account name or address (must be sender or arbiter)
        #[arg(short, long)]
        from: String,

        /// Escrow ID (hex)
        escrow_id: String,
    },

    /// Open a payment channel
    ChannelOpen {
        /// Opener account name or address (participant A)
        #[arg(short, long)]
        from: String,

        /// Participant B address
        to: String,

        /// Deposit from participant A
        #[arg(short = 'a', long)]
        deposit_a: u128,

        /// Deposit from participant B
        #[arg(short = 'b', long)]
        deposit_b: u128,

        /// Timeout in seconds
        #[arg(short = 't', long)]
        timeout: i64,
    },

    /// Update payment channel state
    ChannelUpdate {
        /// Account name or address
        #[arg(short, long)]
        from: String,

        /// Channel ID (hex)
        channel_id: String,

        /// Sequence number
        #[arg(short, long)]
        sequence: u64,

        /// Balance for participant A
        #[arg(short = 'a', long)]
        balance_a: u128,

        /// Balance for participant B
        #[arg(short = 'b', long)]
        balance_b: u128,
    },

    /// Close a payment channel (cooperative)
    ChannelClose {
        /// Account name or address
        #[arg(short, long)]
        from: String,

        /// Channel ID (hex)
        channel_id: String,

        /// Final balance for participant A
        #[arg(short = 'a', long)]
        final_balance_a: u128,

        /// Final balance for participant B
        #[arg(short = 'b', long)]
        final_balance_b: u128,
    },

    /// Create a bilateral trustline with dimensional economics
    TrustlineCreate {
        /// Account name or address (participant A)
        #[arg(short, long)]
        from: String,

        /// Participant B address
        to: String,

        /// Credit limit from A to B
        #[arg(long)]
        limit_a_to_b: u128,

        /// Credit limit from B to A
        #[arg(long)]
        limit_b_to_a: u128,

        /// Dimensional scale (1-8)
        #[arg(short = 'd', long, default_value = "3")]
        dimensional_scale: u8,
    },

    /// Swap tokens between dimensional pools
    PoolSwap {
        /// Account name or address
        #[arg(short, long)]
        from: String,

        /// Pool to swap from (D1, D2, D3)
        #[arg(long)]
        pool_from: String,

        /// Pool to swap to (D1, D2, D3)
        #[arg(long)]
        pool_to: String,

        /// Amount to swap in
        #[arg(long)]
        amount_in: u128,

        /// Minimum amount expected out (slippage protection)
        #[arg(long)]
        min_amount_out: u128,
    },
}

#[derive(Subcommand)]
enum MarketplaceCommands {
    /// List open problems
    ListProblems,

    /// Get marketplace statistics
    Stats,

    /// Get problem details
    Problem {
        /// Problem ID (hex)
        problem_id: String,
    },

    /// Submit a new problem (requires implementation)
    Submit {
        /// Problem type (sat, tsp, subsetsum)
        #[arg(short, long)]
        problem_type: String,

        /// Bounty amount
        #[arg(short, long)]
        bounty: u128,
    },
}

#[derive(Subcommand)]
enum ChainCommands {
    /// Get chain information
    Info,

    /// Get block by height
    Block {
        /// Block height
        height: u64,
    },

    /// Get latest block
    Latest,
}

#[derive(Subcommand)]
enum FaucetCommands {
    /// Request testnet tokens from faucet
    Request {
        /// Account name or address to credit
        #[arg(short, long)]
        account: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Print banner
    print_banner();

    // Create RPC client
    let client = RpcClient::new(&cli.rpc);

    if cli.verbose {
        println!("{}", format!("🔌 Connected to: {}", cli.rpc).dimmed());
        println!();
    }

    // Route to appropriate command handler
    match cli.command {
        Commands::Account(cmd) => handle_account_command(cmd, &client).await?,
        Commands::Transaction(cmd) => handle_transaction_command(cmd, &client).await?,
        Commands::Marketplace(cmd) => handle_marketplace_command(cmd, &client).await?,
        Commands::Chain(cmd) => handle_chain_command(cmd, &client).await?,
        Commands::Faucet(cmd) => handle_faucet_command(cmd, &client).await?,
    }

    Ok(())
}

async fn handle_account_command(cmd: AccountCommands, client: &RpcClient) -> anyhow::Result<()> {
    match cmd {
        AccountCommands::New { name } => account::new_account(name).await?,
        AccountCommands::List => account::list_accounts().await?,
        AccountCommands::Balance { address } => account::get_balance(&address, client).await?,
        AccountCommands::Info { address } => account::get_account_info(&address, client).await?,
        AccountCommands::Export { account } => account::export_account(&account).await?,
        AccountCommands::Import { key, name } => account::import_account(&key, name).await?,
    }
    Ok(())
}

async fn handle_transaction_command(
    cmd: TransactionCommands,
    client: &RpcClient,
) -> anyhow::Result<()> {
    match cmd {
        TransactionCommands::Send { from, to, amount } => {
            transaction::send_tokens(&from, &to, amount, client).await?
        }
        TransactionCommands::Timelock {
            from,
            to,
            amount,
            unlock_in,
        } => transaction::create_timelock(&from, &to, amount, unlock_in, client).await?,
        TransactionCommands::Status { tx_hash } => {
            transaction::get_transaction_status(&tx_hash, client).await?
        }
        TransactionCommands::EscrowCreate {
            from,
            to,
            arbiter,
            amount,
            timeout,
            conditions,
        } => {
            transaction::create_escrow(
                &from,
                &to,
                arbiter.as_deref(),
                amount,
                timeout,
                &conditions,
                client,
            )
            .await?
        }
        TransactionCommands::EscrowRelease { from, escrow_id } => {
            transaction::release_escrow(&from, &escrow_id, client).await?
        }
        TransactionCommands::EscrowRefund { from, escrow_id } => {
            transaction::refund_escrow(&from, &escrow_id, client).await?
        }
        TransactionCommands::ChannelOpen {
            from,
            to,
            deposit_a,
            deposit_b,
            timeout,
        } => transaction::open_channel(&from, &to, deposit_a, deposit_b, timeout, client).await?,
        TransactionCommands::ChannelUpdate {
            from,
            channel_id,
            sequence,
            balance_a,
            balance_b,
        } => {
            transaction::update_channel(&from, &channel_id, sequence, balance_a, balance_b, client)
                .await?
        }
        TransactionCommands::ChannelClose {
            from,
            channel_id,
            final_balance_a,
            final_balance_b,
        } => {
            transaction::close_channel(&from, &channel_id, final_balance_a, final_balance_b, client)
                .await?
        }
        TransactionCommands::TrustlineCreate {
            from,
            to,
            limit_a_to_b,
            limit_b_to_a,
            dimensional_scale,
        } => {
            println!(
                "{}",
                "⚠️  TrustLine transactions are not yet fully implemented".yellow()
            );
            println!(
                "{}",
                "This feature will be available in a future update.".dimmed()
            );
            println!();
            println!("Parameters would be:");
            println!("  From: {}", from);
            println!("  To: {}", to);
            println!("  Limit A→B: {}", limit_a_to_b);
            println!("  Limit B→A: {}", limit_b_to_a);
            println!("  Dimensional Scale: {}", dimensional_scale);
        }
        TransactionCommands::PoolSwap {
            from,
            pool_from,
            pool_to,
            amount_in,
            min_amount_out,
        } => {
            println!(
                "{}",
                "⚠️  Pool Swap transactions are not yet fully implemented".yellow()
            );
            println!(
                "{}",
                "This feature will be available in a future update.".dimmed()
            );
            println!();
            println!("Parameters would be:");
            println!("  From: {}", from);
            println!("  Pool From: {}", pool_from);
            println!("  Pool To: {}", pool_to);
            println!("  Amount In: {}", amount_in);
            println!("  Min Amount Out: {}", min_amount_out);
        }
    }
    Ok(())
}

async fn handle_marketplace_command(
    cmd: MarketplaceCommands,
    client: &RpcClient,
) -> anyhow::Result<()> {
    match cmd {
        MarketplaceCommands::ListProblems => marketplace::list_problems(client).await?,
        MarketplaceCommands::Stats => marketplace::get_stats(client).await?,
        MarketplaceCommands::Problem { problem_id } => {
            marketplace::get_problem(&problem_id, client).await?
        }
        MarketplaceCommands::Submit {
            problem_type,
            bounty,
        } => marketplace::submit_problem(&problem_type, bounty, client).await?,
    }
    Ok(())
}

async fn handle_chain_command(cmd: ChainCommands, client: &RpcClient) -> anyhow::Result<()> {
    match cmd {
        ChainCommands::Info => {
            let info = client.get_chain_info().await?;
            println!("{}", "Chain Information".green().bold());
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("Chain ID:      {}", info.chain_id);
            println!("Best Height:   {}", info.best_height);
            println!("Best Hash:     {}", info.best_hash);
            println!("Genesis Hash:  {}", info.genesis_hash);
            println!("Peers:         {}", info.peer_count);
        }
        ChainCommands::Block { height } => {
            if let Some(block) = client.get_block(height).await? {
                println!("{}", format!("Block #{}", height).green().bold());
                println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                println!(
                    "Hash:       {}",
                    hex::encode(block.header.hash().as_bytes())
                );
                println!(
                    "Parent:     {}",
                    hex::encode(block.header.prev_hash.as_bytes())
                );
                println!("Miner:      {}", hex::encode(block.header.miner.as_bytes()));
                println!("Timestamp:  {}", block.header.timestamp);
                println!("Nonce:      {}", block.header.nonce);
                println!();
                println!("Problem:    {:?}", block.solution_reveal.problem);
                println!("Solution:   {:?}", block.solution_reveal.solution);
                println!("Work Score: {:.4}", block.header.work_score);
                println!("Reward:     {} tokens", block.coinbase.reward);
            } else {
                println!("{}", "Block not found".red());
            }
        }
        ChainCommands::Latest => {
            if let Some(block) = client.get_latest_block().await? {
                let height = block.header.height;
                println!("{}", format!("Latest Block #{}", height).green().bold());
                println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                println!(
                    "Hash:       {}",
                    hex::encode(block.header.hash().as_bytes())
                );
                println!("Miner:      {}", hex::encode(block.header.miner.as_bytes()));
                println!("Timestamp:  {}", block.header.timestamp);
                println!("Work Score: {:.4}", block.header.work_score);
                println!("Reward:     {} tokens", block.coinbase.reward);
            } else {
                println!("{}", "No blocks found".red());
            }
        }
    }
    Ok(())
}

async fn handle_faucet_command(cmd: FaucetCommands, client: &RpcClient) -> anyhow::Result<()> {
    match cmd {
        FaucetCommands::Request { account } => {
            println!("{}", "💧 Requesting tokens from testnet faucet...".dimmed());
            println!();

            // Load account from keystore or use as address directly
            let address = if account.len() == 64 && account.chars().all(|c| c.is_ascii_hexdigit()) {
                account.clone()
            } else {
                // Try to load from keystore
                let keystore = crate::keystore::Keystore::new()?;
                let acc = keystore.get_account(&account)?;
                acc.address
            };

            // Call faucet RPC method
            match client.faucet_request(&address).await {
                Ok(response) => {
                    if response.success {
                        println!("{}", "✅ Faucet Request Successful".green().bold());
                        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                        println!("Amount credited: {} tokens", response.amount.unwrap_or(0));
                        println!(
                            "New balance:     {} tokens",
                            response.new_balance.unwrap_or(0)
                        );
                        println!();
                        println!("{}", response.message.green());
                    } else {
                        println!("{}", "❌ Faucet Request Failed".red().bold());
                        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                        println!("{}", response.message.yellow());
                        if let Some(cooldown) = response.cooldown_remaining {
                            println!();
                            println!("Try again in: {} seconds", cooldown);
                        }
                    }
                }
                Err(e) => {
                    println!("{}", "❌ Failed to contact faucet".red().bold());
                    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                    println!("Error: {}", e);
                }
            }
        }
    }
    Ok(())
}

fn print_banner() {
    println!(
        "{}",
        "╔═══════════════════════════════════════════════════════════════╗".cyan()
    );
    println!(
        "{}",
        "║                                                               ║".cyan()
    );
    println!(
        "{}",
        "║   ██████╗ ██████╗ ██╗███╗   ██╗     ██╗███████╗ ██████╗████████╗███████╗██████╗   ║"
            .cyan()
    );
    println!(
        "{}",
        "║  ██╔════╝██╔═══██╗██║████╗  ██║     ██║██╔════╝██╔════╝╚══██╔══╝██╔════╝██╔══██╗  ║"
            .cyan()
    );
    println!(
        "{}",
        "║  ██║     ██║   ██║██║██╔██╗ ██║     ██║█████╗  ██║        ██║   █████╗  ██████╔╝  ║"
            .cyan()
    );
    println!(
        "{}",
        "║  ██║     ██║   ██║██║██║╚██╗██║██   ██║██╔══╝  ██║        ██║   ██╔══╝  ██╔══██╗  ║"
            .cyan()
    );
    println!(
        "{}",
        "║  ╚██████╗╚██████╔╝██║██║ ╚████║╚█████╔╝███████╗╚██████╗   ██║   ███████╗██║  ██║  ║"
            .cyan()
    );
    println!(
        "{}",
        "║   ╚═════╝ ╚═════╝ ╚═╝╚═╝  ╚═══╝ ╚════╝ ╚══════╝ ╚═════╝   ╚═╝   ╚══════╝╚═╝  ╚═╝  ║"
            .cyan()
    );
    println!(
        "{}",
        "║                                                               ║".cyan()
    );
    println!(
        "{}",
        "║                    Network B Wallet CLI v0.1.0                ║".cyan()
    );
    println!(
        "{}",
        "║                    NP-Hard Consensus • η=1/√2                 ║".cyan()
    );
    println!(
        "{}",
        "║                                                               ║".cyan()
    );
    println!(
        "{}",
        "╚═══════════════════════════════════════════════════════════════╝".cyan()
    );
    println!();
}
