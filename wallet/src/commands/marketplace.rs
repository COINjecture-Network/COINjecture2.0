// Marketplace commands

use crate::rpc_client::RpcClient;
use anyhow::Result;
use colored::*;

/// List open problems in marketplace
pub async fn list_problems(client: &RpcClient) -> Result<()> {
    println!("{}", "Querying marketplace...".dimmed());

    match client.get_open_problems().await {
        Ok(problems) => {
            println!();
            if problems.is_empty() {
                println!("{}", "No open problems in marketplace".yellow());
                return Ok(());
            }

            println!(
                "{}",
                format!("Found {} open problem(s)", problems.len())
                    .green()
                    .bold()
            );
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!();

            for (i, problem) in problems.iter().enumerate() {
                println!("{}. Problem #{}", i + 1, &problem.problem_id[0..16].dimmed());
                println!("   Submitter:  {}", problem.submitter);
                println!("   Bounty:     {} tokens", format_balance(problem.bounty));
                println!("   Min Score:  {:.4}", problem.min_work_score);
                println!("   Status:     {}", format_status(&problem.status));
                println!("   Submitted:  {}", format_timestamp(problem.submitted_at));
                println!("   Expires:    {}", format_timestamp(problem.expires_at));
                println!();
            }
        }
        Err(e) => {
            println!("{}", format!("❌ Failed to query marketplace: {}", e).red());
        }
    }

    Ok(())
}

/// Get marketplace statistics
pub async fn get_stats(client: &RpcClient) -> Result<()> {
    println!("{}", "Querying marketplace statistics...".dimmed());

    match client.get_marketplace_stats().await {
        Ok(stats) => {
            println!();
            println!("{}", "Marketplace Statistics".green().bold());
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("{}", serde_json::to_string_pretty(&stats)?);
        }
        Err(e) => {
            println!("{}", format!("❌ Failed to get marketplace stats: {}", e).red());
        }
    }

    Ok(())
}

/// Get problem details by ID
pub async fn get_problem(problem_id: &str, client: &RpcClient) -> Result<()> {
    let problem_id = problem_id.trim_start_matches("0x");

    println!("{}", "Querying problem details...".dimmed());

    match client.get_problem(problem_id).await {
        Ok(Some(problem)) => {
            println!();
            println!("{}", "Problem Details".cyan().bold());
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("Problem ID:  {}", problem.problem_id);
            println!("Submitter:   {}", problem.submitter);
            println!("Bounty:      {} tokens", format_balance(problem.bounty));
            println!("Min Score:   {:.4}", problem.min_work_score);
            println!("Status:      {}", format_status(&problem.status));
            println!("Submitted:   {}", format_timestamp(problem.submitted_at));
            println!("Expires:     {}", format_timestamp(problem.expires_at));
        }
        Ok(None) => {
            println!("{}", "Problem not found".yellow());
        }
        Err(e) => {
            println!("{}", format!("❌ Failed to get problem details: {}", e).red());
        }
    }

    Ok(())
}

/// Submit a new problem to marketplace (TODO: Implement problem creation)
pub async fn submit_problem(
    problem_type: &str,
    bounty: u128,
    _client: &RpcClient,
) -> Result<()> {
    println!();
    println!("{}", "⚠️  Problem Submission Not Yet Implemented".yellow().bold());
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Problem Type: {}", problem_type);
    println!("Bounty:       {} tokens", format_balance(bounty));
    println!();
    println!("This feature requires:");
    println!("  • Problem generation for {}", problem_type);
    println!("  • Transaction signing and submission");
    println!("  • Problem marketplace integration");
    println!();
    println!("Coming in a future update!");

    Ok(())
}

// Helper functions

fn format_balance(balance: u128) -> String {
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

fn format_timestamp(timestamp: i64) -> String {
    use chrono::{DateTime, Utc};
    let dt = DateTime::<Utc>::from_timestamp(timestamp, 0).unwrap_or_else(|| Utc::now());
    dt.format("%Y-%m-%d %H:%M:%S UTC").to_string()
}

fn format_status(status: &str) -> colored::ColoredString {
    match status {
        "Open" => status.green(),
        "Claimed" => status.yellow(),
        "Expired" => status.red(),
        _ => status.normal(),
    }
}
