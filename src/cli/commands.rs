use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::indexer::build_index;
use crate::models::EntryType;
use crate::utils::{format_path_with_tilde, get_claude_dir};

#[derive(Parser)]
#[command(name = "ai-history-explorer")]
#[command(version = "0.1.0")]
#[command(about = "Search through Claude Code conversation history", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Show statistics about the history
    Stats,
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::Stats) => {
            show_stats()?;
        }
        None => {
            println!("Use --help for usage information");
        }
    }

    Ok(())
}

fn show_stats() -> Result<()> {
    let claude_dir = get_claude_dir()?;
    let index = build_index(&claude_dir)?;

    let user_prompts =
        index.iter().filter(|e| matches!(e.entry_type, EntryType::UserPrompt)).count();
    let agent_messages =
        index.iter().filter(|e| matches!(e.entry_type, EntryType::AgentMessage)).count();

    println!("Claude Code History Statistics");
    println!("================================");
    println!("Total entries: {}", index.len());
    println!("  User prompts: {}", user_prompts);
    println!("  Agent messages: {}", agent_messages);
    println!();
    println!("Claude directory: {}", format_path_with_tilde(&claude_dir));

    if let Some(oldest) = index.last() {
        println!("Oldest entry: {}", oldest.timestamp.format("%Y-%m-%d %H:%M:%S"));
    }
    if let Some(newest) = index.first() {
        println!("Newest entry: {}", newest.timestamp.format("%Y-%m-%d %H:%M:%S"));
    }

    Ok(())
}
