use chrono::{DateTime, Utc};
use serde::de::Error;
use serde::{Deserialize, Deserializer};
use serde_json::Value;
use uuid::Uuid;

/// Custom deserializer for timestamp that accepts both integers (ms) and RFC3339 strings
pub fn deserialize_timestamp<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    match value {
        Value::Number(n) => {
            // Assume it's a Unix timestamp in milliseconds
            let ms = n.as_i64().ok_or_else(|| Error::custom("invalid timestamp"))?;
            DateTime::from_timestamp_millis(ms)
                .ok_or_else(|| Error::custom("timestamp out of range"))
        }
        Value::String(s) => {
            // Parse as RFC3339
            s.parse::<DateTime<Utc>>()
                .map_err(|e| Error::custom(format!("invalid RFC3339 timestamp: {}", e)))
        }
        _ => Err(Error::custom("timestamp must be a number or string")),
    }
}

/// Custom deserializer for session IDs that validates UUID format
pub fn deserialize_session_id<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;

    // Validate that it's not empty
    if s.is_empty() {
        return Err(Error::custom("session ID cannot be empty"));
    }

    // Validate that it's a valid UUID
    Uuid::parse_str(&s)
        .map_err(|e| Error::custom(format!("invalid UUID format for session ID: {}", e)))?;

    Ok(s)
}

#[cfg(test)]
mod tests {
    use chrono::DateTime;

    use crate::models::HistoryEntry;

    #[test]
    fn test_history_entry_timestamp_integer() {
        let json = r#"{
            "display": "test prompt",
            "timestamp": 1762076480016,
            "project": "/Users/test/project",
            "sessionId": "550e8400-e29b-41d4-a716-446655440000"
        }"#;

        let entry: HistoryEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.display, "test prompt");
        assert_eq!(entry.session_id, "550e8400-e29b-41d4-a716-446655440000");

        // Verify timestamp is parsed correctly (Nov 2, 2025 09:41:20 UTC)
        let expected_ts = DateTime::from_timestamp_millis(1762076480016).unwrap();
        assert_eq!(entry.timestamp, expected_ts);
    }

    #[test]
    fn test_history_entry_timestamp_rfc3339() {
        let json = r#"{
            "display": "test prompt",
            "timestamp": "2025-11-02T09:41:20.016Z",
            "project": "/Users/test/project",
            "sessionId": "550e8400-e29b-41d4-a716-446655440001"
        }"#;

        let entry: HistoryEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.display, "test prompt");
        assert_eq!(entry.session_id, "550e8400-e29b-41d4-a716-446655440001");
    }

    #[test]
    fn test_parse_history_entry_with_optional_fields() {
        let json = r#"{
            "display": "test",
            "timestamp": 1762076480016,
            "sessionId": "550e8400-e29b-41d4-a716-446655440002"
        }"#;

        let entry: HistoryEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.display, "test");
        assert_eq!(entry.session_id, "550e8400-e29b-41d4-a716-446655440002");
        assert!(entry.project.is_none());
        assert!(entry.pasted_contents.is_none());
    }

    // ===== Security Tests: Session ID Validation =====

    #[test]
    fn test_empty_session_id() {
        let json = r#"{
            "display": "test",
            "timestamp": 1762076480016,
            "sessionId": ""
        }"#;

        let result: Result<HistoryEntry, _> = serde_json::from_str(json);
        assert!(result.is_err(), "Empty session ID should fail validation");
        assert!(result.unwrap_err().to_string().contains("session ID cannot be empty"));
    }

    #[test]
    fn test_invalid_uuid_format() {
        let json = r#"{
            "display": "test",
            "timestamp": 1762076480016,
            "sessionId": "not-a-valid-uuid"
        }"#;

        let result: Result<HistoryEntry, _> = serde_json::from_str(json);
        assert!(result.is_err(), "Invalid UUID format should fail validation");
        assert!(result.unwrap_err().to_string().contains("invalid UUID format"));
    }

    #[test]
    fn test_malformed_uuid_with_wrong_length() {
        let json = r#"{
            "display": "test",
            "timestamp": 1762076480016,
            "sessionId": "550e8400-e29b-41d4-a716"
        }"#;

        let result: Result<HistoryEntry, _> = serde_json::from_str(json);
        assert!(result.is_err(), "UUID with wrong length should fail validation");
    }

    #[test]
    fn test_uuid_with_invalid_characters() {
        let json = r#"{
            "display": "test",
            "timestamp": 1762076480016,
            "sessionId": "550e8400-e29b-41d4-a716-44665544000g"
        }"#;

        let result: Result<HistoryEntry, _> = serde_json::from_str(json);
        assert!(result.is_err(), "UUID with invalid characters should fail validation");
    }

    #[test]
    fn test_numeric_session_id() {
        let json = r#"{
            "display": "test",
            "timestamp": 1762076480016,
            "sessionId": "12345678-1234-1234-1234-123456789012"
        }"#;

        // This is actually a valid UUID (hex digits can be all numeric)
        let result: Result<HistoryEntry, _> = serde_json::from_str(json);
        assert!(result.is_ok(), "Valid UUID with numeric hex digits should pass");
    }

    // ===== Edge Case Tests: Timestamp Validation =====

    #[test]
    fn test_negative_timestamp() {
        let json = r#"{
            "display": "test",
            "timestamp": -1000,
            "sessionId": "550e8400-e29b-41d4-a716-446655440000"
        }"#;

        // Negative timestamps are valid (before Unix epoch, e.g., Dec 31, 1969)
        let result: Result<HistoryEntry, _> = serde_json::from_str(json);
        assert!(result.is_ok(), "Negative timestamp should be valid (before Unix epoch)");
    }

    #[test]
    fn test_far_future_timestamp() {
        let json = r#"{
            "display": "test",
            "timestamp": 253402300799999,
            "sessionId": "550e8400-e29b-41d4-a716-446655440000"
        }"#;

        // This is Dec 31, 9999 23:59:59.999 UTC - should be within valid range
        let result: Result<HistoryEntry, _> = serde_json::from_str(json);
        assert!(result.is_ok(), "Far future timestamp should be valid if within range");
    }

    #[test]
    fn test_timestamp_overflow() {
        let json = r#"{
            "display": "test",
            "timestamp": 9223372036854775807,
            "sessionId": "550e8400-e29b-41d4-a716-446655440000"
        }"#;

        // i64::MAX - will overflow DateTime range
        let result: Result<HistoryEntry, _> = serde_json::from_str(json);
        assert!(result.is_err(), "Timestamp overflow should fail validation");
        assert!(result.unwrap_err().to_string().contains("out of range"));
    }

    #[test]
    fn test_invalid_rfc3339_timestamp() {
        let json = r#"{
            "display": "test",
            "timestamp": "not-a-valid-date",
            "sessionId": "550e8400-e29b-41d4-a716-446655440000"
        }"#;

        let result: Result<HistoryEntry, _> = serde_json::from_str(json);
        assert!(result.is_err(), "Invalid RFC3339 string should fail validation");
        assert!(result.unwrap_err().to_string().contains("invalid RFC3339"));
    }

    #[test]
    fn test_timestamp_wrong_type() {
        let json = r#"{
            "display": "test",
            "timestamp": true,
            "sessionId": "550e8400-e29b-41d4-a716-446655440000"
        }"#;

        let result: Result<HistoryEntry, _> = serde_json::from_str(json);
        assert!(result.is_err(), "Boolean timestamp should fail validation");
        assert!(result.unwrap_err().to_string().contains("must be a number or string"));
    }

    #[test]
    fn test_timestamp_zero() {
        let json = r#"{
            "display": "test",
            "timestamp": 0,
            "sessionId": "550e8400-e29b-41d4-a716-446655440000"
        }"#;

        // Timestamp 0 = Jan 1, 1970 00:00:00 UTC - valid
        let result: Result<HistoryEntry, _> = serde_json::from_str(json);
        assert!(result.is_ok(), "Timestamp 0 (Unix epoch) should be valid");
    }

    #[test]
    fn test_timestamp_with_nanoseconds() {
        let json = r#"{
            "display": "test",
            "timestamp": "2025-11-02T09:41:20.123456789Z",
            "sessionId": "550e8400-e29b-41d4-a716-446655440000"
        }"#;

        // RFC3339 with nanosecond precision
        let result: Result<HistoryEntry, _> = serde_json::from_str(json);
        assert!(result.is_ok(), "RFC3339 with nanoseconds should be valid");
    }
}
