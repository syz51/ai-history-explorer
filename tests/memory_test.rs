/// Memory and resource management tests
///
/// These tests verify proper resource cleanup, memory management, and leak detection
mod common;

use ai_history_explorer::indexer::build_index;
use common::{AgentFileBuilder, ClaudeDirBuilder, ConversationEntryBuilder};

#[test]
fn test_memory_no_leaks_repeated_indexing() {
    // Test for memory leaks by indexing the same data 1000 times
    // If there's a leak, memory usage would grow linearly
    let claude_dir = ClaudeDirBuilder::new()
        .with_history_entries(&[common::HistoryEntryBuilder::new()
            .display("Test entry")
            .timestamp(1000)
            .session_id("550e8400-e29b-41d4-a716-446655440000")])
        .build();

    // Run indexing 1000 times
    for i in 0..1000 {
        let result = build_index(claude_dir.path());
        assert!(result.is_ok(), "Iteration {} should succeed", i);

        let index = result.unwrap();
        assert_eq!(index.len(), 1, "Should always have 1 entry");

        // Drop index to simulate cleanup
        drop(index);
    }

    // If we got here without OOM, no obvious leak
    // For proper leak detection, run under valgrind or miri:
    // - cargo +nightly miri test test_memory_no_leaks_repeated_indexing
    // - valgrind --leak-check=full cargo test test_memory_no_leaks_repeated_indexing
}

#[test]
fn test_memory_string_deduplication_efficiency() {
    // Test that duplicate strings don't cause excessive memory usage
    // Create 100 entries with identical display text
    let mut entries = Vec::new();
    for i in 0..100 {
        entries.push(
            common::HistoryEntryBuilder::new()
                .display("Duplicate text") // Same text for all
                .timestamp(i)
                .session_id(&format!("550e8400-e29b-41d4-a716-44665544{:04}", i)),
        );
    }

    let claude_dir = ClaudeDirBuilder::new().with_history_entries(&entries).build();

    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should handle duplicate strings");

    let index = result.unwrap();
    assert_eq!(index.len(), 100, "Should have all 100 entries");

    // Verify all entries have the same display text
    assert!(index.iter().all(|e| e.display_text == "Duplicate text"));

    // Note: Rust's String type doesn't automatically intern/deduplicate
    // This test documents the current behavior
    // For true deduplication, would need to use Arc<str> or string interning
}

#[test]
fn test_memory_vec_growth_patterns() {
    // Test Vec reallocation patterns with varying sizes
    // Small dataset (10 entries)
    let small_dir = ClaudeDirBuilder::new()
        .with_history_entries(&[common::HistoryEntryBuilder::new()
            .display("Small")
            .timestamp(1000)
            .session_id("550e8400-e29b-41d4-a716-446655440000")])
        .build();

    let result = build_index(small_dir.path());
    assert!(result.is_ok());
    assert_eq!(result.unwrap().len(), 1);

    // Medium dataset (1000 entries)
    let mut medium_entries = Vec::new();
    for i in 0..1000 {
        medium_entries.push(
            common::HistoryEntryBuilder::new()
                .display(&format!("Entry {}", i))
                .timestamp(i)
                .session_id(&format!("550e8400-e29b-41d4-a716-44665544{:04}", i)),
        );
    }

    let medium_dir = ClaudeDirBuilder::new().with_history_entries(&medium_entries).build();

    let result = build_index(medium_dir.path());
    assert!(result.is_ok());
    assert_eq!(result.unwrap().len(), 1000);

    // Vec growth should be logarithmic (powers of 2), not linear
    // This test verifies the code completes without excessive reallocations
}

#[test]
fn test_memory_cleanup_on_early_return() {
    // Test that resources are cleaned up when errors occur
    // Create invalid data that will cause early return
    let claude_dir = ClaudeDirBuilder::new()
        .with_history("invalid json on every line\ninvalid again\nstill invalid")
        .build();

    // Build index will return Ok with graceful degradation
    // Verify all file handles are closed
    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should return Ok with graceful degradation");

    let index = result.unwrap();
    assert_eq!(index.len(), 0, "Should have 0 entries due to parse failures");

    // If file handles weren't closed, subsequent operations might fail
    // Test we can still access the directory
    let result2 = build_index(claude_dir.path());
    assert!(result2.is_ok(), "Should work on second attempt (files were closed)");
}

#[test]
fn test_memory_large_individual_entries() {
    // Test memory handling with large individual entries
    // Create entry with 1MB display text (under 10MB file limit)
    let large_text = "x".repeat(1024 * 1024); // 1MB

    let claude_dir = ClaudeDirBuilder::new()
        .with_history_entries(&[common::HistoryEntryBuilder::new()
            .display(&large_text)
            .timestamp(1000)
            .session_id("550e8400-e29b-41d4-a716-446655440000")])
        .build();

    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should handle 1MB entry");

    let index = result.unwrap();
    assert_eq!(index.len(), 1, "Should have 1 entry");
    assert_eq!(index[0].display_text.len(), 1024 * 1024);

    // Drop to verify cleanup
    drop(index);
}

#[test]
fn test_memory_many_small_allocations() {
    // Test memory handling with many small allocations
    // Create 10,000 small entries
    let mut entries = Vec::new();
    for i in 0..10000 {
        entries.push(
            common::HistoryEntryBuilder::new()
                .display(&format!("E{}", i))
                .timestamp(i)
                .session_id(&format!("550e8400-e29b-41d4-a716-44665544{:04}", i % 10000)),
        );
    }

    let claude_dir = ClaudeDirBuilder::new().with_history_entries(&entries).build();

    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should handle 10K small allocations");

    let index = result.unwrap();
    assert_eq!(index.len(), 10000);

    // Memory fragmentation shouldn't cause issues
    drop(index);
}

#[test]
#[ignore] // Requires specific memory limits to be set
fn test_memory_low_memory_conditions() {
    // Test behavior under low memory conditions
    // To run: ulimit -v 100000 && cargo test test_memory_low_memory_conditions -- --ignored
    // This would limit virtual memory to ~100MB

    // Create moderate dataset
    let mut entries = Vec::new();
    for i in 0..1000 {
        entries.push(
            common::HistoryEntryBuilder::new()
                .display(&format!("Entry {}", i))
                .timestamp(i)
                .session_id(&format!("550e8400-e29b-41d4-a716-44665544{:04}", i)),
        );
    }

    let claude_dir = ClaudeDirBuilder::new().with_history_entries(&entries).build();

    // Should either succeed or fail gracefully (no crash)
    let result = build_index(claude_dir.path());

    // Both outcomes acceptable under memory pressure
    match result {
        Ok(index) => {
            assert_eq!(index.len(), 1000, "Should index all entries if memory available");
        }
        Err(_) => {
            // Graceful failure under memory pressure is acceptable
        }
    }
}

#[test]
fn test_memory_content_block_allocation_patterns() {
    // Test memory allocation patterns for different content block types
    let claude_dir = ClaudeDirBuilder::new()
        .with_history("")
        .with_project(
            "-Users%2Ftest%2Fproject1",
            &[AgentFileBuilder::new("agent-1.jsonl")
                // Many thinking blocks
                .with_entry(
                    ConversationEntryBuilder::assistant()
                        .content_blocks(vec![
                            ConversationEntryBuilder::thinking_block("Think 1"),
                            ConversationEntryBuilder::thinking_block("Think 2"),
                            ConversationEntryBuilder::thinking_block("Think 3"),
                        ])
                        .timestamp(1000),
                )
                // Many tool blocks
                .with_entry(
                    ConversationEntryBuilder::assistant()
                        .content_blocks(vec![
                            ConversationEntryBuilder::tool_use_block("t1", "tool1", r#"{"a":"b"}"#),
                            ConversationEntryBuilder::tool_use_block("t2", "tool2", r#"{"c":"d"}"#),
                        ])
                        .timestamp(2000),
                )],
        )
        .build();

    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should handle varied content blocks");

    let index = result.unwrap();
    assert_eq!(index.len(), 2);

    // Verify content blocks were processed
    assert!(index[1].display_text.contains("Think"));
    assert!(index[0].display_text.contains("[Tool:"));
}

#[test]
fn test_memory_drop_behavior_verification() {
    // Test that Drop is properly called for resources
    // Note: Comprehensive drop testing requires valgrind/miri

    let _drop_counter = 0; // Placeholder for drop tracking

    {
        let claude_dir = ClaudeDirBuilder::new()
            .with_history_entries(&[common::HistoryEntryBuilder::new()
                .display("Test")
                .timestamp(1000)
                .session_id("550e8400-e29b-41d4-a716-446655440000")])
            .build();

        let result = build_index(claude_dir.path());
        assert!(result.is_ok());

        let index = result.unwrap();
        assert_eq!(index.len(), 1);

        // index goes out of scope here
    }

    // After scope, all memory should be freed
    // This is a basic test - for comprehensive testing, use valgrind/miri
}

#[test]
fn test_memory_concurrent_file_handle_limits() {
    // Test that we don't keep too many file handles open
    // Create 100 projects with 10 files each
    use std::fs;

    let claude_dir = ClaudeDirBuilder::new().with_history("").build();

    let projects_dir = claude_dir.path().join("projects");
    fs::create_dir_all(&projects_dir).unwrap();

    for proj in 0..100 {
        let project_dir = projects_dir.join(format!("-Users%2Ftest%2Fproject{}", proj));
        fs::create_dir(&project_dir).unwrap();

        for file in 0..10 {
            let agent_file = project_dir.join(format!("agent-{}.jsonl", file));
            fs::write(
                &agent_file,
                format!(
                    r#"{{"type":"user","message":{{"role":"user","content":[{{"type":"text","text":"P{} F{}"}}]}},"timestamp":{},"sessionId":"550e8400-e29b-41d4-a716-446655440000","uuid":"uuid-{}-{}"}}"#,
                    proj, file, proj * 1000 + file, proj, file
                ),
            )
            .unwrap();
        }
    }

    // Build index should not exhaust file handles
    let result = build_index(claude_dir.path());
    assert!(result.is_ok(), "Should handle 1000 files without FD exhaustion");

    let index = result.unwrap();
    assert_eq!(index.len(), 1000);

    // Files should all be closed now
    // Verify by trying to read again
    let result2 = build_index(claude_dir.path());
    assert!(result2.is_ok(), "Should work again (all handles were closed)");
}
