//! Terminal output sanitization utilities
//!
//! # Security: Terminal Injection Prevention
//!
//! User-controlled data from JSONL files should be sanitized before display to prevent
//! terminal injection attacks using ANSI escape sequences. Malicious sequences could:
//! - Clear the screen or move the cursor
//! - Change terminal colors or styles
//! - Trigger unexpected terminal behavior
//!
//! **Current mitigation**: The `stats` command does not display user content directly,
//! only summary statistics. If future commands display `display_text` or other user
//! content, they should use [`strip_ansi_codes`] to sanitize output.

/// Strips ANSI escape codes from a string
///
/// Removes ANSI CSI (Control Sequence Introducer) escape codes that could
/// affect terminal display. This prevents terminal injection attacks where
/// malicious data contains escape sequences.
///
/// # Examples
///
/// ```
/// use ai_history_explorer::utils::terminal::strip_ansi_codes;
///
/// let text = "\x1b[31mRed text\x1b[0m";
/// assert_eq!(strip_ansi_codes(text), "Red text");
/// ```
///
/// # Security Note
///
/// This function removes common ANSI CSI sequences (ESC[...m for colors/styles,
/// ESC[...H for cursor movement, etc.). It also removes other control characters
/// like bell (\x07) and backspace (\x08).
pub fn strip_ansi_codes(text: &str) -> String {
    // Remove ANSI CSI sequences: ESC [ ... (letter)
    // Pattern: \x1b\[([0-9;]*)[A-Za-z]
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            // Check for CSI sequence: ESC [
            if chars.peek() == Some(&'[') {
                chars.next(); // consume '['
                // Skip until we find a letter (end of CSI sequence)
                while let Some(&next_ch) = chars.peek() {
                    chars.next();
                    if next_ch.is_ascii_alphabetic() {
                        break;
                    }
                }
                continue;
            }
        }

        // Filter out other control characters (except tab, newline, carriage return)
        if ch.is_control() && ch != '\t' && ch != '\n' && ch != '\r' {
            continue;
        }

        result.push(ch);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_ansi_codes_color() {
        let text = "\x1b[31mRed text\x1b[0m normal";
        assert_eq!(strip_ansi_codes(text), "Red text normal");
    }

    #[test]
    fn test_strip_ansi_codes_cursor_movement() {
        let text = "\x1b[2J\x1b[H Cleared screen";
        assert_eq!(strip_ansi_codes(text), " Cleared screen");
    }

    #[test]
    fn test_strip_ansi_codes_multiple_sequences() {
        let text = "\x1b[1m\x1b[31mBold Red\x1b[0m\x1b[32m Green\x1b[0m";
        assert_eq!(strip_ansi_codes(text), "Bold Red Green");
    }

    #[test]
    fn test_strip_ansi_codes_bell() {
        let text = "Alert! \x07";
        assert_eq!(strip_ansi_codes(text), "Alert! ");
    }

    #[test]
    fn test_strip_ansi_codes_plain_text() {
        let text = "Plain text with no codes";
        assert_eq!(strip_ansi_codes(text), "Plain text with no codes");
    }

    #[test]
    fn test_strip_ansi_codes_preserves_newlines() {
        let text = "Line 1\nLine 2\rLine 3\tTabbed";
        assert_eq!(strip_ansi_codes(text), "Line 1\nLine 2\rLine 3\tTabbed");
    }

    #[test]
    fn test_strip_ansi_codes_unicode() {
        let text = "Hello 👋 \x1b[31mWorld\x1b[0m 🌍";
        assert_eq!(strip_ansi_codes(text), "Hello 👋 World 🌍");
    }

    #[test]
    fn test_strip_ansi_codes_empty() {
        assert_eq!(strip_ansi_codes(""), "");
    }

    #[test]
    fn test_strip_ansi_codes_only_escape_sequences() {
        let text = "\x1b[31m\x1b[0m\x1b[2J";
        assert_eq!(strip_ansi_codes(text), "");
    }

    #[test]
    fn test_strip_ansi_codes_backspace() {
        let text = "Test\x08";
        assert_eq!(strip_ansi_codes(text), "Test");
    }
}
