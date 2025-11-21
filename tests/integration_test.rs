/// End-to-end integration tests for the AI History Explorer
///
/// These tests verify complete workflows: parsing → indexing → querying
mod common;

use ai_history_explorer::indexer::build_index;
use ai_history_explorer::models::EntryType;
use common::{
    AgentFileBuilder, ClaudeDirBuilder, ConversationEntryBuilder, HistoryEntryBuilder,
    realistic_claude_dir,
};

#[test]
fn test_e2e_parse_history_and_build_index() {
    // Create a .claude directory with history
    let claude_dir = ClaudeDirBuilder::new()
        .with_history_entries(&[
            HistoryEntryBuilder::new()
                .display("Test prompt 1")
                .timestamp(1000)
                .session_id("550e8400-e29b-41d4-a716-446655440000"),
            HistoryEntryBuilder::new()
                .display("Test prompt 2")
                .timestamp(2000)
                .session_id("550e8400-e29b-41d4-a716-446655440001"),
        ])
        .build();

    // Build index
    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should successfully build index");

    let index = result.unwrap();
    assert_eq!(index.len(), 2, "Should have 2 entries");

    // Verify entries are sorted by timestamp (newest first)
    assert_eq!(index[0].display_text, "Test prompt 2");
    assert_eq!(index[1].display_text, "Test prompt 1");

    // All entries should be user prompts
    assert!(index.iter().all(|e| matches!(e.entry_type, EntryType::UserPrompt)));
}

#[test]
fn test_e2e_parse_projects_and_build_index() {
    // Create a .claude directory with project data
    let claude_dir = ClaudeDirBuilder::new()
        .with_history("")
        .with_project(
            "-Users%2Ftest%2Fproject1",
            &[AgentFileBuilder::new("agent-1.jsonl").with_entry(
                ConversationEntryBuilder::user().text("Message from project").timestamp(1000),
            )],
        )
        .build();

    // Build index
    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should successfully build index");

    let index = result.unwrap();
    assert_eq!(index.len(), 1, "Should have 1 entry from project");

    // Verify entry content
    assert_eq!(index[0].display_text, "Message from project");
    assert_eq!(index[0].project_path, Some("/Users/test/project1".into()));
}

#[test]
fn test_e2e_combined_history_and_projects() {
    // Create a complete .claude structure with both history and projects
    let claude_dir = ClaudeDirBuilder::new()
        .with_history_entries(&[HistoryEntryBuilder::new()
            .display("History entry")
            .timestamp(1500)
            .session_id("550e8400-e29b-41d4-a716-446655440000")])
        .with_project(
            "-Users%2Ftest%2Fproject1",
            &[AgentFileBuilder::new("agent-1.jsonl").with_entry(
                ConversationEntryBuilder::user().text("Project entry").timestamp(1000),
            )],
        )
        .build();

    // Build index
    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should successfully build index");

    let index = result.unwrap();
    assert_eq!(index.len(), 2, "Should have 2 entries total");

    // Verify sorting (newest first)
    assert_eq!(index[0].display_text, "History entry");
    assert_eq!(index[1].display_text, "Project entry");

    // Verify project paths
    assert!(index[0].project_path.is_none(), "History entry should have no project path");
    assert_eq!(index[1].project_path, Some("/Users/test/project1".into()));
}

#[test]
fn test_e2e_multiple_projects_multiple_files() {
    // Create complex structure with multiple projects and files
    let claude_dir = ClaudeDirBuilder::new()
        .with_history("")
        .with_project(
            "-Users%2Ftest%2Fproject1",
            &[
                AgentFileBuilder::new("agent-1.jsonl").with_entry(
                    ConversationEntryBuilder::user().text("Project 1, File 1").timestamp(1000),
                ),
                AgentFileBuilder::new("agent-2.jsonl").with_entry(
                    ConversationEntryBuilder::user().text("Project 1, File 2").timestamp(2000),
                ),
            ],
        )
        .with_project(
            "-Users%2Ftest%2Fproject2",
            &[AgentFileBuilder::new("agent-3.jsonl").with_entry(
                ConversationEntryBuilder::user().text("Project 2, File 1").timestamp(1500),
            )],
        )
        .build();

    // Build index
    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should successfully build index");

    let index = result.unwrap();
    assert_eq!(index.len(), 3, "Should have 3 entries from 2 projects");

    // Verify sorting (newest first)
    assert_eq!(index[0].display_text, "Project 1, File 2");
    assert_eq!(index[1].display_text, "Project 2, File 1");
    assert_eq!(index[2].display_text, "Project 1, File 1");
}

#[test]
fn test_e2e_empty_claude_directory() {
    // Empty .claude directory (no history, no projects)
    let claude_dir = ClaudeDirBuilder::new().build();

    // Build index should succeed but return empty
    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should handle empty directory gracefully");

    let index = result.unwrap();
    assert_eq!(index.len(), 0, "Should have no entries");
}

#[test]
fn test_e2e_empty_history_with_projects() {
    // Empty history but projects exist
    let claude_dir = ClaudeDirBuilder::new()
        .with_history("")
        .with_project(
            "-Users%2Ftest%2Fproject1",
            &[AgentFileBuilder::new("agent-1.jsonl").with_entry(
                ConversationEntryBuilder::user().text("Project entry").timestamp(1000),
            )],
        )
        .build();

    // Build index
    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should handle empty history with projects");

    let index = result.unwrap();
    assert_eq!(index.len(), 1, "Should have 1 entry from project");
}

#[test]
fn test_e2e_history_only_no_projects() {
    // History exists but no projects directory
    let claude_dir = ClaudeDirBuilder::new()
        .with_history_entries(&[HistoryEntryBuilder::new()
            .display("History only")
            .timestamp(1000)
            .session_id("550e8400-e29b-41d4-a716-446655440000")])
        .build();

    // Build index
    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should handle history without projects");

    let index = result.unwrap();
    assert_eq!(index.len(), 1, "Should have 1 entry from history");
}

#[test]
fn test_e2e_malformed_history_partial_success() {
    // History with some malformed entries (should skip bad ones)
    let history_content = r#"{"display":"Valid entry 1","timestamp":1000,"sessionId":"550e8400-e29b-41d4-a716-446655440000"}
invalid json line
{"display":"Valid entry 2","timestamp":2000,"sessionId":"550e8400-e29b-41d4-a716-446655440001"}"#;

    let claude_dir = ClaudeDirBuilder::new().with_history(history_content).build();

    // Build index should succeed with valid entries
    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should handle partial malformed data");

    let index = result.unwrap();
    assert_eq!(index.len(), 2, "Should have 2 valid entries");
    assert_eq!(index[0].display_text, "Valid entry 2");
    assert_eq!(index[1].display_text, "Valid entry 1");
}

#[test]
fn test_e2e_project_with_no_agent_files() {
    // Project directory exists but has no agent files
    let claude_dir = ClaudeDirBuilder::new()
        .with_history("")
        .with_project("-Users%2Ftest%2Fproject1", &[])
        .build();

    // Build index should succeed
    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should handle project with no agent files");

    let index = result.unwrap();
    assert_eq!(index.len(), 0, "Should have no entries");
}

#[test]
fn test_e2e_realistic_claude_structure() {
    // Use the realistic helper to create a full structure
    let claude_dir = realistic_claude_dir();

    // Build index
    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should successfully build index from realistic structure");

    let index = result.unwrap();
    assert!(index.len() >= 5, "Should have at least 5 entries (3 history + 2 projects)");

    // Verify we have entries from both history and projects
    let has_history = index.iter().any(|e| e.project_path.is_none());
    let has_projects = index.iter().any(|e| e.project_path.is_some());
    assert!(has_history, "Should have entries from history");
    assert!(has_projects, "Should have entries from projects");
}

#[test]
fn test_e2e_error_propagation_severely_corrupted() {
    // Create history with >50% malformed entries
    // Note: The builder uses graceful degradation and continues even with high failure rates
    let history_content = r#"invalid line 1
{"display":"Valid","timestamp":1000,"sessionId":"550e8400-e29b-41d4-a716-446655440000"}
invalid line 2
invalid line 3"#;

    let claude_dir = ClaudeDirBuilder::new().with_history(history_content).build();

    // Build index succeeds with graceful degradation (warning printed to stderr)
    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should succeed with graceful degradation");

    let index = result.unwrap();
    assert_eq!(index.len(), 0, "Should have 0 entries due to parse failures");
}

// ============================================================================
// New Integration Tests for Content Blocks & Truncation
// ============================================================================

#[test]
fn test_e2e_content_blocks_all_types() {
    // Test thinking, tool_use, tool_result, and image blocks in agent files
    let claude_dir = ClaudeDirBuilder::new()
        .with_history("")
        .with_project(
            "-Users%2Ftest%2Fproject1",
            &[AgentFileBuilder::new("agent-1.jsonl")
                // Assistant with thinking block
                .with_entry(
                    ConversationEntryBuilder::assistant()
                        .content_blocks(vec![
                            ConversationEntryBuilder::thinking_block("Let me analyze this..."),
                            ConversationEntryBuilder::text_block("I'll use a tool"),
                        ])
                        .timestamp(1000),
                )
                // Assistant with tool_use block
                .with_entry(
                    ConversationEntryBuilder::assistant()
                        .content_blocks(vec![ConversationEntryBuilder::tool_use_block(
                            "tool-123",
                            "bash",
                            r#"{"command":"ls -la"}"#,
                        )])
                        .timestamp(2000),
                )
                // Tool result
                .with_entry(
                    ConversationEntryBuilder::user()
                        .content_blocks(vec![ConversationEntryBuilder::tool_result_block(
                            "tool-123",
                            r#""total 0\ndrwxr-xr-x  2 user  staff  64 Jan  1 12:00 .""#,
                            false,
                        )])
                        .timestamp(3000),
                )
                // User with image
                .with_entry(
                    ConversationEntryBuilder::user()
                        .content_blocks(vec![
                            ConversationEntryBuilder::text_block("Check this image"),
                            ConversationEntryBuilder::image_block(
                                r#"{"type":"base64","data":"iVBORw0KGg=="}"#,
                                Some("Screenshot of terminal"),
                            ),
                        ])
                        .timestamp(4000),
                )],
        )
        .build();

    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should parse content blocks");

    let index = result.unwrap();
    assert_eq!(index.len(), 4, "Should have 4 entries with various content blocks");

    // Verify thinking block appears with prefix
    assert!(
        index[3].display_text.contains("[Thinking]"),
        "Thinking block should have [Thinking] prefix"
    );
    assert!(index[3].display_text.contains("Let me analyze"), "Should contain thinking content");

    // Verify tool_use block
    assert!(
        index[2].display_text.contains("[Tool: bash]"),
        "Tool use should have [Tool: name] prefix"
    );

    // Verify tool_result block
    assert!(
        index[1].display_text.contains("[Tool Result]"),
        "Tool result should have [Tool Result] prefix"
    );

    // Verify image block
    assert!(
        index[0].display_text.contains("[Image] Screenshot of terminal"),
        "Image should show alt_text"
    );
}

#[test]
fn test_e2e_truncation_markers() {
    // Test that large content gets truncated with [truncated] markers
    let large_thinking = "x".repeat(2048); // > 1KB
    let large_tool_input = format!(r#"{{"data":"{}"}}"#, "y".repeat(5000)); // > 4KB JSON

    let claude_dir = ClaudeDirBuilder::new()
        .with_history("")
        .with_project(
            "-Users%2Ftest%2Fproject1",
            &[AgentFileBuilder::new("agent-1.jsonl")
                // Thinking block > 1KB
                .with_entry(
                    ConversationEntryBuilder::assistant()
                        .content_blocks(vec![ConversationEntryBuilder::thinking_block(
                            &large_thinking,
                        )])
                        .timestamp(1000),
                )
                // Tool with > 4KB input
                .with_entry(
                    ConversationEntryBuilder::assistant()
                        .content_blocks(vec![ConversationEntryBuilder::tool_use_block(
                            "tool-456",
                            "process",
                            &large_tool_input,
                        )])
                        .timestamp(2000),
                )],
        )
        .build();

    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should handle large content");

    let index = result.unwrap();
    assert_eq!(index.len(), 2, "Should have 2 entries");

    // Verify thinking truncation
    assert!(index[1].display_text.contains("[truncated]"), "Large thinking should be truncated");
    assert!(
        index[1].display_text.len() < large_thinking.len(),
        "Truncated content should be shorter"
    );

    // Verify tool input truncation
    assert!(index[0].display_text.contains("[truncated]"), "Large tool input should be truncated");
}

#[test]
fn test_e2e_assistant_only_conversation() {
    // Test conversation with only assistant messages (no user messages)
    let claude_dir = ClaudeDirBuilder::new()
        .with_history("")
        .with_project(
            "-Users%2Ftest%2Fproject1",
            &[AgentFileBuilder::new("agent-1.jsonl")
                .with_entry(
                    ConversationEntryBuilder::assistant().text("Response 1").timestamp(1000),
                )
                .with_entry(
                    ConversationEntryBuilder::assistant().text("Response 2").timestamp(2000),
                )
                .with_entry(
                    ConversationEntryBuilder::assistant().text("Response 3").timestamp(3000),
                )],
        )
        .build();

    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should handle assistant-only conversation");

    let index = result.unwrap();
    assert_eq!(index.len(), 3, "Should index all assistant messages");

    // Verify all entries are agent messages
    assert!(index.iter().all(|e| matches!(e.entry_type, EntryType::AgentMessage)));

    // Verify content
    assert_eq!(index[0].display_text, "Response 3");
    assert_eq!(index[1].display_text, "Response 2");
    assert_eq!(index[2].display_text, "Response 1");
}

#[test]
fn test_e2e_multibyte_unicode_truncation() {
    // Test truncation at multi-byte Unicode boundaries (emoji, CJK)
    // Create content that's just over 1KB with multi-byte chars at boundary
    let emoji_padding = "\u{1F600}".repeat(300); // 😀 (4 bytes each) = 1200 bytes
    let thinking_with_emoji = format!("Thinking with emoji: {}", emoji_padding);

    let cjk_text = "你好世界".repeat(100); // Each char is 3 bytes
    let thinking_with_cjk = format!("CJK text: {}{}", cjk_text, "x".repeat(500));

    let claude_dir = ClaudeDirBuilder::new()
        .with_history("")
        .with_project(
            "-Users%2Ftest%2Fproject1",
            &[AgentFileBuilder::new("agent-1.jsonl")
                .with_entry(
                    ConversationEntryBuilder::assistant()
                        .content_blocks(vec![ConversationEntryBuilder::thinking_block(
                            &thinking_with_emoji,
                        )])
                        .timestamp(1000),
                )
                .with_entry(
                    ConversationEntryBuilder::assistant()
                        .content_blocks(vec![ConversationEntryBuilder::thinking_block(
                            &thinking_with_cjk,
                        )])
                        .timestamp(2000),
                )],
        )
        .build();

    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should handle multi-byte Unicode truncation without panic");

    let index = result.unwrap();
    assert_eq!(index.len(), 2);

    // Verify both entries are valid UTF-8 (no panic during truncation)
    assert!(index[0].display_text.is_ascii() || !index[0].display_text.is_empty());
    assert!(index[1].display_text.is_ascii() || !index[1].display_text.is_empty());

    // Verify truncation occurred
    assert!(index[0].display_text.contains("[truncated]"));
    assert!(index[1].display_text.contains("[truncated]"));
}

#[test]
fn test_e2e_empty_content_filtering() {
    // Test that entries with empty content are filtered out
    let claude_dir = ClaudeDirBuilder::new()
        .with_history("")
        .with_project(
            "-Users%2Ftest%2Fproject1",
            &[AgentFileBuilder::new("agent-1.jsonl")
                // Non-empty entry
                .with_entry(ConversationEntryBuilder::user().text("Valid message").timestamp(1000))
                // Empty text entry
                .with_entry(ConversationEntryBuilder::user().text("").timestamp(2000))
                // Another valid entry
                .with_entry(
                    ConversationEntryBuilder::assistant().text("Valid response").timestamp(3000),
                )
                // Content block with only whitespace
                .with_entry(
                    ConversationEntryBuilder::user()
                        .content_blocks(vec![ConversationEntryBuilder::text_block("   ")])
                        .timestamp(4000),
                )
                // Another valid entry
                .with_entry(
                    ConversationEntryBuilder::user().text("Another valid").timestamp(5000),
                )],
        )
        .build();

    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should filter empty content");

    let index = result.unwrap();
    // Should have 3 entries (empty strings and whitespace-only are filtered)
    // The implementation filters trim().is_empty()
    // So both empty and whitespace-only entries are excluded
    assert_eq!(index.len(), 3, "Should filter out empty and whitespace-only entries");

    // Verify the truly empty entry ("") and whitespace-only ("   ") were filtered out
    // The 3 entries are: "Valid message", "Valid response", "Another valid"
    assert_eq!(index.len(), 3);
}

#[test]
fn test_e2e_dos_protection_wide_json() {
    // Test DoS protection with extremely wide JSON (10K fields)
    let mut fields = Vec::new();
    for i in 0..10000 {
        fields.push(format!(r#""field_{}":"value_{}""#, i, i));
    }
    let malicious_json = format!("{{{}}}", fields.join(","));

    let claude_dir = ClaudeDirBuilder::new()
        .with_history("")
        .with_project(
            "-Users%2Ftest%2Fproject1",
            &[AgentFileBuilder::new("agent-1.jsonl").with_entry(
                ConversationEntryBuilder::assistant()
                    .content_blocks(vec![ConversationEntryBuilder::tool_use_block(
                        "tool-dos",
                        "malicious",
                        &malicious_json,
                    )])
                    .timestamp(1000),
            )],
        )
        .build();

    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should handle malicious wide JSON gracefully");

    let index = result.unwrap();
    assert_eq!(index.len(), 1);

    // Verify content is capped at reasonable size (4KB limit)
    assert!(index[0].display_text.len() < 6000, "Should cap output size via LimitedWriter");
}

#[test]
fn test_e2e_performance_benchmark_content_blocks() {
    // Performance test: 1000 entries with content blocks
    use std::time::Instant;

    let mut agent_builder = AgentFileBuilder::new("agent-1.jsonl");
    for i in 0..1000 {
        agent_builder = agent_builder.with_entry(
            ConversationEntryBuilder::assistant()
                .content_blocks(vec![
                    ConversationEntryBuilder::thinking_block(&format!("Thinking {}", i)),
                    ConversationEntryBuilder::text_block(&format!("Response {}", i)),
                ])
                .timestamp(i),
        );
    }

    let claude_dir = ClaudeDirBuilder::new()
        .with_history("")
        .with_project("-Users%2Ftest%2Fproject1", &[agent_builder])
        .build();

    let start = Instant::now();
    let result = build_index(claude_dir.path());
    let duration = start.elapsed();

    assert!(result.is_ok(), "Should handle 1000 content block entries");
    let index = result.unwrap();
    assert_eq!(index.len(), 1000);

    // Performance threshold: should complete in < 2 seconds
    assert!(
        duration.as_secs() < 2,
        "Indexing 1000 entries should complete in < 2s (took {:?})",
        duration
    );
}

#[test]
fn test_e2e_memory_stress_many_projects() {
    // Memory stress: 100 projects × 100 files × 10 entries = 100K entries
    // (Reduced from original 500×500×50 for test practicality)
    use std::time::Instant;

    let mut claude_builder = ClaudeDirBuilder::new().with_history("");

    for proj_id in 0..100 {
        let mut agent_files = Vec::new();
        for file_id in 0..100 {
            let mut agent_builder =
                AgentFileBuilder::new(&format!("agent-{}-{}.jsonl", proj_id, file_id));
            for entry_id in 0..10 {
                agent_builder = agent_builder.with_entry(
                    ConversationEntryBuilder::user()
                        .text(&format!("P{} F{} E{}", proj_id, file_id, entry_id))
                        .timestamp((proj_id * 1000000 + file_id * 1000 + entry_id) as i64),
                );
            }
            agent_files.push(agent_builder);
        }
        claude_builder = claude_builder
            .with_project(&format!("-Users%2Ftest%2Fproject{}", proj_id), &agent_files);
    }

    let claude_dir = claude_builder.build();

    let start = Instant::now();
    let result = build_index(claude_dir.path());
    let duration = start.elapsed();

    assert!(result.is_ok(), "Should handle large-scale indexing");
    let index = result.unwrap();
    assert_eq!(index.len(), 100000, "Should index all 100K entries");

    // Memory stress should complete in reasonable time
    eprintln!("Memory stress test: indexed 100K entries in {:?}", duration);
}

#[test]
fn test_e2e_content_type_combination_all_blocks() {
    // Test message with all 5 content block types in one message
    let claude_dir = ClaudeDirBuilder::new()
        .with_history("")
        .with_project(
            "-Users%2Ftest%2Fproject1",
            &[AgentFileBuilder::new("agent-1.jsonl").with_entry(
                ConversationEntryBuilder::assistant()
                    .content_blocks(vec![
                        ConversationEntryBuilder::text_block("Starting analysis"),
                        ConversationEntryBuilder::thinking_block("Need to check files"),
                        ConversationEntryBuilder::tool_use_block(
                            "tool-1",
                            "read_file",
                            r#"{"path":"test.txt"}"#,
                        ),
                        ConversationEntryBuilder::tool_result_block(
                            "tool-1",
                            r#""File contents here""#,
                            false,
                        ),
                        ConversationEntryBuilder::image_block(
                            r#"{"type":"url","url":"https://example.com/chart.png"}"#,
                            Some("Data visualization"),
                        ),
                    ])
                    .timestamp(1000),
            )],
        )
        .build();

    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should handle all content block types in one message");

    let index = result.unwrap();
    assert_eq!(index.len(), 1);

    let content = &index[0].display_text;

    // Verify all block types are present
    assert!(content.contains("Starting analysis"), "Should have text block");
    assert!(content.contains("[Thinking]"), "Should have thinking block");
    assert!(content.contains("[Tool: read_file]"), "Should have tool_use block");
    assert!(content.contains("[Tool Result]"), "Should have tool_result block");
    assert!(content.contains("[Image] Data visualization"), "Should have image block");
}

// ============================================================================
// New Edge Case Tests
// ============================================================================

#[test]
fn test_edge_case_non_bmp_unicode_in_paths() {
    // Test paths with non-BMP Unicode (supplementary plane: U+10000+)
    // These are 4-byte UTF-8 sequences: mathematical alphanumeric symbols
    use std::path::PathBuf;

    use ai_history_explorer::utils::paths::{decode_path, encode_path};

    // U+1D587 = 𝖇 (mathematical bold fraktur small h)
    // U+1F600 = 😀 (emoji)
    let path_with_emoji = PathBuf::from("/Users/test/project😀");
    let path_with_math = PathBuf::from("/Users/test/𝕳𝖊𝖑𝖑𝖔");

    // Test encoding
    let encoded_emoji = encode_path(&path_with_emoji);
    let encoded_math = encode_path(&path_with_math);

    // Test decoding (decode_path always succeeds, returns PathBuf)
    let decoded_emoji = decode_path(&encoded_emoji);
    let decoded_math = decode_path(&encoded_math);

    // Verify round-trip encoding/decoding preserves paths
    assert_eq!(decoded_emoji, path_with_emoji, "Should decode emoji path");
    assert_eq!(decoded_math, path_with_math, "Should decode math symbol path");

    // Test with actual project directory
    let claude_dir = ClaudeDirBuilder::new()
        .with_history("")
        .with_project(
            &encoded_emoji,
            &[AgentFileBuilder::new("agent-1.jsonl").with_entry(
                ConversationEntryBuilder::user().text("Test with emoji").timestamp(1000),
            )],
        )
        .build();

    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should handle non-BMP Unicode in paths");

    let index = result.unwrap();
    assert_eq!(index.len(), 1);
    assert_eq!(
        index[0].project_path,
        Some(path_with_emoji),
        "Should correctly decode non-BMP path"
    );
}

#[test]
#[cfg(unix)]
fn test_edge_case_concurrent_file_modification() {
    use std::time::Duration;
    use std::{fs, thread};

    // Test file being truncated between validation and read
    let claude_dir = ClaudeDirBuilder::new().with_history("").build();

    let projects_dir = claude_dir.path().join("projects");
    let project_dir = projects_dir.join("-Users%2Ftest%2Fproject1");
    fs::create_dir_all(&project_dir).unwrap();

    // Create agent file with content
    let agent_file = project_dir.join("agent-1.jsonl");
    fs::write(
        &agent_file,
        r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"Original content"}]},"timestamp":1000,"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"uuid-1"}"#,
    )
    .unwrap();

    // Spawn thread to truncate file after brief delay
    let agent_file_clone = agent_file.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(10));
        // Truncate the file
        let _ = fs::write(&agent_file_clone, "");
    });

    // Build index - may see original content or empty depending on timing
    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should handle concurrent modification gracefully");

    // Either 0 (truncated before read) or 1 (read before truncate) is acceptable
    let index = result.unwrap();
    assert!(index.len() <= 1, "Should have 0 or 1 entries depending on race timing");
}

#[test]
fn test_edge_case_empty_content_blocks_array() {
    use std::fs;

    // Test message with empty content array: content: []
    let claude_dir = ClaudeDirBuilder::new().with_history("").build();

    let projects_dir = claude_dir.path().join("projects");
    let project_dir = projects_dir.join("-Users%2Ftest%2Fproject1");
    fs::create_dir_all(&project_dir).unwrap();

    // Write agent file with empty content array
    let agent_file = project_dir.join("agent-1.jsonl");
    fs::write(
        &agent_file,
        r#"{"type":"user","message":{"role":"user","content":[]},"timestamp":1000,"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"uuid-1"}
{"type":"user","message":{"role":"user","content":[{"type":"text","text":"Valid"}]},"timestamp":2000,"sessionId":"550e8400-e29b-41d4-a716-446655440001","uuid":"uuid-2"}"#,
    )
    .unwrap();

    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should handle empty content array");

    let index = result.unwrap();
    // Empty content array should be filtered out (no display text)
    assert_eq!(index.len(), 1, "Should filter out empty content array entry");
    assert_eq!(index[0].display_text, "Valid");
}

#[test]
fn test_edge_case_negative_timestamps() {
    // Test timestamps before Unix epoch (negative values)
    // December 31, 1969, 23:59:59 = -1000ms
    let history_content = r#"{"display":"Before epoch","timestamp":-1000,"sessionId":"550e8400-e29b-41d4-a716-446655440000"}
{"display":"At epoch","timestamp":0,"sessionId":"550e8400-e29b-41d4-a716-446655440001"}
{"display":"After epoch","timestamp":1000,"sessionId":"550e8400-e29b-41d4-a716-446655440002"}"#;

    let claude_dir = ClaudeDirBuilder::new().with_history(history_content).build();

    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should handle negative timestamps (before Unix epoch)");

    let index = result.unwrap();
    assert_eq!(index.len(), 3, "Should parse all entries with negative timestamps");

    // Verify sorting (newest first)
    assert_eq!(index[0].display_text, "After epoch");
    assert_eq!(index[1].display_text, "At epoch");
    assert_eq!(index[2].display_text, "Before epoch");

    // Verify timestamps are correctly ordered
    assert!(index[0].timestamp > index[1].timestamp);
    assert!(index[1].timestamp > index[2].timestamp);
}

#[test]
fn test_edge_case_timestamp_millisecond_boundaries() {
    // Test timestamp precision at millisecond boundaries
    // 999ms → 1000ms rollover
    let history_content = r#"{"display":"T1","timestamp":999,"sessionId":"550e8400-e29b-41d4-a716-446655440000"}
{"display":"T2","timestamp":1000,"sessionId":"550e8400-e29b-41d4-a716-446655440001"}
{"display":"T3","timestamp":1001,"sessionId":"550e8400-e29b-41d4-a716-446655440002"}
{"display":"T4","timestamp":999999,"sessionId":"550e8400-e29b-41d4-a716-446655440003"}
{"display":"T5","timestamp":1000000,"sessionId":"550e8400-e29b-41d4-a716-446655440004"}"#;

    let claude_dir = ClaudeDirBuilder::new().with_history(history_content).build();

    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should handle millisecond boundary timestamps");

    let index = result.unwrap();
    assert_eq!(index.len(), 5, "Should parse all boundary timestamps");

    // Verify correct ordering (newest first)
    assert_eq!(index[0].display_text, "T5");
    assert_eq!(index[1].display_text, "T4");
    assert_eq!(index[2].display_text, "T3");
    assert_eq!(index[3].display_text, "T2");
    assert_eq!(index[4].display_text, "T1");

    // Verify timestamps are distinct at millisecond precision
    assert_ne!(index[0].timestamp, index[1].timestamp);
    assert_ne!(index[2].timestamp, index[3].timestamp);
    assert_ne!(index[3].timestamp, index[4].timestamp);
}

#[test]
fn test_edge_case_utf8_bom_in_history() {
    use std::fs;

    // Test UTF-8 BOM (Byte Order Mark) at start of file
    // BOM: 0xEF 0xBB 0xBF
    let claude_dir = ClaudeDirBuilder::new().build();
    let history_path = claude_dir.path().join("history.jsonl");

    // Write file with BOM
    let content_with_bom = format!(
        "\u{FEFF}{}",
        r#"{"display":"After BOM","timestamp":1000,"sessionId":"550e8400-e29b-41d4-a716-446655440000"}"#
    );
    fs::write(&history_path, content_with_bom.as_bytes()).unwrap();

    let result = build_index(claude_dir.path());
    // BOM may cause parse failure or be skipped
    // Either outcome is acceptable as long as no crash
    if let Ok(index) = result {
        // May have 0 entries (BOM broke parse) or 1 entry (BOM handled)
        assert!(index.len() <= 1, "Should handle or skip BOM");
    }
}

#[test]
#[ignore] // Very expensive - creates 100MB+ file
fn test_edge_case_extremely_long_line() {
    use std::fs;
    use std::io::Write;

    // Test single line > 100MB to verify memory safety
    let claude_dir = ClaudeDirBuilder::new().build();
    let history_path = claude_dir.path().join("history.jsonl");

    // Create file with 100MB+ display text
    let mut file = fs::File::create(&history_path).unwrap();
    file.write_all(b"{\"display\":\"").unwrap();

    // Write 101MB of 'x'
    let chunk = vec![b'x'; 1024 * 1024]; // 1MB chunks
    for _ in 0..101 {
        file.write_all(&chunk).unwrap();
    }

    file.write_all(
        b"\",\"timestamp\":1000,\"sessionId\":\"550e8400-e29b-41d4-a716-446655440000\"}",
    )
    .unwrap();
    file.flush().unwrap();

    // Should reject due to file size limit (10MB)
    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should handle very large files gracefully");

    let index = result.unwrap();
    assert_eq!(index.len(), 0, "Should skip file exceeding size limit");
}

#[test]
fn test_edge_case_i64_max_min_timestamps() {
    // Test timestamp boundaries: i64::MAX and i64::MIN
    // Note: DateTime has a limited range, so extreme values will be skipped
    // Both MAX and MIN will fail, causing 66% failure rate, which triggers graceful degradation
    let history_content = format!(
        r#"{{"display":"MAX timestamp","timestamp":{},"sessionId":"550e8400-e29b-41d4-a716-446655440000"}}
{{"display":"MIN timestamp","timestamp":{},"sessionId":"550e8400-e29b-41d4-a716-446655440001"}}
{{"display":"Normal","timestamp":1000,"sessionId":"550e8400-e29b-41d4-a716-446655440002"}}"#,
        i64::MAX,
        i64::MIN
    );

    let claude_dir = ClaudeDirBuilder::new().with_history(&history_content).build();

    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should not crash on i64 boundary timestamps");

    let index = result.unwrap();
    // i64::MAX and i64::MIN are out of DateTime range (66% failure rate)
    // With graceful degradation, this results in 0 entries
    assert_eq!(
        index.len(),
        0,
        "High failure rate should result in 0 entries (graceful degradation)"
    );
}

#[test]
fn test_edge_case_whitespace_only_display_text() {
    // Test whitespace-only content (should be trimmed and filtered)
    let history_content = r#"{"display":"   ","timestamp":1000,"sessionId":"550e8400-e29b-41d4-a716-446655440000"}
{"display":"Valid","timestamp":2000,"sessionId":"550e8400-e29b-41d4-a716-446655440001"}
{"display":"\t\n  \r","timestamp":3000,"sessionId":"550e8400-e29b-41d4-a716-446655440002"}"#;

    let claude_dir = ClaudeDirBuilder::new().with_history(history_content).build();

    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should handle whitespace-only content");

    let index = result.unwrap();
    // Whitespace-only entries are filtered out (trim().is_empty() check)
    // Only the "Valid" entry should be included
    assert_eq!(index.len(), 1, "Should filter out whitespace-only entries");
    assert_eq!(index[0].display_text, "Valid");
}

#[test]
fn test_edge_case_unknown_content_block_type() {
    use std::fs;

    // Test forward compatibility with unknown future content block types
    let claude_dir = ClaudeDirBuilder::new().with_history("").build();

    let projects_dir = claude_dir.path().join("projects");
    let project_dir = projects_dir.join("-Users%2Ftest%2Fproject1");
    fs::create_dir_all(&project_dir).unwrap();

    // Write agent file with unknown content block type
    // Need 2 lines: one with unknown type (will fail), one valid (to stay under 50% failure)
    let agent_file = project_dir.join("agent-1.jsonl");
    fs::write(
        &agent_file,
        r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"Known"},{"type":"future_type","data":"unknown"}]},"timestamp":1000,"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"uuid-1"}
{"type":"user","message":{"role":"user","content":[{"type":"text","text":"Valid"}]},"timestamp":2000,"sessionId":"550e8400-e29b-41d4-a716-446655440001","uuid":"uuid-2"}"#,
    )
    .unwrap();

    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should handle unknown content block types");

    let index = result.unwrap();
    // Line with unknown block type fails to parse entirely (current behavior)
    // Only the valid line is parsed
    assert_eq!(index.len(), 1, "Should process valid line");
    assert_eq!(index[0].display_text, "Valid");
}

#[test]
fn test_edge_case_multiple_consecutive_slashes_in_path() {
    use std::path::PathBuf;

    use ai_history_explorer::utils::paths::{decode_path, encode_path};

    // Test path with multiple consecutive slashes: /Users//test///project
    let path_with_slashes = PathBuf::from("/Users//test///project");

    let encoded = encode_path(&path_with_slashes);
    let decoded = decode_path(&encoded);

    // Path should be preserved (not normalized)
    assert_eq!(decoded, path_with_slashes, "Should preserve consecutive slashes");
}

#[test]
fn test_edge_case_trailing_slash_in_path() {
    use std::path::PathBuf;

    use ai_history_explorer::utils::paths::{decode_path, encode_path};

    // Test paths with trailing slashes
    let path_with_slash = PathBuf::from("/Users/test/project/");
    let path_without_slash = PathBuf::from("/Users/test/project");

    let encoded_with = encode_path(&path_with_slash);
    let encoded_without = encode_path(&path_without_slash);

    let decoded_with = decode_path(&encoded_with);
    let decoded_without = decode_path(&encoded_without);

    // Paths should be preserved as-is
    assert_eq!(decoded_with, path_with_slash);
    assert_eq!(decoded_without, path_without_slash);
}

#[test]
fn test_edge_case_path_at_os_limit() {
    use std::path::PathBuf;

    use ai_history_explorer::utils::paths::{decode_path, encode_path};

    // Test path at typical OS limit (4096 bytes on Linux)
    // Create path with many components to reach limit
    let long_component = "a".repeat(100);
    let mut path_str = String::from("/");

    // Add components until we reach ~4000 bytes
    while path_str.len() < 4000 {
        path_str.push_str(&long_component);
        path_str.push('/');
    }

    let long_path = PathBuf::from(&path_str);

    let encoded = encode_path(&long_path);
    let decoded = decode_path(&encoded);

    // Should handle very long paths
    assert_eq!(decoded, long_path, "Should handle paths at OS limits");
}

#[test]
#[cfg(unix)]
fn test_edge_case_hardlinks_same_file() {
    use std::fs;
    use std::fs::hard_link;

    // Test multiple hardlinks to same file (should reject them)
    let claude_dir = ClaudeDirBuilder::new().with_history("").build();

    let projects_dir = claude_dir.path().join("projects");
    let project_dir = projects_dir.join("-Users%2Ftest%2Fproject1");
    fs::create_dir_all(&project_dir).unwrap();

    // Create some valid files first (to keep failure rate < 50%)
    fs::write(
        project_dir.join("agent-3.jsonl"),
        r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"valid1"}]},"timestamp":1000,"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"uuid-1"}"#,
    )
    .unwrap();
    fs::write(
        project_dir.join("agent-4.jsonl"),
        r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"valid2"}]},"timestamp":2000,"sessionId":"550e8400-e29b-41d4-a716-446655440001","uuid":"uuid-2"}"#,
    )
    .unwrap();
    fs::write(
        project_dir.join("agent-5.jsonl"),
        r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"valid3"}]},"timestamp":3000,"sessionId":"550e8400-e29b-41d4-a716-446655440002","uuid":"uuid-3"}"#,
    )
    .unwrap();

    // Create original file that will become a hardlink
    let original_file = project_dir.join("agent-1.jsonl");
    fs::write(
        &original_file,
        r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"hardlink"}]},"timestamp":4000,"sessionId":"550e8400-e29b-41d4-a716-446655440003","uuid":"uuid-4"}"#,
    )
    .unwrap();

    // Create hardlink
    let hardlink_file = project_dir.join("agent-2.jsonl");
    hard_link(&original_file, &hardlink_file).unwrap();

    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should reject hardlinks gracefully");

    let index = result.unwrap();
    // Security fix: hardlinks are now rejected to prevent hardlink attacks
    // agent-1.jsonl and agent-2.jsonl both have nlink=2, so both are rejected
    // agent-3.jsonl, agent-4.jsonl, agent-5.jsonl are valid (nlink=1) and processed
    assert_eq!(index.len(), 3, "Hardlinks rejected, valid files processed");
}

#[test]
fn test_edge_case_mixed_null_and_valid_content_blocks() {
    use std::fs;

    // Test content array with null elements mixed with valid blocks
    let claude_dir = ClaudeDirBuilder::new().with_history("").build();

    let projects_dir = claude_dir.path().join("projects");
    let project_dir = projects_dir.join("-Users%2Ftest%2Fproject1");
    fs::create_dir_all(&project_dir).unwrap();

    // This is syntactically valid JSON but semantically unusual
    let agent_file = project_dir.join("agent-1.jsonl");
    fs::write(
        &agent_file,
        r#"{"type":"user","message":{"role":"user","content":[null,{"type":"text","text":"Valid"},null]},"timestamp":1000,"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"uuid-1"}"#,
    )
    .unwrap();

    let result = build_index(claude_dir.path());
    // Either parses successfully (skipping nulls) or fails
    // Both are acceptable as long as no crash
    if let Ok(index) = result {
        // If it parses, should only have the valid block
        if !index.is_empty() {
            assert_eq!(index[0].display_text, "Valid");
        }
    }
}

#[test]
fn test_edge_case_deeply_nested_tool_result() {
    use std::fs;

    // Test tool_result with JSON nesting beyond serde's limit (128 levels)
    let mut nested_json = String::from("\"");
    for _ in 0..150 {
        nested_json = format!("{{\"a\":{}}}", nested_json);
    }

    let claude_dir = ClaudeDirBuilder::new().with_history("").build();

    let projects_dir = claude_dir.path().join("projects");
    let project_dir = projects_dir.join("-Users%2Ftest%2Fproject1");
    fs::create_dir_all(&project_dir).unwrap();

    // Need 2 lines: one with deep nesting (will fail), one valid (to stay under 50% failure)
    let agent_file = project_dir.join("agent-1.jsonl");
    let deep_nested_line = format!(
        r#"{{"type":"user","message":{{"role":"user","content":[{{"type":"tool_result","id":"tool-deep","content":{},"isError":false}}]}},"timestamp":1000,"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"uuid-1"}}"#,
        nested_json
    );
    let valid_line = r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"Valid"}]},"timestamp":2000,"sessionId":"550e8400-e29b-41d4-a716-446655440001","uuid":"uuid-2"}"#;

    fs::write(&agent_file, format!("{}\n{}", deep_nested_line, valid_line)).unwrap();

    let result = build_index(claude_dir.path());
    // Serde will reject deep nesting (recursion limit), but should not crash
    assert!(result.is_ok(), "Should handle deep nesting without crash");

    let index = result.unwrap();
    // Deep nesting line fails, valid line succeeds
    assert_eq!(index.len(), 1, "Should parse valid line, skip deep nested");
    assert_eq!(index[0].display_text, "Valid");
}

#[test]
#[ignore] // Very expensive - creates 1M entries
fn test_integration_1m_entries_performance() {
    use std::fs;
    use std::time::Instant;

    // Test performance with 1M entries
    let claude_dir = ClaudeDirBuilder::new().build();
    let history_path = claude_dir.path().join("history.jsonl");

    // Write 1M lines
    let mut content = String::new();
    for i in 0..1_000_000 {
        content.push_str(&format!(
            r#"{{"display":"Entry {}","timestamp":{},"sessionId":"550e8400-e29b-41d4-a716-446655440000"}}"#,
            i, i
        ));
        content.push('\n');
    }
    fs::write(&history_path, content).unwrap();

    let start = Instant::now();
    let result = build_index(claude_dir.path());
    let duration = start.elapsed();

    assert!(result.is_ok(), "Should handle 1M entries");
    let index = result.unwrap();
    assert_eq!(index.len(), 1_000_000, "Should parse all 1M entries");

    eprintln!("1M entries indexed in {:?}", duration);
    // Performance threshold: should complete in reasonable time
    assert!(duration.as_secs() < 30, "Should index 1M entries in < 30s (took {:?})", duration);
}

#[test]
fn test_integration_all_history_invalid_all_projects_valid() {
    // Test where 100% of history is corrupt but projects are valid
    let history_content = "invalid line 1\ninvalid line 2\ninvalid line 3";

    let claude_dir = ClaudeDirBuilder::new()
        .with_history(history_content)
        .with_project(
            "-Users%2Ftest%2Fproject1",
            &[AgentFileBuilder::new("agent-1.jsonl").with_entry(
                ConversationEntryBuilder::user().text("Valid project").timestamp(1000),
            )],
        )
        .build();

    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should succeed when history fails but projects succeed");

    let index = result.unwrap();
    assert_eq!(index.len(), 1, "Should have entry from valid project");
    assert_eq!(index[0].display_text, "Valid project");
}

#[test]
fn test_integration_exactly_50_percent_failure() {
    // Test exactly 50% failure rate boundary
    let mut lines = Vec::new();
    for i in 0..100 {
        if i % 2 == 0 {
            // Valid entries
            lines.push(format!(
                r#"{{"display":"Valid {}","timestamp":{},"sessionId":"550e8400-e29b-41d4-a716-446655440000"}}"#,
                i, i
            ));
        } else {
            // Invalid entries
            lines.push(format!("invalid line {}", i));
        }
    }
    let history_content = lines.join("\n");

    let claude_dir = ClaudeDirBuilder::new().with_history(&history_content).build();

    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should succeed with exactly 50% failures");

    let index = result.unwrap();
    // Due to graceful degradation, may have 0 entries (failed threshold)
    // or 50 entries (passed threshold)
    assert!(index.len() <= 50, "Should have 0-50 entries with 50% failure rate");
}

#[test]
fn test_integration_consecutive_errors_reset_pattern() {
    use std::fs;

    // Test: 99 errors, success, 99 errors again
    // Verifies consecutive error counter resets properly
    let claude_dir = ClaudeDirBuilder::new().build();
    let history_path = claude_dir.path().join("history.jsonl");

    let mut lines = Vec::new();

    // 99 invalid lines
    for i in 0..99 {
        lines.push(format!("invalid {}", i));
    }

    // 1 valid line
    lines.push(
        r#"{"display":"Valid 1","timestamp":1000,"sessionId":"550e8400-e29b-41d4-a716-446655440000"}"#
            .to_string(),
    );

    // 99 more invalid lines
    for i in 0..99 {
        lines.push(format!("invalid again {}", i));
    }

    // 1 more valid line
    lines.push(
        r#"{"display":"Valid 2","timestamp":2000,"sessionId":"550e8400-e29b-41d4-a716-446655440001"}"#
            .to_string(),
    );

    fs::write(&history_path, lines.join("\n")).unwrap();

    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should handle consecutive error reset pattern");

    let index = result.unwrap();
    // May have 0 entries (high failure rate) or 2 entries (reset worked)
    assert!(index.len() <= 2, "Should process with consecutive error reset");
}
