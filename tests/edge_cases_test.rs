/// Edge case integration tests
///
/// These tests cover filesystem quirks, data edge cases, and other unusual scenarios
mod common;

use std::fs;

use ai_history_explorer::indexer::build_index;
use common::ClaudeDirBuilder;

#[test]
fn test_edge_case_empty_lines_in_history() {
    // History with empty lines and whitespace-only lines
    let history_content = r#"{"display":"Entry 1","timestamp":1000,"sessionId":"550e8400-e29b-41d4-a716-446655440000"}




{"display":"Entry 2","timestamp":2000,"sessionId":"550e8400-e29b-41d4-a716-446655440001"}"#;

    let claude_dir = ClaudeDirBuilder::new().with_history(history_content).build();

    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should handle empty lines gracefully");

    let index = result.unwrap();
    assert_eq!(index.len(), 2, "Should have 2 valid entries");
}

#[test]
fn test_edge_case_no_trailing_newline() {
    // File without trailing newline
    let history_content = r#"{"display":"Entry 1","timestamp":1000,"sessionId":"550e8400-e29b-41d4-a716-446655440000"}
{"display":"Entry 2","timestamp":2000,"sessionId":"550e8400-e29b-41d4-a716-446655440001"}"#;

    let claude_dir = ClaudeDirBuilder::new().with_history(history_content).build();

    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should handle no trailing newline");

    let index = result.unwrap();
    assert_eq!(index.len(), 2);
}

#[test]
fn test_edge_case_mixed_line_endings() {
    // Mix of LF, CRLF line endings
    let history_content = "{\
\"display\":\"Entry 1\",\
\"timestamp\":1000,\
\"sessionId\":\"550e8400-e29b-41d4-a716-446655440000\"\
}\r\n\
{\
\"display\":\"Entry 2\",\
\"timestamp\":2000,\
\"sessionId\":\"550e8400-e29b-41d4-a716-446655440001\"\
}\n\
{\
\"display\":\"Entry 3\",\
\"timestamp\":3000,\
\"sessionId\":\"550e8400-e29b-41d4-a716-446655440002\"\
}";

    let claude_dir = ClaudeDirBuilder::new().with_history(history_content).build();

    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should handle mixed line endings");

    let index = result.unwrap();
    assert_eq!(index.len(), 3);
}

#[test]
fn test_edge_case_unicode_in_display_text() {
    // Unicode characters: emoji, CJK, RTL text
    let history_content = r#"{"display":"Hello 👋 World 🌍","timestamp":1000,"sessionId":"550e8400-e29b-41d4-a716-446655440000"}
{"display":"测试 中文 テスト","timestamp":2000,"sessionId":"550e8400-e29b-41d4-a716-446655440001"}
{"display":"مرحبا العالم","timestamp":3000,"sessionId":"550e8400-e29b-41d4-a716-446655440002"}"#;

    let claude_dir = ClaudeDirBuilder::new().with_history(history_content).build();

    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should handle Unicode properly");

    let index = result.unwrap();
    assert_eq!(index.len(), 3);
    assert_eq!(index[2].display_text, "Hello 👋 World 🌍");
    assert_eq!(index[1].display_text, "测试 中文 テスト");
}

#[test]
fn test_edge_case_very_long_display_text() {
    // Single entry with 100KB display text
    let long_text = "a".repeat(100 * 1024);
    let history_content = format!(
        r#"{{"display":"{}","timestamp":1000,"sessionId":"550e8400-e29b-41d4-a716-446655440000"}}"#,
        long_text
    );

    let claude_dir = ClaudeDirBuilder::new().with_history(&history_content).build();

    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should handle very long display text");

    let index = result.unwrap();
    assert_eq!(index.len(), 1);
    assert_eq!(index[0].display_text.len(), 100 * 1024);
}

#[test]
fn test_edge_case_many_small_entries() {
    // 1000 small entries
    let mut history_content = String::new();
    for i in 0..1000 {
        history_content.push_str(&format!(
            r#"{{"display":"Entry {}","timestamp":{},"sessionId":"550e8400-e29b-41d4-a716-4466554{:05x}"}}"#,
            i,
            1000 + i,
            i
        ));
        history_content.push('\n');
    }

    let claude_dir = ClaudeDirBuilder::new().with_history(&history_content).build();

    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should handle many small entries");

    let index = result.unwrap();
    assert_eq!(index.len(), 1000);
}

#[test]
fn test_edge_case_duplicate_timestamps() {
    // Multiple entries with identical timestamps
    let history_content = r#"{"display":"Entry 1","timestamp":1000,"sessionId":"550e8400-e29b-41d4-a716-446655440000"}
{"display":"Entry 2","timestamp":1000,"sessionId":"550e8400-e29b-41d4-a716-446655440001"}
{"display":"Entry 3","timestamp":1000,"sessionId":"550e8400-e29b-41d4-a716-446655440002"}"#;

    let claude_dir = ClaudeDirBuilder::new().with_history(history_content).build();

    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should handle duplicate timestamps");

    let index = result.unwrap();
    assert_eq!(index.len(), 3);
    // Order with duplicate timestamps is stable but not guaranteed
}

#[test]
fn test_edge_case_duplicate_session_ids() {
    // Multiple entries with same session ID (valid scenario)
    let history_content = r#"{"display":"Entry 1","timestamp":1000,"sessionId":"550e8400-e29b-41d4-a716-446655440003"}
{"display":"Entry 2","timestamp":2000,"sessionId":"550e8400-e29b-41d4-a716-446655440003"}
{"display":"Entry 3","timestamp":3000,"sessionId":"550e8400-e29b-41d4-a716-446655440003"}"#;

    let claude_dir = ClaudeDirBuilder::new().with_history(history_content).build();

    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should allow duplicate session IDs");

    let index = result.unwrap();
    assert_eq!(index.len(), 3);
}

#[test]
fn test_edge_case_special_characters_in_project_path() {
    // Project with spaces, parentheses, special chars
    let claude_dir = ClaudeDirBuilder::new().with_history("").build();

    let projects_dir = claude_dir.path().join("projects");
    fs::create_dir_all(&projects_dir).unwrap();

    // Create project with special characters
    let project_dir = projects_dir.join("-Users%2Ftest%2Fmy%20project%20%28v1.2%29%20%5Btest%5D");
    fs::create_dir(&project_dir).unwrap();

    let agent_file = project_dir.join("agent-1.jsonl");
    fs::write(&agent_file, r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"test"}]},"timestamp":1000,"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"uuid-1"}"#).unwrap();

    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should handle special characters in project path");

    let index = result.unwrap();
    assert_eq!(index.len(), 1);
    assert_eq!(index[0].project_path, Some("/Users/test/my project (v1.2) [test]".into()));
}

#[test]
fn test_edge_case_non_utf8_filenames() {
    // Skip test on platforms where filenames must be UTF-8
    // On Unix, filenames can be arbitrary bytes
    #[cfg(unix)]
    {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        let claude_dir = ClaudeDirBuilder::new().with_history("").build();

        let projects_dir = claude_dir.path().join("projects");
        let project_dir = projects_dir.join("-Users%2Ftest%2Fproject1");
        fs::create_dir_all(&project_dir).unwrap();

        // Create file with invalid UTF-8 in name
        let invalid_utf8 = OsStr::from_bytes(b"agent-\xFF\xFE.jsonl");
        let invalid_file = project_dir.join(invalid_utf8);
        let _ = fs::write(&invalid_file, b"test data");

        // Create valid agent file
        let valid_file = project_dir.join("agent-1.jsonl");
        fs::write(&valid_file, r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"test"}]},"timestamp":1000,"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"uuid-1"}"#).unwrap();

        let result = build_index(claude_dir.path());
        assert!(result.is_ok(), "Should handle non-UTF8 filenames gracefully");

        let index = result.unwrap();
        // Should have at least the valid entry
        assert!(!index.is_empty());
    }
}

#[test]
fn test_edge_case_truncated_json_at_eof() {
    // File that ends mid-JSON (simulating interrupted write)
    let history_content = r#"{"display":"Valid entry","timestamp":1000,"sessionId":"550e8400-e29b-41d4-a716-446655440000"}
{"display":"Incomplete entry","timestamp":2000"#;

    let claude_dir = ClaudeDirBuilder::new().with_history(history_content).build();

    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should handle truncated JSON gracefully");

    let index = result.unwrap();
    assert_eq!(index.len(), 1, "Should skip incomplete entry");
    assert_eq!(index[0].display_text, "Valid entry");
}

#[test]
fn test_edge_case_empty_display_text() {
    // Entry with empty display text
    let history_content =
        r#"{"display":"","timestamp":1000,"sessionId":"550e8400-e29b-41d4-a716-446655440000"}"#;

    let claude_dir = ClaudeDirBuilder::new().with_history(history_content).build();

    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should handle empty display text");

    let index = result.unwrap();
    assert_eq!(index.len(), 1);
    assert_eq!(index[0].display_text, "");
}

#[test]
fn test_edge_case_project_with_non_agent_jsonl_files() {
    // Project with other .jsonl files that aren't agent-*.jsonl
    let claude_dir = ClaudeDirBuilder::new().with_history("").build();

    let projects_dir = claude_dir.path().join("projects");
    let project_dir = projects_dir.join("-Users%2Ftest%2Fproject1");
    fs::create_dir_all(&project_dir).unwrap();

    // Create non-agent JSONL files (should be ignored)
    fs::write(project_dir.join("history.jsonl"), b"test").unwrap();
    fs::write(project_dir.join("other.jsonl"), b"test").unwrap();
    fs::write(project_dir.join("agent.jsonl"), b"test").unwrap(); // Missing hyphen

    // Create valid agent file
    fs::write(project_dir.join("agent-1.jsonl"), r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"valid"}]},"timestamp":1000,"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"uuid-1"}"#).unwrap();

    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should ignore non-agent JSONL files");

    let index = result.unwrap();
    assert_eq!(index.len(), 1, "Should only process agent-*.jsonl files");
}

#[test]
fn test_edge_case_nested_subdirectories_in_projects() {
    // Verify that nested directories in projects/ are not processed
    let claude_dir = ClaudeDirBuilder::new().with_history("").build();

    let projects_dir = claude_dir.path().join("projects");
    let project_dir = projects_dir.join("-Users%2Ftest%2Fproject1");
    fs::create_dir_all(&project_dir).unwrap();

    // Create nested subdirectory
    let nested_dir = project_dir.join("subdir");
    fs::create_dir(&nested_dir).unwrap();
    fs::write(nested_dir.join("agent-nested.jsonl"), b"should not be processed").unwrap();

    // Create agent file in project root
    fs::write(project_dir.join("agent-1.jsonl"), r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"valid"}]},"timestamp":1000,"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"uuid-1"}"#).unwrap();

    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should ignore nested subdirectories");

    let index = result.unwrap();
    assert_eq!(index.len(), 1, "Should only process files in project root");
}

#[test]
fn test_edge_case_zero_timestamp() {
    // Entry with timestamp of 0 (Unix epoch)
    let history_content = r#"{"display":"Epoch entry","timestamp":0,"sessionId":"550e8400-e29b-41d4-a716-446655440000"}"#;

    let claude_dir = ClaudeDirBuilder::new().with_history(history_content).build();

    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should handle zero timestamp");

    let index = result.unwrap();
    assert_eq!(index.len(), 1);
}

#[test]
fn test_edge_case_far_future_timestamp() {
    // Entry with very large timestamp (year 2100)
    let history_content = r#"{"display":"Future entry","timestamp":4102444800000,"sessionId":"550e8400-e29b-41d4-a716-446655440000"}"#;

    let claude_dir = ClaudeDirBuilder::new().with_history(history_content).build();

    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should handle far future timestamps");

    let index = result.unwrap();
    assert_eq!(index.len(), 1);
}
