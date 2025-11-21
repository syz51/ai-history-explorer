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
