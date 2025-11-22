//! Integration tests for filter functionality

use ai_history_explorer::filters::apply::apply_filters;
use ai_history_explorer::filters::parser::parse_filter;
use ai_history_explorer::models::{EntryType, SearchEntry};
use chrono::{TimeZone, Utc};

fn create_test_entry(
    display_text: &str,
    project_path: Option<&str>,
    entry_type: EntryType,
) -> SearchEntry {
    SearchEntry {
        entry_type,
        display_text: display_text.to_string(),
        timestamp: Utc.timestamp_opt(1234567890, 0).unwrap(),
        project_path: project_path.map(|s| s.into()),
        session_id: "test-session".to_string(),
    }
}

#[test]
fn test_filter_integration_project() {
    let entries = vec![
        create_test_entry("Entry 1", Some("/Users/test/ai-history"), EntryType::UserPrompt),
        create_test_entry("Entry 2", Some("/Users/test/other-project"), EntryType::UserPrompt),
        create_test_entry("Entry 3", None, EntryType::UserPrompt),
    ];

    let filter = parse_filter("project:ai-history").expect("Parse filter");
    let filtered = apply_filters(entries, &filter).expect("Apply filter");

    assert_eq!(filtered.len(), 1);
    assert!(filtered[0].project_path.as_ref().unwrap().to_string_lossy().contains("ai-history"));
}

#[test]
fn test_filter_integration_type() {
    let entries = vec![
        create_test_entry("User entry", None, EntryType::UserPrompt),
        create_test_entry("Agent entry", None, EntryType::AgentMessage),
        create_test_entry("Another user entry", None, EntryType::UserPrompt),
    ];

    let filter = parse_filter("type:user").expect("Parse filter");
    let filtered = apply_filters(entries, &filter).expect("Apply filter");

    assert_eq!(filtered.len(), 2);
    assert!(filtered.iter().all(|e| matches!(e.entry_type, EntryType::UserPrompt)));
}

#[test]
fn test_filter_integration_combined() {
    let entries = vec![
        create_test_entry("Entry 1", Some("/Users/test/ai-history"), EntryType::UserPrompt),
        create_test_entry("Entry 2", Some("/Users/test/ai-history"), EntryType::AgentMessage),
        create_test_entry("Entry 3", Some("/Users/test/other"), EntryType::UserPrompt),
    ];

    let filter = parse_filter("project:ai-history type:user").expect("Parse filter");
    let filtered = apply_filters(entries, &filter).expect("Apply filter");

    assert_eq!(filtered.len(), 1);
    assert!(filtered[0].project_path.as_ref().unwrap().to_string_lossy().contains("ai-history"));
    assert!(matches!(filtered[0].entry_type, EntryType::UserPrompt));
}

#[test]
fn test_filter_integration_or_operator() {
    let entries = vec![
        create_test_entry("Entry 1", Some("/Users/test/project1"), EntryType::UserPrompt),
        create_test_entry("Entry 2", Some("/Users/test/project2"), EntryType::UserPrompt),
        create_test_entry("Entry 3", Some("/Users/test/project3"), EntryType::UserPrompt),
    ];

    let filter = parse_filter("project:project1 project:project2").expect("Parse filter");
    let filtered = apply_filters(entries, &filter).expect("Apply filter");

    assert_eq!(filtered.len(), 2);
}

#[test]
fn test_filter_integration_invalid_filter() {
    let result = parse_filter("invalid:field");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Unknown field"));
}

#[test]
fn test_filter_integration_empty_result() {
    let entries =
        vec![create_test_entry("Entry 1", Some("/Users/test/project"), EntryType::UserPrompt)];

    let filter = parse_filter("project:nonexistent").expect("Parse filter");
    let filtered = apply_filters(entries, &filter).expect("Apply filter");

    assert_eq!(filtered.len(), 0);
}

#[test]
fn test_filter_integration_since() {
    let entries = vec![
        SearchEntry {
            entry_type: EntryType::UserPrompt,
            display_text: "Old entry".to_string(),
            timestamp: Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap(),
            project_path: None,
            session_id: "test".to_string(),
        },
        SearchEntry {
            entry_type: EntryType::UserPrompt,
            display_text: "New entry".to_string(),
            timestamp: Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap(),
            project_path: None,
            session_id: "test".to_string(),
        },
    ];

    let filter = parse_filter("since:2023-01-01").expect("Parse filter");
    let filtered = apply_filters(entries, &filter).expect("Apply filter");

    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].display_text, "New entry");
}

#[test]
fn test_filter_integration_case_insensitive_project() {
    let entries = vec![create_test_entry(
        "Entry",
        Some("/Users/test/AI-History-Explorer"),
        EntryType::UserPrompt,
    )];

    let filter = parse_filter("project:ai-history").expect("Parse filter");
    let filtered = apply_filters(entries, &filter).expect("Apply filter");

    assert_eq!(filtered.len(), 1);
}

#[test]
fn test_filter_integration_quoted_value() {
    let entries =
        vec![create_test_entry("Entry", Some("/Users/test/my project"), EntryType::UserPrompt)];

    let filter = parse_filter("project:\"my project\"").expect("Parse filter");
    let filtered = apply_filters(entries, &filter).expect("Apply filter");

    assert_eq!(filtered.len(), 1);
}
