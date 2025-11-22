use anyhow::{Context, Result};
use arboard::Clipboard;

/// Maximum clipboard size (10MB) to prevent DoS attacks
const MAX_CLIPBOARD_SIZE: usize = 10 * 1024 * 1024;

/// Trait for clipboard operations (allows mocking in tests)
trait ClipboardProvider {
    fn set_text(&mut self, text: &str) -> Result<()>;
}

/// Real clipboard implementation using arboard
struct SystemClipboard {
    clipboard: Clipboard,
}

impl SystemClipboard {
    fn new() -> Result<Self> {
        let clipboard = Clipboard::new().context("Failed to initialize clipboard")?;
        Ok(Self { clipboard })
    }
}

impl ClipboardProvider for SystemClipboard {
    fn set_text(&mut self, text: &str) -> Result<()> {
        self.clipboard.set_text(text).context("Failed to set clipboard contents")?;
        Ok(())
    }
}

/// Validates clipboard text without accessing system clipboard
fn validate_clipboard_text(text: &str) -> Result<()> {
    if text.is_empty() {
        anyhow::bail!("Cannot copy empty text to clipboard");
    }

    if text.len() > MAX_CLIPBOARD_SIZE {
        anyhow::bail!(
            "Text too large for clipboard ({} bytes, max {})",
            text.len(),
            MAX_CLIPBOARD_SIZE
        );
    }

    Ok(())
}

/// Internal function for clipboard operations with dependency injection
fn copy_with_provider(text: &str, provider: &mut dyn ClipboardProvider) -> Result<()> {
    validate_clipboard_text(text)?;
    provider.set_text(text)?;
    Ok(())
}

/// Copy text to the system clipboard.
///
/// # Arguments
/// * `text` - The text to copy to clipboard
///
/// # Returns
/// * `Ok(())` if successful
/// * `Err` if clipboard is unavailable or operation fails
///
/// # Errors
/// Returns error if:
/// - Text is empty
/// - Text is too large for clipboard (>10MB)
/// - Clipboard is locked by another process
/// - Clipboard access is denied (permissions)
/// - System clipboard is unavailable (headless environment)
///
/// # Platform Support
/// - macOS: Primary support via pasteboard API
/// - Linux: X11 (xclip/xsel) or Wayland (wl-clipboard)
/// - Windows: Not officially supported in Phase 2
pub fn copy_to_clipboard(text: &str) -> Result<()> {
    let mut clipboard = SystemClipboard::new()?;
    copy_with_provider(text, &mut clipboard)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock clipboard for testing without system clipboard access
    struct MockClipboard {
        text: Option<String>,
        should_fail: bool,
    }

    impl MockClipboard {
        fn new() -> Self {
            Self { text: None, should_fail: false }
        }

        fn with_failure() -> Self {
            Self { text: None, should_fail: true }
        }

        fn get_text(&self) -> Option<&str> {
            self.text.as_deref()
        }
    }

    impl ClipboardProvider for MockClipboard {
        fn set_text(&mut self, text: &str) -> Result<()> {
            if self.should_fail {
                anyhow::bail!("Mock clipboard error");
            }
            self.text = Some(text.to_string());
            Ok(())
        }
    }

    /// Tests that actually access system clipboard (optional)
    fn should_test_system_clipboard() -> bool {
        std::env::var("ENABLE_CLIPBOARD_TESTS").is_ok()
    }

    #[test]
    fn test_copy_valid_text_with_mock() {
        let mut mock = MockClipboard::new();
        let text = "Hello, clipboard!";

        let result = copy_with_provider(text, &mut mock);

        assert!(result.is_ok());
        assert_eq!(mock.get_text(), Some(text));
    }

    #[test]
    fn test_copy_unicode_with_mock() {
        let mut mock = MockClipboard::new();
        let text = "Hello 世界 🚀 émojis";

        let result = copy_with_provider(text, &mut mock);

        assert!(result.is_ok());
        assert_eq!(mock.get_text(), Some(text));
    }

    #[test]
    fn test_copy_multiline_with_mock() {
        let mut mock = MockClipboard::new();
        let text = "Line 1\nLine 2\nLine 3\n";

        let result = copy_with_provider(text, &mut mock);

        assert!(result.is_ok());
        assert_eq!(mock.get_text(), Some(text));
    }

    #[test]
    fn test_clipboard_provider_failure() {
        let mut mock = MockClipboard::with_failure();
        let text = "This should fail";

        let result = copy_with_provider(text, &mut mock);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Mock clipboard error"));
    }

    #[test]
    fn test_copy_empty_text() {
        let result = copy_to_clipboard("");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("empty"));
    }

    #[test]
    fn test_copy_large_text() {
        // Create 11MB of text (exceeds 10MB limit)
        let large_text = "a".repeat(11 * 1024 * 1024);
        let result = copy_to_clipboard(&large_text);

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("too large"));
    }

    #[test]
    fn test_copy_exactly_at_limit() {
        // Create exactly 10MB of text (should pass validation)
        let text_at_limit = "a".repeat(10 * 1024 * 1024);
        let result = copy_to_clipboard(&text_at_limit);

        // Should fail only due to clipboard unavailability, not size validation
        if let Err(e) = result {
            let err_msg = e.to_string().to_lowercase();
            assert!(!err_msg.contains("too large"), "10MB exactly should pass validation: {}", e);
        }
    }

    #[test]
    fn test_copy_one_byte_over_limit() {
        // Create 10MB + 1 byte (should fail validation)
        let text_over_limit = "a".repeat(10 * 1024 * 1024 + 1);
        let result = copy_to_clipboard(&text_over_limit);

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("too large"));
    }

    #[test]
    fn test_error_message_includes_size_info() {
        // Test that error messages include helpful size information
        let large_text = "a".repeat(15 * 1024 * 1024);
        let result = copy_to_clipboard(&large_text);

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("too large"));
        assert!(err_msg.contains("bytes"));
    }

    #[test]
    fn test_empty_text_error_message() {
        // Test that empty text has clear error message
        let result = copy_to_clipboard("");

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("empty"));
    }

    #[test]
    fn test_whitespace_only_text() {
        // Whitespace-only text should be accepted (not considered empty)
        let text = "   \n\t  ";
        let result = copy_to_clipboard(text);

        // Should fail only due to clipboard, not validation
        if let Err(e) = result {
            let err_msg = e.to_string().to_lowercase();
            assert!(
                !err_msg.contains("empty"),
                "Whitespace should not be rejected as empty: {}",
                e
            );
        }
    }

    #[test]
    fn test_single_character() {
        // Single character should be valid
        let result = copy_to_clipboard("a");

        // Should fail only due to clipboard, not validation
        if let Err(e) = result {
            let err_msg = e.to_string().to_lowercase();
            assert!(
                !err_msg.contains("empty") && !err_msg.contains("too large"),
                "Single character should pass validation: {}",
                e
            );
        }
    }

    #[test]
    fn test_multibyte_unicode_size_calculation() {
        // Test that size is calculated in bytes, not characters
        // "🚀" is 4 bytes in UTF-8
        let emoji = "🚀";
        assert_eq!(emoji.len(), 4); // Verify it's 4 bytes

        // Create text with multibyte characters
        let text = emoji.repeat(3 * 1024 * 1024); // 12MB in bytes
        let result = copy_to_clipboard(&text);

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("too large"));
    }

    #[test]
    fn test_system_clipboard_integration() {
        if !should_test_system_clipboard() {
            // Skip actual system clipboard test in CI
            return;
        }

        // Test with actual system clipboard
        let text = "System clipboard test";
        let result = copy_to_clipboard(text);

        // May fail in headless environments
        if let Err(e) = result {
            eprintln!("System clipboard unavailable (expected in CI): {}", e);
        }
    }
}
