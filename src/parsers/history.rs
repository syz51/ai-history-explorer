use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::models::HistoryEntry;
use crate::utils::validate_file_size;

/// Parse history.jsonl file and return list of history entries
/// Gracefully handles malformed lines by logging and skipping them
/// Returns an error if more than 50% of lines fail to parse or >100 consecutive errors
pub fn parse_history_file(path: &Path) -> Result<Vec<HistoryEntry>> {
    // Open file and validate size to avoid TOCTOU race condition
    let file = File::open(path)
        .with_context(|| format!("Failed to open history file: {}", path.display()))?;
    validate_file_size(&file, path)?;

    let reader = BufReader::new(file);
    let mut entries = Vec::new();
    let mut skipped_count = 0;
    let mut total_lines = 0;
    let mut consecutive_errors = 0;
    const MAX_CONSECUTIVE_ERRORS: usize = 100;

    for (line_num, line) in reader.lines().enumerate() {
        let line = line.context("Failed to read line from history file")?;

        // Skip empty lines
        if line.trim().is_empty() {
            continue;
        }

        total_lines += 1;

        match serde_json::from_str::<HistoryEntry>(&line) {
            Ok(entry) => {
                entries.push(entry);
                consecutive_errors = 0; // Reset on success
            }
            Err(e) => {
                eprintln!("Warning: Failed to parse line {} in history file: {}", line_num + 1, e);
                skipped_count += 1;
                consecutive_errors += 1;

                // Bail if too many consecutive errors
                if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                    bail!(
                        "Too many consecutive parse errors ({}) in history file - file may be corrupted",
                        consecutive_errors
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
                "Too many parse failures in history file: {} of {} lines failed ({:.1}%)",
                skipped_count,
                total_lines,
                failure_rate * 100.0
            );
        }
    }

    if skipped_count > 0 {
        eprintln!("Parsed history file: {} entries ({} skipped)", entries.len(), skipped_count);
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
    fn test_parse_valid_history_entries() {
        let content = r#"{"display":"Hello, world!","timestamp":1234567890,"sessionId":"550e8400-e29b-41d4-a716-446655440000"}
{"display":"Test message","timestamp":"2024-01-15T10:30:00Z","sessionId":"550e8400-e29b-41d4-a716-446655440001"}
{"display":"Another message","timestamp":1234567891,"sessionId":"550e8400-e29b-41d4-a716-446655440002"}"#;

        let file = create_test_file(content);
        let result = parse_history_file(file.path());

        assert!(result.is_ok());
        let entries = result.unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].display, "Hello, world!");
        assert_eq!(entries[1].display, "Test message");
        assert_eq!(entries[1].session_id, "550e8400-e29b-41d4-a716-446655440001");
        assert_eq!(entries[2].display, "Another message");
    }

    #[test]
    fn test_parse_empty_file() {
        let content = "";
        let file = create_test_file(content);
        let result = parse_history_file(file.path());

        assert!(result.is_ok());
        let entries = result.unwrap();
        assert_eq!(entries.len(), 0);
    }

    #[test]
    fn test_parse_file_with_only_empty_lines() {
        let content = "\n\n  \n\t\n";
        let file = create_test_file(content);
        let result = parse_history_file(file.path());

        assert!(result.is_ok());
        let entries = result.unwrap();
        assert_eq!(entries.len(), 0);
    }

    #[test]
    fn test_parse_skips_malformed_lines() {
        let content = r#"{"display":"Valid entry 1","timestamp":1234567890,"sessionId":"550e8400-e29b-41d4-a716-446655440000"}
invalid json line
{"display":"Valid entry 2","timestamp":1234567891,"sessionId":"550e8400-e29b-41d4-a716-446655440001"}
{"incomplete": "entry"
{"display":"Valid entry 3","timestamp":1234567892,"sessionId":"550e8400-e29b-41d4-a716-446655440002"}"#;

        let file = create_test_file(content);
        let result = parse_history_file(file.path());

        assert!(result.is_ok());
        let entries = result.unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].display, "Valid entry 1");
        assert_eq!(entries[1].display, "Valid entry 2");
        assert_eq!(entries[2].display, "Valid entry 3");
    }

    #[test]
    fn test_parse_fails_with_over_50_percent_failures() {
        // 2 valid, 3 invalid = 60% failure rate
        let content = r#"invalid line 1
{"display":"Valid","timestamp":1234567890,"sessionId":"550e8400-e29b-41d4-a716-446655440000"}
invalid line 2
invalid line 3
{"display":"Valid 2","timestamp":1234567891,"sessionId":"550e8400-e29b-41d4-a716-446655440001"}"#;

        let file = create_test_file(content);
        let result = parse_history_file(file.path());

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Too many parse failures"));
    }

    #[test]
    fn test_parse_fails_with_100_consecutive_errors() {
        // Generate 101 consecutive invalid lines
        let mut content = String::new();
        for i in 0..101 {
            content.push_str(&format!("invalid line {}\n", i));
        }

        let file = create_test_file(&content);
        let result = parse_history_file(file.path());

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Too many consecutive parse errors"));
    }

    #[test]
    fn test_parse_resets_consecutive_errors_on_success() {
        // Test that consecutive error counter resets on success
        // Pattern: 99 errors + 1 valid, repeated - should NOT hit consecutive limit
        // But need to keep overall failure rate <=50%
        // So: (99 errors + 1 valid) + (99 valid) = 99 errors, 100 valid = 49.7% failure
        let mut content = String::new();

        // First: 99 consecutive errors
        for i in 0..99 {
            content.push_str(&format!("invalid line {}\n", i));
        }

        // One valid entry to reset consecutive counter
        content.push_str(r#"{"display":"Reset entry","timestamp":1234567890,"sessionId":"550e8400-e29b-41d4-a716-446655440000"}"#);
        content.push('\n');

        // Now 99 more valid entries to keep failure rate low
        for i in 1..100 {
            content.push_str(&format!(r#"{{"display":"Valid entry {}","timestamp":{},"sessionId":"550e8400-e29b-41d4-a716-44665544{:04x}"}}"#, i, 1234567890 + i, i));
            content.push('\n');
        }

        let file = create_test_file(&content);
        let result = parse_history_file(file.path());

        assert!(result.is_ok());
        let entries = result.unwrap();
        assert_eq!(entries.len(), 100); // 1 reset + 99 valid
    }

    #[test]
    fn test_parse_nonexistent_file() {
        let result = parse_history_file(Path::new("/nonexistent/path/history.jsonl"));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Failed to open"));
    }

    #[test]
    fn test_parse_allows_exactly_50_percent_failures() {
        // 2 valid, 2 invalid = exactly 50% failure rate (should pass)
        let content = r#"{"display":"Valid 1","timestamp":1234567890,"sessionId":"550e8400-e29b-41d4-a716-446655440000"}
invalid line 1
{"display":"Valid 2","timestamp":1234567891,"sessionId":"550e8400-e29b-41d4-a716-446655440001"}
invalid line 2"#;

        let file = create_test_file(content);
        let result = parse_history_file(file.path());

        assert!(result.is_ok());
        let entries = result.unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_parse_mixed_timestamp_formats() {
        let content = r#"{"display":"Unix timestamp","timestamp":1234567890,"sessionId":"550e8400-e29b-41d4-a716-446655440000"}
{"display":"RFC3339 timestamp","timestamp":"2024-01-15T10:30:00Z","sessionId":"550e8400-e29b-41d4-a716-446655440001"}"#;

        let file = create_test_file(content);
        let result = parse_history_file(file.path());

        assert!(result.is_ok());
        let entries = result.unwrap();
        assert_eq!(entries.len(), 2);
    }

    // ===== Large File Handling Tests =====

    #[test]
    fn test_parse_very_long_single_line() {
        // Create a JSONL entry with 100KB display text
        let long_display = "a".repeat(100 * 1024);
        let content = format!(
            r#"{{"display":"{}","timestamp":1234567890,"sessionId":"550e8400-e29b-41d4-a716-446655440000"}}"#,
            long_display
        );

        let file = create_test_file(&content);
        let result = parse_history_file(file.path());

        assert!(result.is_ok(), "Should handle very long single line");
        let entries = result.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].display.len(), 100 * 1024);
    }

    #[test]
    fn test_parse_many_entries() {
        // Test with 1000 entries
        let mut content = String::new();
        for i in 0..1000 {
            content.push_str(&format!(
                r#"{{"display":"Entry {}","timestamp":{},"sessionId":"550e8400-e29b-41d4-a716-4466554{:05x}"}}"#,
                i, 1234567890 + i, i
            ));
            content.push('\n');
        }

        let file = create_test_file(&content);
        let result = parse_history_file(file.path());

        assert!(result.is_ok(), "Should handle many entries");
        let entries = result.unwrap();
        assert_eq!(entries.len(), 1000);
    }

    #[test]
    fn test_parse_file_approaching_size_limit() {
        // Create a file close to 10MB limit (9MB)
        // Each entry is ~100 bytes, so 90000 entries ≈ 9MB
        let mut content = String::new();
        let entry_template = r#"{"display":"Test entry for size check","timestamp":1234567890,"sessionId":"550e8400-e29b-41d4-a716-446655440000"}"#;
        let entry_size = entry_template.len() + 1; // +1 for newline
        let num_entries = (9 * 1024 * 1024) / entry_size;

        for i in 0..num_entries {
            content.push_str(&format!(
                r#"{{"display":"Entry {}","timestamp":{},"sessionId":"550e8400-e29b-41d4-a716-4466554{:05x}"}}"#,
                i, 1234567890 + i, i % 100000
            ));
            content.push('\n');
        }

        let file = create_test_file(&content);
        let result = parse_history_file(file.path());

        assert!(result.is_ok(), "Should handle file approaching size limit");
        assert!(!result.unwrap().is_empty());
    }

    // ===== I/O Error Scenario Tests =====

    #[test]
    fn test_parse_permission_denied() {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;

        // Create file with content
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(b"test").unwrap();
        temp.flush().unwrap();

        // Change permissions to 000 (no read/write/execute)
        let metadata = fs::metadata(temp.path()).unwrap();
        let mut permissions = metadata.permissions();
        permissions.set_mode(0o000);
        fs::set_permissions(temp.path(), permissions).unwrap();

        // Try to parse - should fail with permission error
        let result = parse_history_file(temp.path());

        // Restore permissions before asserting (so temp file can be cleaned up)
        let metadata = fs::metadata(temp.path()).unwrap();
        let mut permissions = metadata.permissions();
        permissions.set_mode(0o644);
        fs::set_permissions(temp.path(), permissions).unwrap();

        assert!(result.is_err(), "Should fail with permission denied");
        assert!(result.unwrap_err().to_string().contains("Failed to open"));
    }

    #[test]
    fn test_parse_truncated_json_at_eof() {
        // File that ends mid-JSON (simulating interrupted write)
        let content = r#"{"display":"Valid entry","timestamp":1234567890,"sessionId":"550e8400-e29b-41d4-a716-446655440000"}
{"display":"Incomplete entry","timestamp":123456"#;

        let file = create_test_file(content);
        let result = parse_history_file(file.path());

        // Should gracefully skip the incomplete entry
        assert!(result.is_ok());
        let entries = result.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].display, "Valid entry");
    }

    #[test]
    fn test_parse_mixed_line_endings() {
        // Mix of LF and CRLF line endings
        let content = "{\
\"display\":\"Entry 1\",\
\"timestamp\":1234567890,\
\"sessionId\":\"550e8400-e29b-41d4-a716-446655440000\"\
}\r\n\
{\
\"display\":\"Entry 2\",\
\"timestamp\":1234567891,\
\"sessionId\":\"550e8400-e29b-41d4-a716-446655440001\"\
}\n\
{\
\"display\":\"Entry 3\",\
\"timestamp\":1234567892,\
\"sessionId\":\"550e8400-e29b-41d4-a716-446655440002\"\
}";

        let file = create_test_file(content);
        let result = parse_history_file(file.path());

        assert!(result.is_ok(), "Should handle mixed line endings");
        let entries = result.unwrap();
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn test_parse_no_trailing_newline() {
        // File without trailing newline (common in some editors)
        let content = r#"{"display":"Entry 1","timestamp":1234567890,"sessionId":"550e8400-e29b-41d4-a716-446655440000"}
{"display":"Entry 2","timestamp":1234567891,"sessionId":"550e8400-e29b-41d4-a716-446655440001"}"#;

        let file = create_test_file(content);
        let result = parse_history_file(file.path());

        assert!(result.is_ok(), "Should handle no trailing newline");
        let entries = result.unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_parse_duplicate_session_ids() {
        // Multiple entries with same session ID (should be allowed)
        let content = r#"{"display":"Entry 1","timestamp":1234567890,"sessionId":"550e8400-e29b-41d4-a716-446655440000"}
{"display":"Entry 2","timestamp":1234567891,"sessionId":"550e8400-e29b-41d4-a716-446655440000"}
{"display":"Entry 3","timestamp":1234567892,"sessionId":"550e8400-e29b-41d4-a716-446655440000"}"#;

        let file = create_test_file(content);
        let result = parse_history_file(file.path());

        assert!(result.is_ok(), "Should allow duplicate session IDs");
        let entries = result.unwrap();
        assert_eq!(entries.len(), 3);
        assert!(entries.iter().all(|e| e.session_id == "550e8400-e29b-41d4-a716-446655440000"));
    }

    #[test]
    fn test_parse_identical_timestamps() {
        // Multiple entries with same timestamp (should be allowed)
        let content = r#"{"display":"Entry 1","timestamp":1234567890000,"sessionId":"550e8400-e29b-41d4-a716-446655440000"}
{"display":"Entry 2","timestamp":1234567890000,"sessionId":"550e8400-e29b-41d4-a716-446655440001"}
{"display":"Entry 3","timestamp":1234567890000,"sessionId":"550e8400-e29b-41d4-a716-446655440002"}"#;

        let file = create_test_file(content);
        let result = parse_history_file(file.path());

        assert!(result.is_ok(), "Should allow identical timestamps");
        let entries = result.unwrap();
        assert_eq!(entries.len(), 3);
        // Check using timestamp_millis() since we store milliseconds
        assert!(entries.iter().all(|e| e.timestamp.timestamp_millis() == 1234567890000));
    }

    #[test]
    fn test_parse_deeply_nested_json() {
        // JSON with nested structures in optional fields
        let content = r#"{"display":"Entry with nested data","timestamp":1234567890,"sessionId":"550e8400-e29b-41d4-a716-446655440000","project":"/Users/test/project","pastedContents":{"type":"text","data":"nested content"}}"#;

        let file = create_test_file(content);
        let result = parse_history_file(file.path());

        assert!(result.is_ok(), "Should handle nested JSON structures");
        let entries = result.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].display, "Entry with nested data");
    }
}
