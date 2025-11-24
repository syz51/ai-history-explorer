//! Filter query parser for search entry filtering.
//!
//! Parses user-provided filter expressions into an AST ([`FilterExpr`]) for evaluation.
//! Supports field-based filtering with operators and quoted values.
//!
//! # Syntax
//!
//! ```text
//! filter_expr := field_filter (operator field_filter)*
//! field_filter := field_name:value | field_name:"quoted value"
//! operator := AND | OR (case-insensitive)
//! field_name := project | type | since (case-insensitive)
//! ```
//!
//! # Supported Fields
//!
//! - `project:path` - Filter by project path (supports ~ expansion and partial matches)
//! - `type:user|agent` - Filter by entry type (user prompts or agent messages)
//! - `since:YYYY-MM-DD` - Filter by timestamp (entries on or after date)
//!
//! # Examples
//!
//! ```rust
//! # use ai_history_explorer::filters::parser::parse_filter;
//! // Single filter
//! let expr = parse_filter("project:ai-explorer").unwrap();
//!
//! // Multiple filters with implicit AND (different fields)
//! let expr = parse_filter("project:ai-explorer type:user").unwrap();
//!
//! // Explicit OR operator
//! let expr = parse_filter("type:user OR type:agent").unwrap();
//!
//! // Same field gets implicit OR
//! let expr = parse_filter("project:foo project:bar").unwrap();
//!
//! // Quoted values for spaces
//! let expr = parse_filter("project:\"my project\"").unwrap();
//!
//! // Complex query
//! let expr = parse_filter("project:ai-explorer AND type:user since:2024-01-01").unwrap();
//! ```
//!
//! # Operator Precedence
//!
//! - Implicit operators (no keyword): AND for different fields, OR for same field
//! - Explicit operators (AND/OR keywords): Always respected
//!
//! # Validation
//!
//! - `type` values must be "user" or "agent" (case-insensitive)
//! - `since` dates must be YYYY-MM-DD format and semantically valid
//! - Empty field names or values are rejected

use anyhow::{Context, Result, anyhow};
use chrono::NaiveDate;

use super::ast::{FieldFilter, FilterExpr, FilterField, FilterOperator};

/// Token types produced by the tokenizer
#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    /// field:value or field:"quoted value"
    FieldValue { field: String, value: String },
    /// AND keyword
    And,
    /// OR keyword
    Or,
}

/// Tokenize filter input string into tokens
///
/// Supports:
/// - field:value patterns
/// - field:"quoted value" with spaces
/// - AND/OR keywords (case-insensitive)
/// - Whitespace separation
fn tokenize(input: &str) -> Result<Vec<Token>> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(&ch) = chars.peek() {
        // Skip whitespace
        if ch.is_whitespace() {
            chars.next();
            continue;
        }

        // Try to parse a word or field:value
        let word = read_word(&mut chars);

        if word.is_empty() {
            return Err(anyhow!("Unexpected character in filter input"));
        }

        // Check if it's an operator keyword
        match word.to_uppercase().as_str() {
            "AND" => tokens.push(Token::And),
            "OR" => tokens.push(Token::Or),
            _ => {
                // Try to parse as field:value
                if let Some(colon_pos) = word.find(':') {
                    let field = word[..colon_pos].to_string();
                    let mut value = word[colon_pos + 1..].to_string();

                    // Check if value starts with quote
                    if value.starts_with('"') {
                        // Need to read quoted value
                        value = read_quoted_value(&mut chars, &value)?;
                    }

                    if field.is_empty() || value.is_empty() {
                        return Err(anyhow!("Invalid field:value format: {}", word));
                    }

                    tokens.push(Token::FieldValue { field, value });
                } else {
                    return Err(anyhow!(
                        "Invalid token: '{}' (expected field:value or AND/OR)",
                        word
                    ));
                }
            }
        }
    }

    Ok(tokens)
}

/// Read a word (until whitespace or end)
fn read_word(chars: &mut std::iter::Peekable<std::str::Chars>) -> String {
    let mut word = String::new();

    while let Some(&ch) = chars.peek() {
        if ch.is_whitespace() {
            break;
        }
        word.push(ch);
        chars.next();
    }

    word
}

/// Read a quoted value, handling the case where word already contains the opening quote
fn read_quoted_value(
    chars: &mut std::iter::Peekable<std::str::Chars>,
    initial: &str,
) -> Result<String> {
    // initial is like "foo or "foo bar" depending on if quote was mid-word
    let mut value = initial[1..].to_string(); // Remove opening quote

    // If the initial part already has closing quote, we're done
    if let Some(quote_pos) = value.find('"') {
        return Ok(value[..quote_pos].to_string());
    }

    // Otherwise keep reading until closing quote
    for ch in chars.by_ref() {
        if ch == '"' {
            return Ok(value);
        }
        value.push(ch);
    }

    Err(anyhow!("Unterminated quoted string"))
}

/// Parse field name into FilterField enum
fn parse_field(field: &str) -> Result<FilterField> {
    match field.to_lowercase().as_str() {
        "project" => Ok(FilterField::Project),
        "type" => Ok(FilterField::Type),
        "since" => Ok(FilterField::Since),
        _ => Err(anyhow!("Unknown field: '{}' (valid fields: project, type, since)", field)),
    }
}

/// Parse filter string into FilterExpr
///
/// Examples:
/// - "project:foo" → single filter
/// - "project:foo type:user" → two filters with implicit AND
/// - "type:user OR type:agent" → two filters with explicit OR
/// - "project:foo project:bar" → two filters with implicit OR (same field)
/// - "project:\"foo bar\"" → filter with quoted value containing spaces
pub fn parse_filter(input: &str) -> Result<FilterExpr> {
    if input.trim().is_empty() {
        return Ok(FilterExpr::new());
    }

    let tokens = tokenize(input).context("Failed to tokenize filter")?;

    if tokens.is_empty() {
        return Ok(FilterExpr::new());
    }

    let mut expr = FilterExpr::new();
    let mut expecting_filter = true;
    let mut last_field: Option<FilterField> = None;

    for token in tokens {
        match token {
            Token::FieldValue { field, value } => {
                let filter_field = parse_field(&field)?;

                // Validate value based on field type
                validate_value(&filter_field, &value)?;

                // Add implicit operator if we're not expecting a filter
                // (meaning there was no explicit operator between filters)
                if !expecting_filter && !expr.filters.is_empty() {
                    // Need to add implicit operator
                    let implicit_op = if let Some(ref prev_field) = last_field {
                        if prev_field == &filter_field {
                            FilterOperator::Or // Same field → OR
                        } else {
                            FilterOperator::And // Different field → AND
                        }
                    } else {
                        FilterOperator::And
                    };
                    expr.add_operator(implicit_op);
                }

                expr.add_filter(FieldFilter::new(filter_field.clone(), value));
                last_field = Some(filter_field);
                expecting_filter = false;
            }
            Token::And => {
                if expecting_filter {
                    return Err(anyhow!("Unexpected AND operator (expected field:value)"));
                }
                expr.add_operator(FilterOperator::And);
                expecting_filter = true;
            }
            Token::Or => {
                if expecting_filter {
                    return Err(anyhow!("Unexpected OR operator (expected field:value)"));
                }
                expr.add_operator(FilterOperator::Or);
                expecting_filter = true;
            }
        }
    }

    if expecting_filter {
        return Err(anyhow!("Filter ended with operator (expected field:value)"));
    }

    // Validate operators count
    if expr.operators.len() != expr.filters.len().saturating_sub(1) {
        return Err(anyhow!(
            "Internal parser error: operator count mismatch (filters: {}, operators: {})",
            expr.filters.len(),
            expr.operators.len()
        ));
    }

    Ok(expr)
}

/// Validate filter value based on field type
fn validate_value(field: &FilterField, value: &str) -> Result<()> {
    match field {
        FilterField::Type => {
            // Must be "user" or "agent"
            match value.to_lowercase().as_str() {
                "user" | "agent" => Ok(()),
                _ => Err(anyhow!("Invalid type value: '{}' (must be 'user' or 'agent')", value)),
            }
        }
        FilterField::Since => {
            // Must be YYYY-MM-DD format
            if !is_valid_date_format(value) {
                return Err(anyhow!("Invalid date format: '{}' (expected YYYY-MM-DD)", value));
            }
            Ok(())
        }
        FilterField::Project => {
            // Any non-empty string is valid
            if value.is_empty() {
                return Err(anyhow!("Project path cannot be empty"));
            }
            Ok(())
        }
    }
}

/// Check if string is valid YYYY-MM-DD format
fn is_valid_date_format(s: &str) -> bool {
    // Enforce strict YYYY-MM-DD format (10 chars)
    if s.len() != 10 {
        return false;
    }
    // Use chrono for semantic validation (e.g., reject 2024-02-31)
    NaiveDate::parse_from_str(s, "%Y-%m-%d").is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_single_field() {
        let tokens = tokenize("project:foo").unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(
            tokens[0],
            Token::FieldValue { field: "project".to_string(), value: "foo".to_string() }
        );
    }

    #[test]
    fn test_tokenize_multiple_fields() {
        let tokens = tokenize("project:foo type:user").unwrap();
        assert_eq!(tokens.len(), 2);
    }

    #[test]
    fn test_tokenize_with_operators() {
        let tokens = tokenize("project:foo AND type:user").unwrap();
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[1], Token::And);
    }

    #[test]
    fn test_tokenize_quoted_value() {
        let tokens = tokenize("project:\"foo bar\"").unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(
            tokens[0],
            Token::FieldValue { field: "project".to_string(), value: "foo bar".to_string() }
        );
    }

    #[test]
    fn test_tokenize_unterminated_quote() {
        let result = tokenize("project:\"foo bar");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unterminated"));
    }

    #[test]
    fn test_tokenize_invalid_token() {
        let result = tokenize("invalid");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid token"));
    }

    #[test]
    fn test_parse_field_valid() {
        assert_eq!(parse_field("project").unwrap(), FilterField::Project);
        assert_eq!(parse_field("type").unwrap(), FilterField::Type);
        assert_eq!(parse_field("since").unwrap(), FilterField::Since);
        assert_eq!(parse_field("PROJECT").unwrap(), FilterField::Project); // Case insensitive
    }

    #[test]
    fn test_parse_field_invalid() {
        let result = parse_field("invalid");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unknown field"));
    }

    #[test]
    fn test_validate_type_value() {
        assert!(validate_value(&FilterField::Type, "user").is_ok());
        assert!(validate_value(&FilterField::Type, "agent").is_ok());
        assert!(validate_value(&FilterField::Type, "USER").is_ok()); // Case insensitive
        assert!(validate_value(&FilterField::Type, "invalid").is_err());
    }

    #[test]
    fn test_validate_date_format() {
        // Valid dates
        assert!(is_valid_date_format("2024-01-15"));
        assert!(is_valid_date_format("2024-12-31"));
        assert!(is_valid_date_format("2024-02-29")); // Leap year

        // Invalid format
        assert!(!is_valid_date_format("2024-1-15")); // Single digit month
        assert!(!is_valid_date_format("24-01-15")); // 2-digit year
        assert!(!is_valid_date_format("2024/01/15")); // Wrong separator

        // Invalid month
        assert!(!is_valid_date_format("2024-13-01"));
        assert!(!is_valid_date_format("2024-00-01"));

        // Invalid day (format check)
        assert!(!is_valid_date_format("2024-01-32"));

        // Semantically invalid dates (format correct but date doesn't exist)
        assert!(!is_valid_date_format("2024-02-31")); // Feb has max 29 days in 2024
        assert!(!is_valid_date_format("2024-04-31")); // April has 30 days
        assert!(!is_valid_date_format("2024-11-31")); // November has 30 days
        assert!(!is_valid_date_format("2023-02-29")); // Not a leap year
    }

    #[test]
    fn test_parse_filter_empty() {
        let expr = parse_filter("").unwrap();
        assert!(expr.is_empty());
    }

    #[test]
    fn test_parse_filter_single() {
        let expr = parse_filter("project:foo").unwrap();
        assert_eq!(expr.filters.len(), 1);
        assert_eq!(expr.operators.len(), 0);
        assert_eq!(expr.filters[0].field, FilterField::Project);
        assert_eq!(expr.filters[0].value, "foo");
    }

    #[test]
    fn test_parse_filter_implicit_and() {
        let expr = parse_filter("project:foo type:user").unwrap();
        assert_eq!(expr.filters.len(), 2);
        assert_eq!(expr.operators.len(), 1);
        assert_eq!(expr.operators[0], FilterOperator::And);
    }

    #[test]
    fn test_parse_filter_explicit_or() {
        let expr = parse_filter("type:user OR type:agent").unwrap();
        assert_eq!(expr.filters.len(), 2);
        assert_eq!(expr.operators.len(), 1);
        assert_eq!(expr.operators[0], FilterOperator::Or);
    }

    #[test]
    fn test_parse_filter_same_field_implicit_or() {
        let expr = parse_filter("project:foo project:bar").unwrap();
        assert_eq!(expr.filters.len(), 2);
        assert_eq!(expr.operators.len(), 1);
        assert_eq!(expr.operators[0], FilterOperator::Or);
    }

    #[test]
    fn test_parse_filter_quoted() {
        let expr = parse_filter("project:\"foo bar\"").unwrap();
        assert_eq!(expr.filters.len(), 1);
        assert_eq!(expr.filters[0].value, "foo bar");
    }

    #[test]
    fn test_parse_filter_invalid_type() {
        let result = parse_filter("type:invalid");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid type"));
    }

    #[test]
    fn test_parse_filter_invalid_date() {
        let result = parse_filter("since:2024-13-01");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid date"));
    }

    #[test]
    fn test_parse_filter_ends_with_operator() {
        let result = parse_filter("project:foo AND");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("ended with operator"));
    }

    #[test]
    fn test_parse_filter_starts_with_operator() {
        let result = parse_filter("AND project:foo");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_filter_complex() {
        let expr = parse_filter("project:foo AND type:user since:2024-01-01").unwrap();
        assert_eq!(expr.filters.len(), 3);
        assert_eq!(expr.operators.len(), 2);
        assert_eq!(expr.operators[0], FilterOperator::And);
        assert_eq!(expr.operators[1], FilterOperator::And); // Implicit AND between type and since
    }

    #[test]
    fn test_tokenize_empty_field_or_value() {
        // Empty field
        let result = tokenize(":value");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid field:value"));

        // Empty value
        let result2 = tokenize("field:");
        assert!(result2.is_err());
        assert!(result2.unwrap_err().to_string().contains("Invalid field:value"));
    }
}
