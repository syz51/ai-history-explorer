/// CLI binary integration tests using assert_cmd
///
/// These tests invoke the actual binary and verify command-line behavior
mod common;

use std::process::Command;

use assert_cmd::prelude::*;
use predicates::prelude::*;

#[test]
fn test_cli_stats_command_with_data() {
    // Create test .claude directory
    let temp_home = tempfile::TempDir::new().unwrap();
    let claude_dir = temp_home.path().join(".claude");
    std::fs::create_dir(&claude_dir).unwrap();

    // Write history file
    let history_content = r#"{"display":"Test entry 1","timestamp":1234567890,"sessionId":"550e8400-e29b-41d4-a716-446655440000"}
{"display":"Test entry 2","timestamp":1234567891,"sessionId":"550e8400-e29b-41d4-a716-446655440001"}"#;
    std::fs::write(claude_dir.join("history.jsonl"), history_content).unwrap();

    // Set HOME to our test home directory (parent of .claude)
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ai-history-explorer"));
    cmd.env("HOME", temp_home.path())
        .arg("stats")
        .assert()
        .success()
        .stdout(predicate::str::contains("Claude Code History Statistics"))
        .stdout(predicate::str::contains("Total entries: 2"))
        .stdout(predicate::str::contains("User prompts: 2"));
}

#[test]
fn test_cli_stats_command_empty_directory() {
    // Create empty .claude directory
    let temp_home = tempfile::TempDir::new().unwrap();
    let claude_dir = temp_home.path().join(".claude");
    std::fs::create_dir(&claude_dir).unwrap();
    std::fs::write(claude_dir.join("history.jsonl"), "").unwrap();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ai-history-explorer"));
    cmd.env("HOME", temp_home.path())
        .arg("stats")
        .assert()
        .success()
        .stdout(predicate::str::contains("Total entries: 0"));
}

#[test]
fn test_cli_no_command_shows_help_message() {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ai-history-explorer"));
    cmd.assert().success().stdout(predicate::str::contains("Use --help for usage information"));
}

#[test]
fn test_cli_help_flag() {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ai-history-explorer"));
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Search through Claude Code conversation history"))
        .stdout(predicate::str::contains("stats"));
}

#[test]
fn test_cli_version_flag() {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ai-history-explorer"));
    cmd.arg("--version").assert().success().stdout(predicate::str::contains("0.1.0"));
}

#[test]
fn test_cli_invalid_command() {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ai-history-explorer"));
    cmd.arg("invalid-command").assert().failure(); // Should fail with invalid command
}

#[test]
fn test_cli_stats_with_missing_claude_dir() {
    // Set HOME to a directory without .claude
    let temp_dir = tempfile::TempDir::new().unwrap();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ai-history-explorer"));
    cmd.env("HOME", temp_dir.path())
        .arg("stats")
        .assert()
        .success() // Should succeed with warning and empty results
        .stdout(predicate::str::contains("Total entries: 0"))
        .stderr(predicate::str::contains("history.jsonl not found"));
}

#[test]
fn test_cli_stats_with_corrupted_history() {
    // Create .claude directory with severely corrupted history (>50% bad)
    let history_content = r#"invalid line 1
invalid line 2
invalid line 3
{"display":"Valid","timestamp":1000,"sessionId":"550e8400-e29b-41d4-a716-446655440000"}"#;

    let temp_home = tempfile::TempDir::new().unwrap();
    let claude_dir = temp_home.path().join(".claude");
    std::fs::create_dir(&claude_dir).unwrap();
    std::fs::write(claude_dir.join("history.jsonl"), history_content).unwrap();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ai-history-explorer"));
    cmd.env("HOME", temp_home.path())
        .arg("stats")
        .assert()
        .success() // Graceful degradation - continues with warning
        .stdout(predicate::str::contains("Total entries: 0"))
        .stderr(predicate::str::contains("Too many parse failures"));
}

#[test]
fn test_cli_stats_with_partial_corruption() {
    // Create .claude directory with some corrupted lines (< 50% bad)
    let history_content = r#"{"display":"Valid 1","timestamp":1000,"sessionId":"550e8400-e29b-41d4-a716-446655440000"}
invalid line
{"display":"Valid 2","timestamp":2000,"sessionId":"550e8400-e29b-41d4-a716-446655440001"}"#;

    let temp_home = tempfile::TempDir::new().unwrap();
    let claude_dir = temp_home.path().join(".claude");
    std::fs::create_dir(&claude_dir).unwrap();
    std::fs::write(claude_dir.join("history.jsonl"), history_content).unwrap();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ai-history-explorer"));
    cmd.env("HOME", temp_home.path())
        .arg("stats")
        .assert()
        .success() // Should succeed with partial corruption
        .stdout(predicate::str::contains("Total entries: 2"));
}
