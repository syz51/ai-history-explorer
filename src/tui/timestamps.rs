use chrono::{DateTime, Datelike, Utc};

/// Format timestamp with tiered display:
/// - Relative for <7 days: "2h ago", "3d ago"
/// - Absolute for ≥7 days: "Jan 15", "Dec 3, 2024"
pub fn format_timestamp(timestamp: &DateTime<Utc>) -> String {
    let now = Utc::now();
    let duration = now.signed_duration_since(*timestamp);

    if duration.num_days() < 7 {
        // Relative format
        format_relative(duration.num_seconds())
    } else {
        // Absolute format
        format_absolute(timestamp, &now)
    }
}

fn format_relative(seconds: i64) -> String {
    let minutes = seconds / 60;
    let hours = minutes / 60;
    let days = hours / 24;

    if days > 0 {
        format!("{}d ago", days)
    } else if hours > 0 {
        format!("{}h ago", hours)
    } else if minutes > 0 {
        format!("{}m ago", minutes)
    } else {
        "just now".to_string()
    }
}

fn format_absolute(timestamp: &DateTime<Utc>, now: &DateTime<Utc>) -> String {
    let same_year = timestamp.year() == now.year();

    if same_year {
        // "Jan 15"
        timestamp.format("%b %-d").to_string()
    } else {
        // "Dec 3, 2024"
        timestamp.format("%b %-d, %Y").to_string()
    }
}

#[cfg(test)]
mod tests {
    use chrono::Duration;

    use super::*;

    #[test]
    fn test_format_relative_just_now() {
        let now = Utc::now();
        let timestamp = now - Duration::seconds(30);
        assert_eq!(format_timestamp(&timestamp), "just now");
    }

    #[test]
    fn test_format_relative_minutes() {
        let now = Utc::now();
        let timestamp = now - Duration::minutes(45);
        assert_eq!(format_timestamp(&timestamp), "45m ago");
    }

    #[test]
    fn test_format_relative_hours() {
        let now = Utc::now();
        let timestamp = now - Duration::hours(3);
        assert_eq!(format_timestamp(&timestamp), "3h ago");
    }

    #[test]
    fn test_format_relative_days() {
        let now = Utc::now();
        let timestamp = now - Duration::days(5);
        assert_eq!(format_timestamp(&timestamp), "5d ago");
    }

    #[test]
    fn test_format_absolute_same_year() {
        // Mock current time as Dec 31, 2024
        let now = Utc::now();
        let timestamp = now - Duration::days(30); // ~30 days ago, same year

        let formatted = format_timestamp(&timestamp);
        // Should be month + day, no year
        assert!(!formatted.contains(&now.year().to_string()));
        assert!(formatted.contains(&timestamp.format("%b").to_string()));
    }

    #[test]
    fn test_format_absolute_different_year() {
        let now = Utc::now();
        let timestamp = now - Duration::days(400); // Over a year ago

        let formatted = format_timestamp(&timestamp);
        // Should include year
        assert!(formatted.contains(&timestamp.year().to_string()));
    }
}
