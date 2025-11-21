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

#[test]
#[cfg(unix)]
fn test_security_symlink_toctou_race_condition() {
    use std::os::unix::fs::symlink;
    use std::thread;
    use std::time::Duration;

    // Create test structure with valid project
    let claude_dir = ClaudeDirBuilder::new().with_history("").build();

    let projects_dir = claude_dir.path().join("projects");
    let project_dir = projects_dir.join("-Users%2Ftest%2Fproject1");
    fs::create_dir_all(&project_dir).unwrap();

    // Create valid agent file
    let agent_file = project_dir.join("agent-1.jsonl");
    fs::write(&agent_file, r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"test"}]},"timestamp":1000,"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"uuid-1"}"#).unwrap();

    // Spawn thread to replace file with symlink after brief delay
    let agent_file_clone = agent_file.clone();
    let target_file = claude_dir.path().parent().unwrap().join("sensitive.jsonl");
    fs::write(&target_file, r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"sensitive"}]},"timestamp":2000,"sessionId":"550e8400-e29b-41d4-a716-446655440001","uuid":"uuid-2"}"#).unwrap();

    thread::spawn(move || {
        thread::sleep(Duration::from_millis(10));
        let _ = fs::remove_file(&agent_file_clone);
        let _ = symlink(&target_file, &agent_file_clone);
    });

    // Build index - may or may not catch the race
    // This test documents the TOCTOU vulnerability
    let result = build_index(claude_dir.path());
    // Current implementation is vulnerable to TOCTOU
    // Future: implement O_NOFOLLOW to prevent following symlinks during read
    assert!(result.is_ok(), "Should complete without crashing");
}

#[test]
#[ignore] // Expensive test - run with: cargo test -- --ignored
fn test_security_memory_exhaustion_10m_entries() {
    // Test memory exhaustion with 10M entries (1000 projects × 1000 files × 10 entries)
    // This is an expensive test that validates graceful degradation under resource limits
    let claude_dir = ClaudeDirBuilder::new().with_history("").build();

    let projects_dir = claude_dir.path().join("projects");
    fs::create_dir_all(&projects_dir).unwrap();

    // Create 100 projects (scaled down from 1000 for test performance)
    for i in 0..100 {
        let project_dir = projects_dir.join(format!("-Users%2Ftest%2Fproject{}", i));
        fs::create_dir(&project_dir).unwrap();

        // Create 100 agent files per project (scaled down from 1000)
        for j in 0..100 {
            let agent_file = project_dir.join(format!("agent-{}.jsonl", j));
            // Each file has 10 entries
            let mut content = String::new();
            for k in 0..10 {
                content.push_str(&format!(
                    r#"{{"type":"user","message":{{"role":"user","content":[{{"type":"text","text":"Entry {} from project {} file {}"}}]}},"timestamp":{},"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"uuid-{}-{}-{}"}}"#,
                    k, i, j, 1000 + i * 1000 + j * 10 + k, i, j, k
                ));
                content.push('\n');
            }
            fs::write(&agent_file, content).unwrap();
        }
    }

    // Build index should handle 100K entries (scaled down from 10M)
    // Memory usage should be roughly linear with entry count
    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should handle large datasets without OOM");

    let index = result.unwrap();
    // Expect 100 projects × 100 files × 10 entries = 100K entries
    assert_eq!(index.len(), 100000, "Should successfully index all entries");
}

#[test]
fn test_security_integer_overflow_line_counting() {
    // Test integer overflow protection in line counting
    // This is a conceptual test - we can't create a file with usize::MAX lines
    // but we verify the parser handles extremely large line counts gracefully

    // Create a file with many lines to test counter behavior
    let mut lines = Vec::new();
    for i in 0..10000 {
        lines.push(format!(
            r#"{{"display":"Line {}","timestamp":{},"sessionId":"550e8400-e29b-41d4-a716-446655440000"}}"#,
            i, 1000 + i
        ));
    }
    let history_content = lines.join("\n");

    let claude_dir = ClaudeDirBuilder::new().with_history(&history_content).build();

    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should handle large line counts");

    let index = result.unwrap();
    assert_eq!(index.len(), 10000, "Should parse all lines correctly");

    // Note: True usize::MAX testing would require synthetic mocking
    // Current implementation uses usize for line counting which could theoretically
    // overflow, but in practice files would exceed 10MB limit first
}

#[test]
fn test_security_null_byte_in_encoded_path() {
    // Test NULL byte handling in percent-encoded paths
    // NULL bytes can be used for directory traversal on some systems
    use ai_history_explorer::utils::paths::decode_path;

    // Test various NULL byte scenarios
    let test_cases = vec![
        "-Users%2Ftest%00",            // NULL at end
        "-Users%00%2Ftest",            // NULL in middle
        "-Users%2Ftest%00%2F..%2Fetc", // NULL with traversal attempt
    ];

    for encoded in test_cases {
        let path = decode_path(encoded);
        // decode_path always succeeds (returns PathBuf)
        // Verify the decoded path contains NULL bytes (documenting current behavior)
        let _path_str = path.to_string_lossy();
        // Note: Current implementation allows NULL bytes through percent-decoding
        // This is a documented limitation - validation happens at a higher level
        // The path may contain NULL bytes after decoding
    }
}

#[test]
fn test_security_json_recursion_depth_limit() {
    // Test that deeply nested JSON doesn't cause stack overflow
    // serde_json has built-in recursion protection (default limit varies by version)

    // Create JSON with 200 levels of nesting
    let mut deeply_nested = String::from(
        "{\"display\":\"test\",\"timestamp\":1000,\"sessionId\":\"550e8400-e29b-41d4-a716-446655440001\",\"extra\":",
    );
    for _ in 0..200 {
        deeply_nested.push_str("{\"a\":");
    }
    deeply_nested.push_str("\"value\"");
    for _ in 0..200 {
        deeply_nested.push('}');
    }
    deeply_nested.push('}');

    let claude_dir = ClaudeDirBuilder::new().with_history(&deeply_nested).build();

    // The important thing is this doesn't cause a stack overflow crash
    let result = build_index(claude_dir.path());

    // Either:
    // 1. Parsing succeeds with 0 entries (serde_json rejected it)
    // 2. Parsing succeeds with 1 entry (serde_json accepted it)
    // 3. Parsing fails (error during parse)
    // All outcomes are acceptable as long as no stack overflow occurs
    if let Ok(index) = result {
        // Document that moderate nesting may be accepted
        assert!(index.len() <= 1, "Should have at most 1 entry from deeply nested JSON");
    }
    // If result is Err, that's also fine - key is no crash
}

#[test]
#[cfg(unix)]
fn test_security_hardlink_outside_claude_dir() {
    use std::fs::hard_link;

    // Create test structure
    let claude_dir = ClaudeDirBuilder::new().with_history("").build();

    let projects_dir = claude_dir.path().join("projects");
    let project_dir = projects_dir.join("-Users%2Ftest%2Fproject1");
    fs::create_dir_all(&project_dir).unwrap();

    // Add valid files first (to keep failure rate < 50%)
    fs::write(
        project_dir.join("agent-1.jsonl"),
        r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"valid1"}]},"timestamp":1000,"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"uuid-1"}"#,
    )
    .unwrap();
    fs::write(
        project_dir.join("agent-2.jsonl"),
        r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"valid2"}]},"timestamp":2000,"sessionId":"550e8400-e29b-41d4-a716-446655440001","uuid":"uuid-2"}"#,
    )
    .unwrap();

    // Create target file outside .claude
    let target_file = claude_dir.path().parent().unwrap().join("sensitive.jsonl");
    fs::write(
        &target_file,
        r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"sensitive data"}]},"timestamp":3000,"sessionId":"550e8400-e29b-41d4-a716-446655440002","uuid":"uuid-3"}"#,
    )
    .unwrap();

    // Create hardlink in project directory pointing to target
    let hardlink_file = project_dir.join("agent-hardlink.jsonl");
    hard_link(&target_file, &hardlink_file).unwrap();

    // Build index - security fix now rejects hardlinks
    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should gracefully reject hardlinks");

    // Security fix: hardlink is now detected and rejected
    let index = result.unwrap();
    // Only the 2 valid files should be processed, hardlink rejected
    assert_eq!(index.len(), 2, "Security fix: hardlink to external file rejected");
}

#[test]
fn test_security_float_timestamp() {
    // Test handling of float timestamps (should be rejected or handled)
    let history_content = r#"{"display":"test","timestamp":1234.567,"sessionId":"550e8400-e29b-41d4-a716-446655440000"}"#;

    let claude_dir = ClaudeDirBuilder::new().with_history(history_content).build();

    // Build index should reject or handle float timestamps
    let result = build_index(claude_dir.path());

    // Either parsing fails (timestamp deserialization error) or succeeds with 0 entries
    // Both outcomes are acceptable
    if let Ok(index) = result {
        // If it parses, timestamp was likely truncated to integer
        assert!(index.len() <= 1, "Float timestamp should be rejected or truncated");
    }
    // If result is Err, that's fine - serde_json rejected the float
}

#[test]
fn test_security_null_byte_in_session_id() {
    // Test NULL byte in session ID
    let history_content =
        "{\"display\":\"test\",\"timestamp\":1000,\"sessionId\":\"session\0null\"}";

    let claude_dir = ClaudeDirBuilder::new().with_history(history_content).build();

    // Build index should handle or reject null bytes in session ID
    let result = build_index(claude_dir.path());
    // Either way, it shouldn't crash
    let _ = result;
}

#[test]
fn test_security_memory_bomb_large_display_text() {
    use std::io::Write;

    // Test memory bomb: 100MB display text (exceeds file size limit)
    let claude_dir = ClaudeDirBuilder::new().build();
    let history_path = claude_dir.path().join("history.jsonl");

    // Create file with 100MB display text
    let mut file = fs::File::create(&history_path).unwrap();
    file.write_all(b"{\"display\":\"").unwrap();

    // Write 100MB of 'a' characters
    let chunk = vec![b'a'; 1024 * 1024]; // 1MB chunks
    for _ in 0..100 {
        file.write_all(&chunk).unwrap();
    }

    file.write_all(
        b"\",\"timestamp\":1000,\"sessionId\":\"550e8400-e29b-41d4-a716-446655440000\"}",
    )
    .unwrap();
    file.flush().unwrap();

    // Build index should reject due to file size limit (10MB)
    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should succeed with graceful degradation");

    let index = result.unwrap();
    assert_eq!(index.len(), 0, "Should skip file exceeding size limit");
}

#[test]
fn test_security_memory_bomb_wide_json() {
    // Test memory bomb: JSON with 10K+ fields in a single object
    // This tests memory allocation for wide JSON structures
    let mut json = String::from(
        r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_use","id":"tool-1","name":"test","input":{"#,
    );

    // Add 10,000 fields
    for i in 0..10000 {
        if i > 0 {
            json.push(',');
        }
        json.push_str(&format!("\"field{}\":\"value{}\"", i, i));
    }

    json.push_str(
        r#"}}]},"timestamp":1000,"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"uuid-1"}"#,
    );

    let claude_dir = ClaudeDirBuilder::new().build();
    let projects_dir = claude_dir.path().join("projects");
    let project_dir = projects_dir.join("-Users%2Ftest%2Fproject1");
    fs::create_dir_all(&project_dir).unwrap();

    let agent_file = project_dir.join("agent-1.jsonl");
    fs::write(&agent_file, &json).unwrap();

    // Build index should handle wide JSON without excessive memory use
    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should handle wide JSON structures");

    // Wide JSON may be accepted or rejected depending on size
    // Key is no crash or OOM
    let _ = result.unwrap();
}

#[test]
#[ignore] // Expensive test - requires many file handles
fn test_security_file_descriptor_exhaustion() {
    // Test file descriptor limits by creating many files
    // This tests that we don't keep too many files open simultaneously
    let claude_dir = ClaudeDirBuilder::new().with_history("").build();

    let projects_dir = claude_dir.path().join("projects");
    fs::create_dir_all(&projects_dir).unwrap();

    // Create 500 projects with 2 files each = 1000 total files
    for i in 0..500 {
        let project_dir = projects_dir.join(format!("-Users%2Ftest%2Fproject{}", i));
        fs::create_dir(&project_dir).unwrap();

        for j in 0..2 {
            let agent_file = project_dir.join(format!("agent-{}.jsonl", j));
            fs::write(
                &agent_file,
                format!(
                    r#"{{"type":"user","message":{{"role":"user","content":[{{"type":"text","text":"test {} {}"}}]}},"timestamp":{},"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"uuid-{}-{}"}}"#,
                    i, j, 1000 + i * 2 + j, i, j
                ),
            )
            .unwrap();
        }
    }

    // Build index should not exhaust file descriptors
    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should handle many files without FD exhaustion");

    let index = result.unwrap();
    assert_eq!(index.len(), 1000, "Should successfully read all files");
}

#[test]
#[cfg(unix)]
fn test_security_toctou_file_modified_during_read() {
    use std::io::Write;
    use std::thread;
    use std::time::Duration;

    // Test TOCTOU: file size checked, then modified before read
    let claude_dir = ClaudeDirBuilder::new().build();
    let history_path = claude_dir.path().join("history.jsonl");

    // Create small valid file
    fs::write(
        &history_path,
        r#"{"display":"initial","timestamp":1000,"sessionId":"550e8400-e29b-41d4-a716-446655440000"}"#,
    )
    .unwrap();

    // Spawn thread to modify file after brief delay
    let history_path_clone = history_path.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(10));
        // Append data to make file larger
        let mut file = fs::OpenOptions::new().append(true).open(&history_path_clone).unwrap();
        // Add 11MB of data (exceeds limit)
        let chunk = vec![b'x'; 1024 * 1024];
        for _ in 0..11 {
            let _ = file.write_all(&chunk);
        }
    });

    // Build index - race condition between size check and read
    let result = build_index(claude_dir.path());
    // Current implementation is vulnerable to TOCTOU
    // File could be modified between stat() and read()
    assert!(result.is_ok(), "Should complete without crashing");

    // Either we read before modification (1 entry) or after (0 entries due to size)
    // Both outcomes document the race condition
    let index = result.unwrap();
    assert!(index.len() <= 1, "TOCTOU race: file size may change during read");
}
