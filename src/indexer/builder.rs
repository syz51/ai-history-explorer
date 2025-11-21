//! Index builder for Claude Code conversation history.
//!
//! # Error Handling Strategy
//!
//! This module follows a **graceful degradation** approach suitable for CLI tools:
//!
//! - **File-level errors**: Missing files (history.jsonl, agent files) are logged as warnings
//!   but don't fail the entire operation, allowing partial index building
//! - **Parse-level errors**: Malformed lines/entries are skipped with warnings, tracked by parsers
//! - **Failure thresholds**: Operations fail if >50% of items fail (parsers, agent files)
//! - **User feedback**: Summary statistics printed at end showing success/warning/failure counts
//!
//! This approach balances robustness (handles corrupted files) with reliability (fails on
//! systematic issues). Errors are reported via stderr (eprintln!) and critical failures
//! propagated via Result types.

use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::indexer::project_discovery::discover_projects;
use crate::models::{EntryType, SearchEntry};
use crate::parsers::{parse_conversation_file, parse_history_file};

const ENTRY_TYPE_USER: &str = "user";
const CONTENT_TYPE_TEXT: &str = "text";

/// Build unified index from user prompts and agent messages
///
/// Creates a searchable index by combining:
/// 1. User prompts from history.jsonl
/// 2. User messages from agent conversation files across all projects
///
/// The resulting index is sorted by timestamp (newest first) and includes metadata
/// like project paths and session IDs for each entry.
///
/// # Arguments
///
/// * `claude_dir` - Path to the ~/.claude directory
///
/// # Returns
///
/// Returns a Vec of [`SearchEntry`] sorted by timestamp (newest first).
///
/// # Errors
///
/// Returns an error if:
/// - More than 50% of agent files fail to parse (systematic corruption)
/// - File size validation fails (files >10MB)
/// - Parser error thresholds are exceeded (>50% lines fail or >100 consecutive errors)
///
/// Individual missing files (history.jsonl) or failed agent files are logged as warnings
/// and don't fail the entire operation, allowing partial index building.
///
/// # Examples
///
/// ```no_run
/// use std::path::PathBuf;
/// use ai_history_explorer::build_index;
///
/// let claude_dir = PathBuf::from("/Users/alice/.claude");
/// let index = build_index(&claude_dir)?;
/// println!("Indexed {} entries", index.len());
/// # Ok::<(), anyhow::Error>(())
/// ```
pub fn build_index(claude_dir: &Path) -> Result<Vec<SearchEntry>> {
    let mut index = Vec::new();
    let mut agent_files_success = 0;
    let mut agent_files_failed = 0;

    // Parse user prompts from history.jsonl
    let history_path = claude_dir.join("history.jsonl");
    if history_path.exists() {
        match parse_history_file(&history_path) {
            Ok(entries) => {
                for entry in entries {
                    // Validate project path to prevent path traversal
                    let project_path = entry.project.as_ref().and_then(|p| {
                        let path = PathBuf::from(p);
                        // Reject paths with .. components
                        if path.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
                            eprintln!(
                                "Warning: Skipping entry with suspicious project path: {}",
                                p
                            );
                            return None;
                        }
                        Some(path)
                    });
                    index.push(SearchEntry {
                        entry_type: EntryType::UserPrompt,
                        display_text: entry.display,
                        timestamp: entry.timestamp,
                        project_path,
                        session_id: entry.session_id,
                    });
                }
            }
            Err(e) => {
                eprintln!("Warning: Failed to parse history file: {}", e);
            }
        }
    } else {
        eprintln!("Warning: history.jsonl not found at {}", history_path.display());
    }

    // Discover projects and parse agent conversations
    match discover_projects(claude_dir) {
        Ok(projects) => {
            for project in projects {
                for agent_file in project.agent_files {
                    match parse_conversation_file(&agent_file) {
                        Ok(entries) => {
                            agent_files_success += 1;
                            for entry in entries {
                                // Only include user messages from agent conversations
                                if entry.entry_type == ENTRY_TYPE_USER {
                                    // Extract text from message content (optimized with capacity pre-allocation)
                                    let text_parts: Vec<&str> = entry
                                        .message
                                        .content
                                        .iter()
                                        .filter(|c| c.content_type == CONTENT_TYPE_TEXT)
                                        .filter_map(|c| c.text.as_deref())
                                        .collect();

                                    let display_text = if !text_parts.is_empty() {
                                        // Pre-allocate capacity: sum of all text lengths + newlines between them
                                        let total_len: usize =
                                            text_parts.iter().map(|s| s.len()).sum();
                                        let capacity =
                                            total_len + text_parts.len().saturating_sub(1); // +1 for each newline

                                        let mut result = String::with_capacity(capacity);
                                        result.push_str(text_parts[0]);
                                        for text in &text_parts[1..] {
                                            result.push('\n');
                                            result.push_str(text);
                                        }
                                        result
                                    } else {
                                        String::new()
                                    };

                                    index.push(SearchEntry {
                                        entry_type: EntryType::UserPrompt,
                                        display_text,
                                        timestamp: entry.timestamp,
                                        project_path: Some(project.decoded_path.clone()),
                                        session_id: entry.session_id,
                                    });
                                }
                            }
                        }
                        Err(e) => {
                            agent_files_failed += 1;
                            eprintln!(
                                "Warning: Failed to parse agent file {}: {}",
                                agent_file.display(),
                                e
                            );
                        }
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("Warning: Failed to discover projects: {}", e);
        }
    }

    // Check error rate and fail if >50% of agent files failed
    let total_agent_files = agent_files_success + agent_files_failed;
    if total_agent_files > 0 {
        let failure_rate = agent_files_failed as f64 / total_agent_files as f64;
        if failure_rate > 0.5 {
            anyhow::bail!(
                "Index building failed: {}/{} agent files failed to parse ({}% failure rate)",
                agent_files_failed,
                total_agent_files,
                (failure_rate * 100.0) as u32
            );
        }
    }

    // Print summary statistics
    eprintln!(
        "Indexed {} entries ({} agent files parsed, {} failed)",
        index.len(),
        agent_files_success,
        agent_files_failed
    );

    // Sort by timestamp (newest first)
    index.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    Ok(index)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;

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

    /// Helper to create a project directory with agent files
    fn create_project(
        claude_dir: &Path,
        encoded_name: &str,
        agent_files: &[(&str, &str)],
    ) -> PathBuf {
        let projects_dir = claude_dir.join("projects");
        fs::create_dir_all(&projects_dir).expect("Failed to create projects dir");

        let project_dir = projects_dir.join(encoded_name);
        fs::create_dir(&project_dir).expect("Failed to create project dir");

        for (filename, content) in agent_files {
            let file_path = project_dir.join(filename);
            let mut file = fs::File::create(file_path).expect("Failed to create agent file");
            file.write_all(content.as_bytes()).expect("Failed to write agent file");
        }

        project_dir
    }

    #[test]
    fn test_build_index_with_valid_data() {
        let claude_dir = create_test_claude_dir();

        // Create history.jsonl
        let history_content = r#"{"display":"History prompt 1","timestamp":1234567890,"sessionId":"550e8400-e29b-41d4-a716-446655440000"}
{"display":"History prompt 2","timestamp":1234567891,"sessionId":"550e8400-e29b-41d4-a716-446655440001"}"#;
        write_history_file(claude_dir.path(), history_content);

        // Create project with agent file
        let agent_content = r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"Agent prompt 1"}]},"timestamp":1234567892,"sessionId":"550e8400-e29b-41d4-a716-446655440002","uuid":"uuid1"}
{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Response"}]},"timestamp":1234567893,"sessionId":"550e8400-e29b-41d4-a716-446655440002","uuid":"uuid2"}"#;
        create_project(
            claude_dir.path(),
            "-Users%2Ftest%2Fproject",
            &[("agent-123.jsonl", agent_content)],
        );

        let result = build_index(claude_dir.path());
        assert!(result.is_ok());
        let index = result.unwrap();

        // Should have 3 entries: 2 from history + 1 user message from agent file
        assert_eq!(index.len(), 3);

        // Check sorting (newest first)
        assert_eq!(index[0].display_text, "Agent prompt 1");
        assert_eq!(index[1].display_text, "History prompt 2");
        assert_eq!(index[2].display_text, "History prompt 1");
    }

    #[test]
    fn test_build_index_with_missing_history() {
        let claude_dir = create_test_claude_dir();

        // Create project with agent file (no history.jsonl)
        let agent_content = r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"Agent prompt"}]},"timestamp":1234567890,"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"uuid1"}"#;
        create_project(
            claude_dir.path(),
            "-Users%2Ftest%2Fproject",
            &[("agent-123.jsonl", agent_content)],
        );

        let result = build_index(claude_dir.path());
        assert!(result.is_ok());
        let index = result.unwrap();

        // Should still work with just agent files
        assert_eq!(index.len(), 1);
        assert_eq!(index[0].display_text, "Agent prompt");
    }

    #[test]
    fn test_build_index_with_missing_projects() {
        let claude_dir = create_test_claude_dir();

        // Only create history.jsonl (no projects directory)
        let history_content = r#"{"display":"History prompt","timestamp":1234567890,"sessionId":"550e8400-e29b-41d4-a716-446655440000"}"#;
        write_history_file(claude_dir.path(), history_content);

        let result = build_index(claude_dir.path());
        assert!(result.is_ok());
        let index = result.unwrap();

        // Should work with just history
        assert_eq!(index.len(), 1);
        assert_eq!(index[0].display_text, "History prompt");
    }

    #[test]
    fn test_build_index_empty_data() {
        let claude_dir = create_test_claude_dir();

        // Create empty history.jsonl
        write_history_file(claude_dir.path(), "");

        // Create empty projects directory
        fs::create_dir(claude_dir.path().join("projects")).expect("Failed to create projects dir");

        let result = build_index(claude_dir.path());
        assert!(result.is_ok());
        let index = result.unwrap();

        // Should return empty index
        assert_eq!(index.len(), 0);
    }

    #[test]
    fn test_build_index_filters_agent_messages() {
        let claude_dir = create_test_claude_dir();

        // Create agent file with both user and assistant messages
        let agent_content = r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"User message"}]},"timestamp":1234567890,"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"uuid1"}
{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Assistant message"}]},"timestamp":1234567891,"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"uuid2"}"#;
        create_project(
            claude_dir.path(),
            "-Users%2Ftest%2Fproject",
            &[("agent-123.jsonl", agent_content)],
        );

        let result = build_index(claude_dir.path());
        assert!(result.is_ok());
        let index = result.unwrap();

        // Should only include user message, not assistant
        assert_eq!(index.len(), 1);
        assert_eq!(index[0].display_text, "User message");
        assert!(matches!(index[0].entry_type, EntryType::UserPrompt));
    }

    #[test]
    fn test_build_index_fails_with_over_50_percent_agent_failures() {
        let claude_dir = create_test_claude_dir();

        // Create 3 agent files: 1 valid, 2 invalid (66% failure rate)
        let valid_content = r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"Valid"}]},"timestamp":1234567890,"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"uuid1"}"#;
        let invalid_content = "invalid json content";

        create_project(
            claude_dir.path(),
            "-Users%2Ftest%2Fproject1",
            &[("agent-123.jsonl", valid_content)],
        );
        create_project(
            claude_dir.path(),
            "-Users%2Ftest%2Fproject2",
            &[("agent-456.jsonl", invalid_content)],
        );
        create_project(
            claude_dir.path(),
            "-Users%2Ftest%2Fproject3",
            &[("agent-789.jsonl", invalid_content)],
        );

        let result = build_index(claude_dir.path());
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Index building failed"));
        assert!(err.to_string().contains("66%"));
    }

    #[test]
    fn test_build_index_succeeds_with_under_50_percent_agent_failures() {
        let claude_dir = create_test_claude_dir();

        // Create 3 agent files: 2 valid, 1 invalid (33% failure rate)
        let valid_content = r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"Valid"}]},"timestamp":1234567890,"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"uuid1"}"#;
        let invalid_content = "invalid json content";

        create_project(
            claude_dir.path(),
            "-Users%2Ftest%2Fproject1",
            &[("agent-123.jsonl", valid_content)],
        );
        create_project(
            claude_dir.path(),
            "-Users%2Ftest%2Fproject2",
            &[("agent-456.jsonl", valid_content)],
        );
        create_project(
            claude_dir.path(),
            "-Users%2Ftest%2Fproject3",
            &[("agent-789.jsonl", invalid_content)],
        );

        let result = build_index(claude_dir.path());
        assert!(result.is_ok());
        let index = result.unwrap();

        // Should succeed with 2 valid entries
        assert_eq!(index.len(), 2);
    }

    #[test]
    fn test_build_index_rejects_path_traversal() {
        let claude_dir = create_test_claude_dir();

        // Create history entry with path traversal attempt
        let history_content = r#"{"display":"Malicious prompt","timestamp":1234567890,"sessionId":"550e8400-e29b-41d4-a716-446655440000","project":"/Users/test/../etc/passwd"}"#;
        write_history_file(claude_dir.path(), history_content);

        let result = build_index(claude_dir.path());
        assert!(result.is_ok());
        let index = result.unwrap();

        // Entry should be included but project_path should be None (filtered out)
        assert_eq!(index.len(), 1);
        assert_eq!(index[0].display_text, "Malicious prompt");
        assert!(index[0].project_path.is_none());
    }

    #[test]
    fn test_build_index_timestamp_sorting() {
        let claude_dir = create_test_claude_dir();

        // Create entries with timestamps in non-sorted order
        let history_content = r#"{"display":"Middle entry","timestamp":1234567891,"sessionId":"550e8400-e29b-41d4-a716-446655440000"}
{"display":"Oldest entry","timestamp":1234567890,"sessionId":"550e8400-e29b-41d4-a716-446655440001"}
{"display":"Newest entry","timestamp":1234567892,"sessionId":"550e8400-e29b-41d4-a716-446655440002"}"#;
        write_history_file(claude_dir.path(), history_content);

        let result = build_index(claude_dir.path());
        assert!(result.is_ok());
        let index = result.unwrap();

        // Should be sorted newest first
        assert_eq!(index.len(), 3);
        assert_eq!(index[0].display_text, "Newest entry");
        assert_eq!(index[1].display_text, "Middle entry");
        assert_eq!(index[2].display_text, "Oldest entry");
    }

    #[test]
    fn test_build_index_multiple_text_content_parts() {
        let claude_dir = create_test_claude_dir();

        // Create agent file with multiple text content parts
        let agent_content = r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"Part 1"},{"type":"text","text":"Part 2"},{"type":"text","text":"Part 3"}]},"timestamp":1234567890,"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"uuid1"}"#;
        create_project(
            claude_dir.path(),
            "-Users%2Ftest%2Fproject",
            &[("agent-123.jsonl", agent_content)],
        );

        let result = build_index(claude_dir.path());
        assert!(result.is_ok());
        let index = result.unwrap();

        // Should join text parts with newlines
        assert_eq!(index.len(), 1);
        assert_eq!(index[0].display_text, "Part 1\nPart 2\nPart 3");
    }

    #[test]
    fn test_build_index_non_text_content_filtered() {
        let claude_dir = create_test_claude_dir();

        // Create agent file with mixed content types
        let agent_content = r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"Text content"},{"type":"image","source":"base64data"}]},"timestamp":1234567890,"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"uuid1"}"#;
        create_project(
            claude_dir.path(),
            "-Users%2Ftest%2Fproject",
            &[("agent-123.jsonl", agent_content)],
        );

        let result = build_index(claude_dir.path());
        assert!(result.is_ok());
        let index = result.unwrap();

        // Should only include text content
        assert_eq!(index.len(), 1);
        assert_eq!(index[0].display_text, "Text content");
    }

    #[test]
    fn test_build_index_empty_text_content() {
        let claude_dir = create_test_claude_dir();

        // Create agent file with no text content
        let agent_content = r#"{"type":"user","message":{"role":"user","content":[{"type":"image","source":"base64data"}]},"timestamp":1234567890,"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"uuid1"}"#;
        create_project(
            claude_dir.path(),
            "-Users%2Ftest%2Fproject",
            &[("agent-123.jsonl", agent_content)],
        );

        let result = build_index(claude_dir.path());
        assert!(result.is_ok());
        let index = result.unwrap();

        // Should include entry with empty display_text
        assert_eq!(index.len(), 1);
        assert_eq!(index[0].display_text, "");
    }

    #[test]
    fn test_build_index_multiple_projects() {
        let claude_dir = create_test_claude_dir();

        // Create multiple projects with agent files
        let agent_content1 = r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"Project 1"}]},"timestamp":1234567890,"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"uuid1"}"#;
        let agent_content2 = r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"Project 2"}]},"timestamp":1234567891,"sessionId":"550e8400-e29b-41d4-a716-446655440001","uuid":"uuid2"}"#;

        create_project(
            claude_dir.path(),
            "-Users%2Ftest%2Fproject1",
            &[("agent-123.jsonl", agent_content1)],
        );
        create_project(
            claude_dir.path(),
            "-Users%2Ftest%2Fproject2",
            &[("agent-456.jsonl", agent_content2)],
        );

        let result = build_index(claude_dir.path());
        assert!(result.is_ok());
        let index = result.unwrap();

        // Should have entries from both projects
        assert_eq!(index.len(), 2);
    }
}
