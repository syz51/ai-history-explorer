use std::path::Path;

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
    show_stats_impl(None)
}

// Internal implementation that allows passing in a custom claude_dir for testing
#[cfg(not(test))]
fn show_stats_impl(_claude_dir_override: Option<&Path>) -> Result<()> {
    let claude_dir = get_claude_dir()?;
    let index = build_index(&claude_dir)?;
    print_stats(&index, &claude_dir);
    Ok(())
}

#[cfg(test)]
fn show_stats_impl(claude_dir_override: Option<&Path>) -> Result<()> {
    let claude_dir =
        if let Some(dir) = claude_dir_override { dir.to_path_buf() } else { get_claude_dir()? };
    let index = build_index(&claude_dir)?;
    print_stats(&index, &claude_dir);
    Ok(())
}

fn print_stats(index: &[crate::models::SearchEntry], claude_dir: &Path) {
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
    println!("Claude directory: {}", format_path_with_tilde(claude_dir));

    if let Some(oldest) = index.last() {
        println!("Oldest entry: {}", oldest.timestamp.format("%Y-%m-%d %H:%M:%S"));
    }
    if let Some(newest) = index.first() {
        println!("Newest entry: {}", newest.timestamp.format("%Y-%m-%d %H:%M:%S"));
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::path::{Path, PathBuf};
    use std::{env, fs};

    use tempfile::TempDir;

    use super::*;

    /// Helper to create a test .claude directory structure
    fn create_test_claude_dir() -> TempDir {
        TempDir::new().expect("Failed to create temp dir")
    }

    /// Helper to write content to history.jsonl
    fn write_history_file(claude_dir: &Path, content: &str) {
        let history_path = claude_dir.join("history.jsonl");
        let mut file = fs::File::create(history_path).expect("Failed to create history.jsonl");
        file.write_all(content.as_bytes()).expect("Failed to write history.jsonl");
    }

    #[test]
    fn test_show_stats_with_valid_data() {
        let claude_dir = create_test_claude_dir();

        // Create test data
        let history_content = r#"{"display":"Test prompt 1","timestamp":1234567890,"sessionId":"550e8400-e29b-41d4-a716-446655440000"}
{"display":"Test prompt 2","timestamp":1234567891,"sessionId":"550e8400-e29b-41d4-a716-446655440001"}"#;
        write_history_file(claude_dir.path(), history_content);

        let result = show_stats_impl(Some(claude_dir.path()));
        assert!(result.is_ok());
    }

    #[test]
    fn test_show_stats_with_empty_index() {
        let claude_dir = create_test_claude_dir();

        // Create empty history.jsonl
        write_history_file(claude_dir.path(), "");

        let result = show_stats_impl(Some(claude_dir.path()));
        assert!(result.is_ok());
    }

    #[test]
    fn test_show_stats_with_missing_claude_dir() {
        // Save original HOME value
        let original_home = env::var("HOME").ok();

        // SAFETY: Setting environment variables in tests is safe as long as:
        // 1. Tests don't run in parallel (cargo test runs them in parallel by default, but we restore the value)
        // 2. No other threads are reading this variable concurrently
        // 3. We restore the original value afterwards
        unsafe {
            env::set_var("HOME", "/nonexistent/directory");
        }

        let result = show_stats_impl(None);
        // Should propagate error from get_claude_dir or build_index
        // The exact error depends on whether .claude exists

        // Restore original HOME
        if let Some(home) = original_home {
            unsafe {
                env::set_var("HOME", home);
            }
        }

        // Don't assert specific error since we don't control the environment
        // Just verify it doesn't panic
        let _ = result;
    }

    #[test]
    fn test_print_stats_formats_output() {
        // Create sample index data
        use chrono::{TimeZone, Utc};
        let entries = vec![
            crate::models::SearchEntry {
                entry_type: EntryType::UserPrompt,
                display_text: "Test 1".to_string(),
                timestamp: Utc.timestamp_opt(1234567892, 0).unwrap(),
                project_path: None,
                session_id: "session1".to_string(),
            },
            crate::models::SearchEntry {
                entry_type: EntryType::UserPrompt,
                display_text: "Test 2".to_string(),
                timestamp: Utc.timestamp_opt(1234567890, 0).unwrap(),
                project_path: None,
                session_id: "session2".to_string(),
            },
        ];

        let claude_dir = PathBuf::from("/Users/test/.claude");

        // Just verify it doesn't panic
        print_stats(&entries, &claude_dir);
    }

    #[test]
    fn test_print_stats_empty_index() {
        let entries = vec![];
        let claude_dir = PathBuf::from("/Users/test/.claude");

        // Just verify it doesn't panic with empty index
        print_stats(&entries, &claude_dir);
    }
}
