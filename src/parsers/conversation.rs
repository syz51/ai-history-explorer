use std::io::{BufRead, BufReader};
use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::models::ConversationEntry;
use crate::utils::safe_open_file;

/// Parse a conversation JSONL file (agent or session file)
/// Gracefully handles malformed lines by logging and skipping them
/// Returns an error if more than 50% of lines fail to parse or >100 consecutive errors
pub fn parse_conversation_file(path: &Path) -> Result<Vec<ConversationEntry>> {
    // Safely open file with TOCTOU protection and validation
    let file = safe_open_file(path)?;

    let reader = BufReader::new(file);
    let mut entries = Vec::new();
    let mut skipped_count = 0;
    let mut total_lines = 0;
    let mut consecutive_errors = 0;
    const MAX_CONSECUTIVE_ERRORS: usize = 100;

    for (line_num, line) in reader.lines().enumerate() {
        let line = line.context("Failed to read line from conversation file")?;

        // Skip empty lines
        if line.trim().is_empty() {
            continue;
        }

        total_lines += 1;

        match serde_json::from_str::<ConversationEntry>(&line) {
            Ok(entry) => {
                entries.push(entry);
                consecutive_errors = 0; // Reset on success
            }
            Err(e) => {
                eprintln!(
                    "Warning: Failed to parse line {} in {}: {}",
                    line_num + 1,
                    path.display(),
                    e
                );
                skipped_count += 1;
                consecutive_errors += 1;

                // Bail if too many consecutive errors
                if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                    bail!(
                        "Too many consecutive parse errors ({}) in {} - file may be corrupted",
                        consecutive_errors,
                        path.display()
                    );
                }
            }
        }
    }

    // Check if failure rate is too high
    if total_lines > 0 {
        let failure_rate = (skipped_count as f64) / (total_lines as f64);
        if failure_rate > 0.5 {
            bail!(
                "Too many parse failures in {}: {} of {} lines failed ({:.1}%)",
                path.display(),
                skipped_count,
                total_lines,
                failure_rate * 100.0
            );
        }
    }

    if skipped_count > 0 {
        eprintln!(
            "Parsed {}: {} entries ({} skipped)",
            path.display(),
            entries.len(),
            skipped_count
        );
    }

    Ok(entries)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use tempfile::NamedTempFile;

    use super::*;

    /// Helper to create a temporary test file with given content
    fn create_test_file(content: &str) -> NamedTempFile {
        let mut file = NamedTempFile::new().expect("Failed to create temp file");
        file.write_all(content.as_bytes()).expect("Failed to write to temp file");
        file.flush().expect("Failed to flush temp file");
        file
    }

    #[test]
    fn test_parse_valid_conversation_entries() {
        let content = r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"Hello"}]},"timestamp":1234567890,"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"550e8400-e29b-41d4-a716-446655440001"}
{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Hi there"}]},"timestamp":"2024-01-15T10:30:00Z","sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"550e8400-e29b-41d4-a716-446655440002"}"#;

        let file = create_test_file(content);
        let result = parse_conversation_file(file.path());

        assert!(result.is_ok());
        let entries = result.unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].entry_type, "user");
        assert_eq!(entries[0].message.role, "user");
        assert_eq!(entries[1].entry_type, "assistant");
    }

    #[test]
    fn test_parse_empty_conversation_file() {
        let content = "";
        let file = create_test_file(content);
        let result = parse_conversation_file(file.path());

        assert!(result.is_ok());
        let entries = result.unwrap();
        assert_eq!(entries.len(), 0);
    }

    #[test]
    fn test_parse_skips_malformed_conversation_lines() {
        let content = r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"Valid 1"}]},"timestamp":1234567890,"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"550e8400-e29b-41d4-a716-446655440001"}
invalid json line
{"type":"user","message":{"role":"user","content":[{"type":"text","text":"Valid 2"}]},"timestamp":1234567891,"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"550e8400-e29b-41d4-a716-446655440002"}"#;

        let file = create_test_file(content);
        let result = parse_conversation_file(file.path());

        assert!(result.is_ok());
        let entries = result.unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_parse_conversation_fails_with_over_50_percent_failures() {
        let content = r#"invalid line 1
{"type":"user","message":{"role":"user","content":[{"type":"text","text":"Valid"}]},"timestamp":1234567890,"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"550e8400-e29b-41d4-a716-446655440001"}
invalid line 2
invalid line 3"#;

        let file = create_test_file(content);
        let result = parse_conversation_file(file.path());

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Too many parse failures"));
    }

    #[test]
    fn test_parse_conversation_fails_with_100_consecutive_errors() {
        let mut content = String::new();
        for i in 0..101 {
            content.push_str(&format!("invalid line {}\n", i));
        }

        let file = create_test_file(&content);
        let result = parse_conversation_file(file.path());

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Too many consecutive parse errors"));
    }

    #[test]
    fn test_parse_conversation_nonexistent_file() {
        let result = parse_conversation_file(Path::new("/nonexistent/conversation.jsonl"));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Failed to open"));
    }

    #[test]
    fn test_parse_conversation_with_optional_fields() {
        let content = r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"Test"}]},"timestamp":1234567890,"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"550e8400-e29b-41d4-a716-446655440001","parent_uuid":"550e8400-e29b-41d4-a716-446655440000","is_sidechain":true}"#;

        let file = create_test_file(content);
        let result = parse_conversation_file(file.path());

        assert!(result.is_ok());
        let entries = result.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].parent_uuid,
            Some("550e8400-e29b-41d4-a716-446655440000".to_string())
        );
        assert_eq!(entries[0].is_sidechain, Some(true));
    }

    #[test]
    fn test_parse_conversation_with_string_content() {
        let content = r#"{"type":"user","message":{"role":"user","content":"Simple string content"},"timestamp":1234567890,"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"550e8400-e29b-41d4-a716-446655440001"}"#;

        let file = create_test_file(content);
        let result = parse_conversation_file(file.path());

        assert!(result.is_ok());
        let entries = result.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].entry_type, "user");
        assert_eq!(entries[0].message.role, "user");
    }

    #[test]
    fn test_parse_conversation_with_thinking_blocks() {
        let content = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"thinking","thinking":"Let me think..."},{"type":"text","text":"Here's my answer"}]},"timestamp":1234567890,"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"550e8400-e29b-41d4-a716-446655440001"}"#;

        let file = create_test_file(content);
        let result = parse_conversation_file(file.path());

        assert!(result.is_ok());
        let entries = result.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].message.role, "assistant");
    }

    #[test]
    fn test_parse_conversation_with_tool_use_blocks() {
        let content = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"tool_123","name":"read_file","input":{"path":"/test/file.txt"}}]},"timestamp":1234567890,"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"550e8400-e29b-41d4-a716-446655440001"}"#;

        let file = create_test_file(content);
        let result = parse_conversation_file(file.path());

        assert!(result.is_ok());
        let entries = result.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].message.role, "assistant");
    }

    #[test]
    fn test_parse_conversation_with_tool_result_blocks() {
        let content = r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"tool_123","content":"File contents here"}]},"timestamp":1234567890,"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"550e8400-e29b-41d4-a716-446655440001"}"#;

        let file = create_test_file(content);
        let result = parse_conversation_file(file.path());

        assert!(result.is_ok());
        let entries = result.unwrap();
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn test_parse_conversation_with_mixed_content_blocks() {
        let content = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"thinking","thinking":"Analyzing..."},{"type":"text","text":"Answer:"},{"type":"tool_use","id":"tool_456","name":"search","input":{"query":"test"}}]},"timestamp":1234567890,"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"550e8400-e29b-41d4-a716-446655440001"}"#;

        let file = create_test_file(content);
        let result = parse_conversation_file(file.path());

        assert!(result.is_ok());
        let entries = result.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].message.role, "assistant");
    }
}
