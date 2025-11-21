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
