/// Security-focused integration tests
///
/// These tests verify security boundaries: symlinks, path traversal, resource limits
mod common;

use std::fs;

use ai_history_explorer::indexer::build_index;
use common::ClaudeDirBuilder;

#[test]
#[cfg(unix)] // Symlinks work differently on Windows
fn test_security_symlink_project_directory_rejected() {
    use std::os::unix::fs::symlink;

    // Create test structure
    let claude_dir = ClaudeDirBuilder::new().with_history("").build();

    let projects_dir = claude_dir.path().join("projects");
    fs::create_dir_all(&projects_dir).unwrap();

    // Create target directory outside .claude
    let target_dir = claude_dir.path().parent().unwrap().join("sensitive_data");
    let _ = fs::create_dir(&target_dir); // Ignore error if already exists

    // Create symlink in projects directory pointing to target
    let symlink_path = projects_dir.join("-Users%2Ftest%2Fmalicious");
    symlink(&target_dir, &symlink_path).unwrap();

    // Build index should skip the symlinked project
    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should succeed but skip symlinked project");

    let index = result.unwrap();
    assert_eq!(index.len(), 0, "Should have no entries from symlinked project");
}

#[test]
#[cfg(unix)]
fn test_security_symlink_agent_file_rejected() {
    use std::os::unix::fs::symlink;

    // Create test structure
    let claude_dir = ClaudeDirBuilder::new().with_history("").build();

    let projects_dir = claude_dir.path().join("projects");
    let project_dir = projects_dir.join("-Users%2Ftest%2Fproject1");
    fs::create_dir_all(&project_dir).unwrap();

    // Create target file outside project
    let target_file = claude_dir.path().parent().unwrap().join("sensitive.jsonl");
    fs::write(&target_file, r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"sensitive data"}]},"timestamp":1000,"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"uuid-1"}"#).unwrap();

    // Create symlink agent file pointing to target
    let symlink_file = project_dir.join("agent-malicious.jsonl");
    symlink(&target_file, &symlink_file).unwrap();

    // Build index should skip the symlinked agent file
    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should succeed but skip symlinked agent file");

    let index = result.unwrap();
    assert_eq!(index.len(), 0, "Should have no entries from symlinked file");
}

#[test]
fn test_security_path_traversal_in_encoded_name() {
    // Create project with path traversal in encoded name
    let claude_dir = ClaudeDirBuilder::new().with_history("").build();

    let projects_dir = claude_dir.path().join("projects");
    fs::create_dir_all(&projects_dir).unwrap();

    // Create directory with path traversal attempt
    let traversal_dir = projects_dir.join("-Users%2F..%2Fetc%2Fpasswd");
    fs::create_dir(&traversal_dir).unwrap();

    // Create agent file in traversal directory
    let agent_file = traversal_dir.join("agent-1.jsonl");
    fs::write(&agent_file, r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"malicious"}]},"timestamp":1000,"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"uuid-1"}"#).unwrap();

    // Build index should skip the directory with path traversal
    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should succeed but skip traversal directory");

    let index = result.unwrap();
    assert_eq!(index.len(), 0, "Should have no entries from traversal directory");
}

#[test]
fn test_security_resource_limit_max_projects() {
    // Create structure exceeding MAX_PROJECTS (1000)
    let claude_dir = ClaudeDirBuilder::new().with_history("").build();

    let projects_dir = claude_dir.path().join("projects");
    fs::create_dir_all(&projects_dir).unwrap();

    // Create 1001 project directories (exceeds limit)
    for i in 0..=1000 {
        let project_dir = projects_dir.join(format!("-Users%2Ftest%2Fproject{}", i));
        fs::create_dir(&project_dir).unwrap();

        // Add one agent file to each
        let agent_file = project_dir.join("agent-1.jsonl");
        fs::write(&agent_file, r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"test"}]},"timestamp":1000,"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"uuid-1"}"#).unwrap();
    }

    // Build index should succeed with graceful degradation (projects skipped)
    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should succeed with graceful degradation");

    let index = result.unwrap();
    assert_eq!(index.len(), 0, "Should have no entries due to max projects limit");
}

#[test]
fn test_security_resource_limit_max_agent_files() {
    // Create project with >1000 agent files (exceeds limit)
    let claude_dir = ClaudeDirBuilder::new().with_history("").build();

    let projects_dir = claude_dir.path().join("projects");
    let project_dir = projects_dir.join("-Users%2Ftest%2Fproject1");
    fs::create_dir_all(&project_dir).unwrap();

    // Create 1001 agent files (exceeds limit)
    for i in 0..=1000 {
        let agent_file = project_dir.join(format!("agent-{}.jsonl", i));
        fs::write(&agent_file, r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"test"}]},"timestamp":1000,"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"uuid-1"}"#).unwrap();
    }

    // Build index should succeed with graceful degradation (agent files skipped)
    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should succeed with graceful degradation");

    let index = result.unwrap();
    assert_eq!(index.len(), 0, "Should have no entries due to max agent files limit");
}

#[test]
fn test_security_file_size_limit_history() {
    use std::io::Write;

    // Create history file > 10MB (exceeds limit)
    let claude_dir = ClaudeDirBuilder::new().build();
    let history_path = claude_dir.path().join("history.jsonl");

    let mut file = fs::File::create(&history_path).unwrap();
    // Write 10MB + 1 byte
    let chunk = vec![b'a'; 1024 * 1024]; // 1MB chunk
    for _ in 0..10 {
        file.write_all(&chunk).unwrap();
    }
    file.write_all(b"x").unwrap(); // Extra byte to exceed limit
    file.flush().unwrap();

    // Build index should succeed with graceful degradation (file skipped)
    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should succeed with graceful degradation");

    let index = result.unwrap();
    assert_eq!(index.len(), 0, "Should have no entries due to file size limit");
}

#[test]
fn test_security_json_depth_limit_reasonable_nesting() {
    // Verify that moderately nested JSON (10 levels) is accepted
    // Note: serde_json has built-in recursion limit (default 128) that automatically
    // protects against deeply nested JSON that could cause stack overflow
    let mut nested = String::from(
        "{\"display\":\"test\",\"timestamp\":1000,\"sessionId\":\"550e8400-e29b-41d4-a716-446655440001\",\"extra\":",
    );
    for _ in 0..10 {
        nested.push_str("{\"a\":");
    }
    nested.push_str("\"value\"");
    for _ in 0..10 {
        nested.push('}');
    }
    nested.push('}');

    let claude_dir = ClaudeDirBuilder::new().with_history(&nested).build();

    // Build index should succeed with reasonable nesting
    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should handle reasonably nested JSON");

    let index = result.unwrap();
    assert_eq!(index.len(), 1, "Moderately nested entry should be accepted");
}

#[test]
fn test_security_hidden_files_ignored() {
    // Verify that hidden files like .DS_Store are not processed
    let claude_dir = ClaudeDirBuilder::new().with_history("").build();

    let projects_dir = claude_dir.path().join("projects");
    let project_dir = projects_dir.join("-Users%2Ftest%2Fproject1");
    fs::create_dir_all(&project_dir).unwrap();

    // Create hidden file (should be ignored)
    let hidden_file = project_dir.join(".DS_Store");
    fs::write(&hidden_file, b"binary data").unwrap();

    // Create valid agent file
    let agent_file = project_dir.join("agent-1.jsonl");
    fs::write(&agent_file, r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"valid"}]},"timestamp":1000,"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"uuid-1"}"#).unwrap();

    // Build index should only process agent file
    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should skip hidden files");

    let index = result.unwrap();
    assert_eq!(index.len(), 1, "Should only have entry from agent file");
    assert_eq!(index[0].display_text, "valid");
}

#[test]
fn test_security_null_byte_in_display_text() {
    // Test handling of null bytes in content
    let history_content =
        "{\"display\":\"Test\0null\0byte\",\"timestamp\":1000,\"sessionId\":\"session-1\"}";

    let claude_dir = ClaudeDirBuilder::new().with_history(history_content).build();

    // Build index should handle null bytes
    let result = build_index(claude_dir.path());
    // May fail to parse or succeed depending on serde_json behavior
    // Either way, it shouldn't crash
    let _ = result;
}

#[test]
fn test_security_unicode_normalization() {
    // Test with different Unicode representations
    // U+00E9 (é) vs U+0065 U+0301 (e + combining acute)
    let history_content_1 = "{\"display\":\"caf\u{00E9}\",\"timestamp\":1000,\"sessionId\":\"550e8400-e29b-41d4-a716-446655440001\"}";
    let history_content_2 = "{\"display\":\"cafe\u{0301}\",\"timestamp\":2000,\"sessionId\":\"550e8400-e29b-41d4-a716-446655440002\"}";

    let claude_dir = ClaudeDirBuilder::new()
        .with_history(&format!("{}\n{}", history_content_1, history_content_2))
        .build();

    // Build index should handle both forms
    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should handle Unicode normalization forms");

    let index = result.unwrap();
    assert_eq!(index.len(), 2, "Should have both entries");
}

#[test]
fn test_security_highly_compressible_content() {
    // Test with highly compressible content (10MB of zeros)
    // This tests for potential zip bomb style attacks
    let large_display = "0".repeat(10 * 1024 * 1024 - 100); // Close to 10MB limit
    let history_content = format!(
        r#"{{"display":"{}","timestamp":1000,"sessionId":"550e8400-e29b-41d4-a716-446655440000"}}"#,
        large_display
    );

    let claude_dir = ClaudeDirBuilder::new().with_history(&history_content).build();

    // Should be accepted since it's under the 10MB limit
    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should accept file under size limit");

    let index = result.unwrap();
    assert_eq!(index.len(), 1, "Should have the compressible entry");
}
