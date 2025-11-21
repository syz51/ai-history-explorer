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
}
