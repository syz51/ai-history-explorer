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

use std::borrow::Cow;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::Result;
use rayon::prelude::*;

use crate::indexer::project_discovery::discover_projects;
use crate::models::{ContentBlock, EntryType, MessageContent, SearchEntry};
use crate::parsers::{parse_conversation_file, parse_history_file};
use crate::utils::strip_ansi_codes;

const ENTRY_TYPE_USER: &str = "user";
const ENTRY_TYPE_ASSISTANT: &str = "assistant";

/// Maximum bytes for thinking blocks and image alt text before truncation.
/// Keeps internal reasoning/descriptions concise for search purposes.
const MAX_THINKING_CONTENT: usize = 1024;

/// Maximum bytes for tool inputs/results before truncation.
/// Larger limit provides better context for searching tool interactions.
const MAX_TOOL_CONTENT: usize = 4096;

/// Maximum bytes for JSON serialization output before truncation.
/// Prevents DoS via deeply nested or large JSON structures in tool inputs/results.
/// Caps memory allocation during serialization.
const MAX_JSON_SERIALIZATION: usize = 4096;

/// Safely truncate string to max bytes at UTF-8 char boundary.
///
/// Prevents panics when truncating multibyte UTF-8 characters. Finds the largest
/// valid character boundary at or before `max_bytes`.
///
/// # Examples
///
/// ```ignore
/// let text = "Hello 世界";
/// assert_eq!(truncate_at_char_boundary(text, 100), "Hello 世界");
/// assert_eq!(truncate_at_char_boundary(text, 8), "Hello "); // Stops before "世" (3 bytes)
/// ```
fn truncate_at_char_boundary(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    // Find largest valid char boundary <= max_bytes
    let mut boundary = max_bytes;
    while boundary > 0 && !s.is_char_boundary(boundary) {
        boundary -= 1;
    }
    &s[..boundary]
}

/// Writer that limits output size to prevent unbounded allocations.
///
/// Used with serde_json::to_writer to cap JSON serialization output before truncation.
/// Prevents DoS via large JSON structures in tool inputs/results.
struct LimitedWriter {
    buf: String,
    limit: usize,
    truncated: bool,
}

impl LimitedWriter {
    fn new(limit: usize) -> Self {
        Self { buf: String::with_capacity(limit), limit, truncated: false }
    }

    fn into_result(self) -> (String, bool) {
        (self.buf, self.truncated)
    }
}

impl Write for LimitedWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let remaining = self.limit.saturating_sub(self.buf.len());
        if remaining == 0 {
            self.truncated = true;
            return Ok(buf.len()); // Report success but don't write
        }

        let to_write = buf.len().min(remaining);
        // Ensure we don't break UTF-8 encoding
        let valid_str = std::str::from_utf8(&buf[..to_write])
            .or_else(|e| {
                // If invalid UTF-8, truncate to last valid boundary
                let valid_up_to = e.valid_up_to();
                if valid_up_to > 0 { std::str::from_utf8(&buf[..valid_up_to]) } else { Err(e) }
            })
            .unwrap_or("");

        self.buf.push_str(valid_str);
        if to_write < buf.len() {
            self.truncated = true;
        }
        Ok(buf.len()) // Always report full write to continue serialization
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Serialize JSON value with size limit to prevent DoS.
///
/// Uses LimitedWriter to cap output size during serialization, preventing
/// large memory allocations from deeply nested or large JSON structures.
///
/// Returns the serialized string and a boolean indicating if truncation occurred.
fn serialize_json_limited(value: &serde_json::Value, limit: usize) -> (String, bool) {
    let mut writer = LimitedWriter::new(limit);
    // Compact serialization to save space
    if serde_json::to_writer(&mut writer, value).is_ok() {
        writer.into_result()
    } else {
        // Fallback if serialization fails
        ("[Serialization error]".to_string(), false)
    }
}

/// Extract text content from message content blocks.
///
/// Handles both simple string content and complex content block arrays.
/// Truncates large content blocks to prevent DoS and adds truncation indicators.
///
/// # Content Block Handling
///
/// - **Text**: Included as-is (borrowed for zero-copy)
/// - **Thinking**: Truncated to MAX_THINKING_CONTENT bytes with "[Thinking]" prefix
/// - **ToolUse**: JSON input serialized and truncated to MAX_TOOL_CONTENT with "[Tool: name]" prefix
/// - **ToolResult**: JSON content serialized and truncated to MAX_TOOL_CONTENT with "[Tool Result]" prefix
/// - **Image**: Alt text truncated to MAX_THINKING_CONTENT with "[Image]" prefix
///
/// Large content is truncated with "[truncated]" indicator for user awareness.
///
/// # Returns
///
/// Vector of text parts (using Cow for efficient memory usage). Empty vector if no text content.
fn extract_text_from_content(content: &MessageContent) -> Vec<Cow<'_, str>> {
    match content {
        MessageContent::String(s) => vec![Cow::Borrowed(s)],
        MessageContent::Array(blocks) => blocks
            .iter()
            .filter_map(|block| match block {
                ContentBlock::Text { text } => Some(Cow::Borrowed(text.as_str())),

                ContentBlock::Thinking { thinking, .. } => {
                    // Truncate large thinking blocks to prevent DoS
                    let truncated = truncate_at_char_boundary(thinking, MAX_THINKING_CONTENT);
                    if truncated.len() < thinking.len() {
                        Some(Cow::Owned(format!("[Thinking][truncated] {}...", truncated)))
                    } else {
                        Some(Cow::Owned(format!("[Thinking] {}", thinking)))
                    }
                }

                ContentBlock::ToolUse { name, input, .. } => {
                    // Serialize JSON with size limit to prevent DoS before truncation
                    let (json_str, was_truncated) =
                        serialize_json_limited(input, MAX_JSON_SERIALIZATION);

                    let content_to_display = truncate_at_char_boundary(&json_str, MAX_TOOL_CONTENT);
                    let truncated = content_to_display.len() < json_str.len() || was_truncated;

                    if truncated {
                        Some(Cow::Owned(format!(
                            "[Tool: {}][truncated] Input: {}...",
                            name, content_to_display
                        )))
                    } else {
                        Some(Cow::Owned(format!("[Tool: {}] Input: {}", name, json_str)))
                    }
                }

                ContentBlock::ToolResult { content, .. } => {
                    // Serialize JSON with size limit to prevent DoS before truncation
                    let (json_str, was_truncated) =
                        serialize_json_limited(content, MAX_JSON_SERIALIZATION);

                    let content_to_display = truncate_at_char_boundary(&json_str, MAX_TOOL_CONTENT);
                    let truncated = content_to_display.len() < json_str.len() || was_truncated;

                    if truncated {
                        Some(Cow::Owned(format!(
                            "[Tool Result][truncated] {}...",
                            content_to_display
                        )))
                    } else {
                        Some(Cow::Owned(format!("[Tool Result] {}", json_str)))
                    }
                }

                ContentBlock::Image { alt_text, .. } => {
                    // Truncate large alt_text to prevent DoS
                    alt_text.as_ref().map(|s| {
                        let truncated = truncate_at_char_boundary(s, MAX_THINKING_CONTENT);
                        if truncated.len() < s.len() {
                            Cow::Owned(format!("[Image][truncated] {}...", truncated))
                        } else {
                            Cow::Owned(format!("[Image] {}", s))
                        }
                    })
                }
            })
            .collect(),
    }
}

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
                    // Filter out whitespace-only entries (not useful for search)
                    if entry.display.trim().is_empty() {
                        continue;
                    }

                    // Validate project path to prevent path traversal and misleading paths
                    let project_path = entry.project.as_ref().and_then(|p| {
                        let path = PathBuf::from(p);
                        if !path.is_absolute() {
                            eprintln!(
                                "Warning: Skipping entry with non-absolute project path: {}",
                                p
                            );
                            return None;
                        }
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
                        display_text: strip_ansi_codes(&entry.display),
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

    // Discover projects and parse agent conversations in parallel
    match discover_projects(claude_dir) {
        Ok(projects) => {
            // Collect all (agent_file, project_path) pairs for parallel processing
            let agent_tasks: Vec<(PathBuf, PathBuf)> = projects
                .into_iter()
                .flat_map(|project| {
                    let project_path = project.decoded_path.clone();
                    project
                        .agent_files
                        .into_iter()
                        .map(move |agent_file| (agent_file, project_path.clone()))
                })
                .collect();

            // Thread-safe counters for success/failure tracking
            let success_counter = AtomicUsize::new(0);
            let failure_counter = AtomicUsize::new(0);

            // Process agent files in parallel using rayon
            let agent_entries: Vec<Vec<SearchEntry>> = agent_tasks
                .par_iter()
                .filter_map(|(agent_file, project_path)| {
                    match parse_conversation_file(agent_file) {
                        Ok(entries) => {
                            success_counter.fetch_add(1, Ordering::Relaxed);

                            // Process entries for this agent file
                            let search_entries: Vec<SearchEntry> = entries
                                .into_iter()
                                .filter_map(|entry| {
                                    // Include both user and assistant messages
                                    if entry.message.role == ENTRY_TYPE_USER
                                        || entry.message.role == ENTRY_TYPE_ASSISTANT
                                    {
                                        // Extract text from message content using helper function
                                        let text_parts =
                                            extract_text_from_content(&entry.message.content);

                                        let display_text = if !text_parts.is_empty() {
                                            // Pre-allocate capacity: sum of all text lengths + newlines
                                            let total_len: usize =
                                                text_parts.iter().map(|s| s.len()).sum();
                                            let capacity =
                                                total_len + text_parts.len().saturating_sub(1);

                                            let mut result = String::with_capacity(capacity);
                                            result.push_str(&text_parts[0]);
                                            for text in &text_parts[1..] {
                                                result.push('\n');
                                                result.push_str(text);
                                            }
                                            // Sanitize ANSI escape codes to prevent terminal injection
                                            strip_ansi_codes(&result)
                                        } else {
                                            String::new()
                                        };

                                        // Filter out entries with no text content
                                        if display_text.trim().is_empty() {
                                            return None;
                                        }

                                        // Determine entry type based on message role
                                        let entry_type =
                                            if entry.message.role == ENTRY_TYPE_ASSISTANT {
                                                EntryType::AgentMessage
                                            } else {
                                                EntryType::UserPrompt
                                            };

                                        Some(SearchEntry {
                                            entry_type,
                                            display_text,
                                            timestamp: entry.timestamp,
                                            project_path: Some(project_path.clone()),
                                            session_id: entry.session_id,
                                        })
                                    } else {
                                        None
                                    }
                                })
                                .collect();

                            Some(search_entries)
                        }
                        Err(e) => {
                            failure_counter.fetch_add(1, Ordering::Relaxed);
                            eprintln!(
                                "Warning: Failed to parse agent file {}: {}",
                                agent_file.display(),
                                e
                            );
                            None
                        }
                    }
                })
                .collect();

            // Flatten and merge all agent entries into main index
            for entries in agent_entries {
                index.extend(entries);
            }

            // Update counters from atomic values
            agent_files_success = success_counter.load(Ordering::Relaxed);
            agent_files_failed = failure_counter.load(Ordering::Relaxed);
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

        // Should have 4 entries: 2 from history + 1 user message + 1 assistant message from agent file
        assert_eq!(index.len(), 4);

        // Check sorting (newest first)
        assert_eq!(index[0].display_text, "Response");
        assert_eq!(index[1].display_text, "Agent prompt 1");
        assert_eq!(index[2].display_text, "History prompt 2");
        assert_eq!(index[3].display_text, "History prompt 1");
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
    fn test_build_index_includes_agent_messages() {
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

        // Should include both user and assistant messages
        assert_eq!(index.len(), 2);
        assert_eq!(index[0].display_text, "Assistant message");
        assert!(matches!(index[0].entry_type, EntryType::AgentMessage));
        assert_eq!(index[1].display_text, "User message");
        assert!(matches!(index[1].entry_type, EntryType::UserPrompt));
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
    fn test_build_index_rejects_relative_project_paths() {
        let claude_dir = create_test_claude_dir();

        // Create history entry with a relative project path (should be ignored)
        let history_content = r#"{"display":"Relative path","timestamp":1234567890,"sessionId":"550e8400-e29b-41d4-a716-446655440000","project":"relative/path"}"#;
        write_history_file(claude_dir.path(), history_content);

        let result = build_index(claude_dir.path());
        assert!(result.is_ok());
        let index = result.unwrap();

        assert_eq!(index.len(), 1);
        assert!(index[0].project_path.is_none(), "Relative project paths should be dropped");
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

        // Create agent file with mixed content types (image without alt_text should be filtered out)
        let agent_content = r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"Text content"},{"type":"image","source":"base64data"}]},"timestamp":1234567890,"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"uuid1"}"#;
        create_project(
            claude_dir.path(),
            "-Users%2Ftest%2Fproject",
            &[("agent-123.jsonl", agent_content)],
        );

        let result = build_index(claude_dir.path());
        assert!(result.is_ok());
        let index = result.unwrap();

        // Should only include text content (image without alt_text is filtered)
        assert_eq!(index.len(), 1);
        assert_eq!(index[0].display_text, "Text content");
    }

    #[test]
    fn test_build_index_empty_text_content() {
        let claude_dir = create_test_claude_dir();

        // Create agent file with no text content (only image without alt_text)
        let agent_content = r#"{"type":"user","message":{"role":"user","content":[{"type":"image","source":"base64data"}]},"timestamp":1234567890,"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"uuid1"}"#;
        create_project(
            claude_dir.path(),
            "-Users%2Ftest%2Fproject",
            &[("agent-123.jsonl", agent_content)],
        );

        let result = build_index(claude_dir.path());
        assert!(result.is_ok());
        let index = result.unwrap();

        // Should filter out entry with no text content (empty content filtered)
        assert_eq!(index.len(), 0);
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

    #[test]
    fn test_build_index_tool_input_unicode_truncation() {
        let claude_dir = create_test_claude_dir();

        // Create tool input that exceeds MAX_TOOL_CONTENT with multibyte Unicode char spanning boundary
        // "世" is 3 bytes in UTF-8 (0xE4 0xB8 0x96)
        let padding = "a".repeat(4093); // Close to MAX_TOOL_CONTENT limit
        let unicode_at_boundary = format!("{}{}", padding, "世界"); // Total exceeds limit

        let agent_content = format!(
            r#"{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"tool_use","id":"tool1","name":"test_tool","input":"{}"}}]}},"timestamp":1234567890,"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"uuid1"}}"#,
            unicode_at_boundary
        );

        create_project(
            claude_dir.path(),
            "-Users%2Ftest%2Fproject",
            &[("agent-123.jsonl", &agent_content)],
        );

        let result = build_index(claude_dir.path());
        // Should not panic - this is the critical test
        assert!(result.is_ok());
        let index = result.unwrap();
        assert_eq!(index.len(), 1);
        // Verify truncation occurred with proper indicator
        assert!(index[0].display_text.contains("[truncated]"));
        assert!(index[0].display_text.contains("[Tool: test_tool]"));
    }

    #[test]
    fn test_build_index_tool_result_unicode_truncation() {
        let claude_dir = create_test_claude_dir();

        // Create tool result with Unicode at truncation boundary
        let padding = "b".repeat(4093);
        let unicode_content = format!("{}{}", padding, "测试中文");

        let agent_content = format!(
            r#"{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"tool_result","tool_use_id":"tool1","content":"{}"}}]}},"timestamp":1234567890,"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"uuid1"}}"#,
            unicode_content
        );

        create_project(
            claude_dir.path(),
            "-Users%2Ftest%2Fproject",
            &[("agent-123.jsonl", &agent_content)],
        );

        let result = build_index(claude_dir.path());
        assert!(result.is_ok());
        let index = result.unwrap();
        assert_eq!(index.len(), 1);
        assert!(index[0].display_text.contains("[truncated]"));
        assert!(index[0].display_text.contains("[Tool Result]"));
    }

    #[test]
    fn test_build_index_thinking_block_unicode_truncation() {
        let claude_dir = create_test_claude_dir();

        // Create thinking block with Unicode at truncation boundary
        let padding = "t".repeat(1022);
        let unicode_thinking = format!("{}🤔", padding); // Emoji is 4 bytes

        let agent_content = format!(
            r#"{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"thinking","thinking":"{}"}}]}},"timestamp":1234567890,"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"uuid1"}}"#,
            unicode_thinking
        );

        create_project(
            claude_dir.path(),
            "-Users%2Ftest%2Fproject",
            &[("agent-123.jsonl", &agent_content)],
        );

        let result = build_index(claude_dir.path());
        assert!(result.is_ok());
        let index = result.unwrap();
        assert_eq!(index.len(), 1);
        // Should truncate and add "[truncated]"
        assert!(index[0].display_text.contains("[truncated]"));
        assert!(index[0].display_text.contains("[Thinking]"));
    }

    #[test]
    fn test_build_index_image_alt_text_unicode_truncation() {
        let claude_dir = create_test_claude_dir();

        // Create image with alt_text containing Unicode at boundary
        let padding = "i".repeat(1021);
        let unicode_alt = format!("{}日本語", padding);

        let agent_content = format!(
            r#"{{"type":"user","message":{{"role":"user","content":[{{"type":"image","source":{{"type":"base64","data":"xyz"}},"alt_text":"{}"}}]}},"timestamp":1234567890,"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"uuid1"}}"#,
            unicode_alt
        );

        create_project(
            claude_dir.path(),
            "-Users%2Ftest%2Fproject",
            &[("agent-123.jsonl", &agent_content)],
        );

        let result = build_index(claude_dir.path());
        assert!(result.is_ok());
        let index = result.unwrap();
        assert_eq!(index.len(), 1);
        assert!(index[0].display_text.contains("[truncated]"));
        assert!(index[0].display_text.contains("[Image]"));
    }

    #[test]
    fn test_build_index_no_truncation_for_short_unicode() {
        let claude_dir = create_test_claude_dir();

        // Short Unicode content should not be truncated
        let short_unicode = "Hello 世界 🌍";

        let agent_content = format!(
            r#"{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"thinking","thinking":"{}"}}]}},"timestamp":1234567890,"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"uuid1"}}"#,
            short_unicode
        );

        create_project(
            claude_dir.path(),
            "-Users%2Ftest%2Fproject",
            &[("agent-123.jsonl", &agent_content)],
        );

        let result = build_index(claude_dir.path());
        assert!(result.is_ok());
        let index = result.unwrap();
        assert_eq!(index.len(), 1);
        // Should not contain "[truncated]" since content is short
        assert!(!index[0].display_text.contains("[truncated]"));
        assert!(index[0].display_text.contains("[Thinking]"));
        assert!(index[0].display_text.contains(short_unicode));
    }

    #[test]
    fn test_build_index_large_json_dos_prevention() {
        let claude_dir = create_test_claude_dir();

        // Create large JSON with many fields (not deeply nested, which hits serde recursion limits)
        // This tests our LimitedWriter protection against wide (not deep) JSON structures
        let mut large_fields = Vec::new();
        for i in 0..500 {
            large_fields.push(format!(r#""field_{}":"value_{}""#, i, i));
        }
        let large_json = format!(r#"{{{}}}"#, large_fields.join(","));

        let agent_content = format!(
            r#"{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"tool_use","id":"tool1","name":"wide_tool","input":{}}}]}},"timestamp":1234567890,"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"uuid1"}}"#,
            large_json
        );

        create_project(
            claude_dir.path(),
            "-Users%2Ftest%2Fproject",
            &[("agent-123.jsonl", &agent_content)],
        );

        let result = build_index(claude_dir.path());
        // Should not panic or allocate excessive memory
        assert!(result.is_ok());
        let index = result.unwrap();
        assert_eq!(index.len(), 1);
        // Should be truncated due to JSON serialization limit
        assert!(index[0].display_text.contains("[Tool: wide_tool]"));
        // Display text should be bounded by our limits (MAX_TOOL_CONTENT + formatting overhead)
        // With 500 fields, serialized JSON would be ~10KB, but we limit to 4KB + overhead
        assert!(index[0].display_text.len() < 8000);
        // Should have truncation indicator since content exceeds limit
        assert!(index[0].display_text.contains("[truncated]"));
    }

    #[test]
    fn test_build_index_filters_empty_content() {
        let claude_dir = create_test_claude_dir();

        // Create entries: one with text, one empty (images only), one with text again
        let agent_content = r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"First message"}]},"timestamp":1234567890,"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"uuid1"}
{"type":"user","message":{"role":"user","content":[{"type":"image","source":"base64data"}]},"timestamp":1234567891,"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"uuid2"}
{"type":"user","message":{"role":"user","content":[{"type":"text","text":"Third message"}]},"timestamp":1234567892,"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"uuid3"}"#;

        create_project(
            claude_dir.path(),
            "-Users%2Ftest%2Fproject",
            &[("agent-123.jsonl", agent_content)],
        );

        let result = build_index(claude_dir.path());
        assert!(result.is_ok());
        let index = result.unwrap();

        // Should only include 2 entries (empty content filtered out)
        assert_eq!(index.len(), 2);
        assert_eq!(index[0].display_text, "Third message");
        assert_eq!(index[1].display_text, "First message");
    }

    #[test]
    fn test_build_index_image_with_alt_text() {
        let claude_dir = create_test_claude_dir();

        // Image with alt text should be included
        let agent_content = r#"{"type":"user","message":{"role":"user","content":[{"type":"image","source":"base64data","alt_text":"A beautiful sunset"}]},"timestamp":1234567890,"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"uuid1"}"#;

        create_project(
            claude_dir.path(),
            "-Users%2Ftest%2Fproject",
            &[("agent-123.jsonl", agent_content)],
        );

        let result = build_index(claude_dir.path());
        assert!(result.is_ok());
        let index = result.unwrap();

        // Should include entry with alt text
        assert_eq!(index.len(), 1);
        assert!(index[0].display_text.contains("[Image]"));
        assert!(index[0].display_text.contains("A beautiful sunset"));
    }

    #[test]
    fn test_limited_writer_respects_utf8_boundaries() {
        use std::io::Write;

        // Test that LimitedWriter properly handles UTF-8 boundaries
        let mut writer = LimitedWriter::new(10);

        // Write ASCII (should fit)
        writer.write_all(b"Hello").unwrap();
        assert_eq!(writer.buf, "Hello");

        // Write multibyte UTF-8 that would exceed limit
        // "世界" is 6 bytes (3 bytes each), total would be 11 bytes
        writer.write_all("世界".as_bytes()).unwrap();

        // Should not panic and should respect UTF-8 boundaries
        let (result, truncated) = writer.into_result();
        assert!(result.len() <= 10);
        assert!(truncated);
        // Result should be valid UTF-8
        assert!(std::str::from_utf8(result.as_bytes()).is_ok());
    }
}
