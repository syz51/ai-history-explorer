use anyhow::Result;
use chrono::NaiveDate;

use super::ast::{FieldFilter, FilterExpr, FilterField, FilterOperator};
use crate::models::search::{EntryType, SearchEntry};

/// Apply filters to search entries, returning filtered results
///
/// Filter logic:
/// - Same-field OR: project:foo project:bar → (foo OR bar)
/// - Cross-field AND: project:foo type:user → (foo AND user)
/// - Explicit operators override defaults
///
/// Filters are evaluated left-to-right with operator precedence
pub fn apply_filters(entries: Vec<SearchEntry>, filter: &FilterExpr) -> Result<Vec<SearchEntry>> {
    if filter.is_empty() {
        return Ok(entries);
    }

    Ok(entries.into_iter().filter(|entry| evaluate_filter(entry, filter)).collect())
}

/// Evaluate filter expression against a single entry
fn evaluate_filter(entry: &SearchEntry, filter: &FilterExpr) -> bool {
    if filter.filters.is_empty() {
        return true;
    }

    // Start with first filter
    let mut result = evaluate_field_filter(entry, &filter.filters[0]);

    // Apply operators and remaining filters
    for (i, operator) in filter.operators.iter().enumerate() {
        let next_filter_result = evaluate_field_filter(entry, &filter.filters[i + 1]);

        result = match operator {
            FilterOperator::And => result && next_filter_result,
            FilterOperator::Or => result || next_filter_result,
        };
    }

    result
}

/// Evaluate single field filter against entry
fn evaluate_field_filter(entry: &SearchEntry, filter: &FieldFilter) -> bool {
    match filter.field {
        FilterField::Project => match_project(entry, &filter.value),
        FilterField::Type => match_type(entry, &filter.value),
        FilterField::Since => match_since(entry, &filter.value),
    }
}

/// Match project path (case-insensitive substring match)
fn match_project(entry: &SearchEntry, value: &str) -> bool {
    if let Some(ref project_path) = entry.project_path {
        let path_str = project_path.to_string_lossy();
        let lower_path = path_str.to_lowercase();
        let lower_value = value.to_lowercase();

        // Support ~ expansion
        let search_value = if lower_value.starts_with('~') {
            // Try to expand ~ to home directory
            if let Some(home) = dirs::home_dir() {
                let home_str = home.to_string_lossy().to_lowercase();
                lower_value.replacen("~", &home_str, 1)
            } else {
                lower_value
            }
        } else {
            lower_value
        };

        lower_path.contains(&search_value)
    } else {
        false
    }
}

/// Match entry type (case-insensitive exact match)
fn match_type(entry: &SearchEntry, value: &str) -> bool {
    let lower_value = value.to_lowercase();
    match lower_value.as_str() {
        "user" => entry.entry_type == EntryType::UserPrompt,
        "agent" => entry.entry_type == EntryType::AgentMessage,
        _ => false,
    }
}

/// Match since date (timestamp >= date)
fn match_since(entry: &SearchEntry, value: &str) -> bool {
    // Parse YYYY-MM-DD format
    if let Ok(date) = NaiveDate::parse_from_str(value, "%Y-%m-%d") {
        let filter_datetime = date.and_hms_opt(0, 0, 0).expect("Valid time").and_utc();
        entry.timestamp >= filter_datetime
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use chrono::{DateTime, TimeZone, Utc};

    use super::*;

    fn create_test_entry(
        entry_type: EntryType,
        project_path: Option<&str>,
        timestamp: DateTime<chrono::Utc>,
    ) -> SearchEntry {
        SearchEntry {
            entry_type,
            display_text: "test".to_string(),
            timestamp,
            project_path: project_path.map(PathBuf::from),
            session_id: "test-session".to_string(),
        }
    }

    #[test]
    fn test_apply_filters_empty() {
        let entries = vec![create_test_entry(EntryType::UserPrompt, Some("/foo/bar"), Utc::now())];
        let filter = FilterExpr::new();
        let result = apply_filters(entries.clone(), &filter).unwrap();
        assert_eq!(result.len(), entries.len());
    }

    #[test]
    fn test_match_project_exact() {
        let entry = create_test_entry(EntryType::UserPrompt, Some("/foo/bar"), Utc::now());
        assert!(match_project(&entry, "foo"));
        assert!(match_project(&entry, "bar"));
        assert!(match_project(&entry, "/foo/bar"));
        assert!(!match_project(&entry, "baz"));
    }

    #[test]
    fn test_match_project_case_insensitive() {
        let entry = create_test_entry(EntryType::UserPrompt, Some("/Foo/Bar"), Utc::now());
        assert!(match_project(&entry, "foo"));
        assert!(match_project(&entry, "FOO"));
        assert!(match_project(&entry, "bar"));
    }

    #[test]
    fn test_match_project_none() {
        let entry = create_test_entry(EntryType::UserPrompt, None, Utc::now());
        assert!(!match_project(&entry, "foo"));
    }

    #[test]
    fn test_match_type_user() {
        let entry = create_test_entry(EntryType::UserPrompt, Some("/foo"), Utc::now());
        assert!(match_type(&entry, "user"));
        assert!(match_type(&entry, "USER"));
        assert!(!match_type(&entry, "agent"));
    }

    #[test]
    fn test_match_type_agent() {
        let entry = create_test_entry(EntryType::AgentMessage, Some("/foo"), Utc::now());
        assert!(match_type(&entry, "agent"));
        assert!(match_type(&entry, "AGENT"));
        assert!(!match_type(&entry, "user"));
    }

    #[test]
    fn test_match_since_after() {
        let entry = create_test_entry(
            EntryType::UserPrompt,
            Some("/foo"),
            Utc.with_ymd_and_hms(2024, 6, 15, 12, 0, 0).unwrap(),
        );
        assert!(match_since(&entry, "2024-01-01")); // Entry after filter
        assert!(match_since(&entry, "2024-06-15")); // Same day
        assert!(!match_since(&entry, "2024-12-31")); // Entry before filter
    }

    #[test]
    fn test_match_since_invalid_date() {
        let entry = create_test_entry(EntryType::UserPrompt, Some("/foo"), Utc::now());
        assert!(!match_since(&entry, "invalid"));
        assert!(!match_since(&entry, "2024-13-01"));
    }

    #[test]
    fn test_evaluate_single_filter() {
        let entry = create_test_entry(EntryType::UserPrompt, Some("/foo/bar"), Utc::now());
        let mut filter = FilterExpr::new();
        filter.add_filter(FieldFilter::new(FilterField::Project, "foo".to_string()));

        assert!(evaluate_filter(&entry, &filter));
    }

    #[test]
    fn test_evaluate_and_operator() {
        let entry = create_test_entry(
            EntryType::UserPrompt,
            Some("/foo/bar"),
            Utc.with_ymd_and_hms(2024, 6, 15, 12, 0, 0).unwrap(),
        );

        let mut filter = FilterExpr::new();
        filter.add_filter(FieldFilter::new(FilterField::Project, "foo".to_string()));
        filter.add_operator(FilterOperator::And);
        filter.add_filter(FieldFilter::new(FilterField::Type, "user".to_string()));

        assert!(evaluate_filter(&entry, &filter));
    }

    #[test]
    fn test_evaluate_and_operator_fails() {
        let entry = create_test_entry(
            EntryType::UserPrompt,
            Some("/foo/bar"),
            Utc.with_ymd_and_hms(2024, 6, 15, 12, 0, 0).unwrap(),
        );

        let mut filter = FilterExpr::new();
        filter.add_filter(FieldFilter::new(FilterField::Project, "foo".to_string()));
        filter.add_operator(FilterOperator::And);
        filter.add_filter(FieldFilter::new(FilterField::Type, "agent".to_string()));

        assert!(!evaluate_filter(&entry, &filter)); // Type mismatch
    }

    #[test]
    fn test_evaluate_or_operator() {
        let entry = create_test_entry(EntryType::UserPrompt, Some("/foo/bar"), Utc::now());

        let mut filter = FilterExpr::new();
        filter.add_filter(FieldFilter::new(FilterField::Project, "baz".to_string()));
        filter.add_operator(FilterOperator::Or);
        filter.add_filter(FieldFilter::new(FilterField::Project, "foo".to_string()));

        assert!(evaluate_filter(&entry, &filter)); // Second filter matches
    }

    #[test]
    fn test_evaluate_complex_expression() {
        let entry = create_test_entry(
            EntryType::UserPrompt,
            Some("/foo/bar"),
            Utc.with_ymd_and_hms(2024, 6, 15, 12, 0, 0).unwrap(),
        );

        // project:foo AND type:user AND since:2024-01-01
        let mut filter = FilterExpr::new();
        filter.add_filter(FieldFilter::new(FilterField::Project, "foo".to_string()));
        filter.add_operator(FilterOperator::And);
        filter.add_filter(FieldFilter::new(FilterField::Type, "user".to_string()));
        filter.add_operator(FilterOperator::And);
        filter.add_filter(FieldFilter::new(FilterField::Since, "2024-01-01".to_string()));

        assert!(evaluate_filter(&entry, &filter));
    }

    #[test]
    fn test_apply_filters_integration() {
        let entries = vec![
            create_test_entry(
                EntryType::UserPrompt,
                Some("/foo/bar"),
                Utc.with_ymd_and_hms(2024, 6, 15, 12, 0, 0).unwrap(),
            ),
            create_test_entry(
                EntryType::AgentMessage,
                Some("/foo/bar"),
                Utc.with_ymd_and_hms(2024, 6, 15, 12, 0, 0).unwrap(),
            ),
            create_test_entry(
                EntryType::UserPrompt,
                Some("/baz/qux"),
                Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap(),
            ),
        ];

        // Filter: type:user
        let mut filter = FilterExpr::new();
        filter.add_filter(FieldFilter::new(FilterField::Type, "user".to_string()));

        let result = apply_filters(entries.clone(), &filter).unwrap();
        assert_eq!(result.len(), 2); // Two UserPrompt entries

        // Filter: project:foo AND type:user
        let mut filter2 = FilterExpr::new();
        filter2.add_filter(FieldFilter::new(FilterField::Project, "foo".to_string()));
        filter2.add_operator(FilterOperator::And);
        filter2.add_filter(FieldFilter::new(FilterField::Type, "user".to_string()));

        let result2 = apply_filters(entries.clone(), &filter2).unwrap();
        assert_eq!(result2.len(), 1); // Only first entry

        // Filter: since:2024-06-01
        let mut filter3 = FilterExpr::new();
        filter3.add_filter(FieldFilter::new(FilterField::Since, "2024-06-01".to_string()));

        let result3 = apply_filters(entries, &filter3).unwrap();
        assert_eq!(result3.len(), 2); // First two entries
    }
}
