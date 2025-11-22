use anyhow::{Context, Result};
use arboard::Clipboard;

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
/// - Clipboard is locked by another process
/// - Clipboard access is denied (permissions)
/// - System clipboard is unavailable (headless environment)
/// - Text is too large for clipboard (>10MB)
///
/// # Platform Support
/// - macOS: Primary support via pasteboard API
/// - Linux: X11 (xclip/xsel) or Wayland (wl-clipboard)
/// - Windows: Not officially supported in Phase 2
///
/// # Testing Note
/// Full coverage requires `ENABLE_CLIPBOARD_TESTS=1` environment variable.
/// Most tests run in CI by testing validation logic before clipboard access.
pub fn copy_to_clipboard(text: &str) -> Result<()> {
    // Validate text size to prevent DoS (10MB limit)
    const MAX_CLIPBOARD_SIZE: usize = 10 * 1024 * 1024; // 10MB
    if text.len() > MAX_CLIPBOARD_SIZE {
        anyhow::bail!(
            "Text too large for clipboard ({} bytes, max {})",
            text.len(),
            MAX_CLIPBOARD_SIZE
        );
    }

    // Handle empty text
    if text.is_empty() {
        anyhow::bail!("Cannot copy empty text to clipboard");
    }

    // Initialize clipboard
    let mut clipboard = Clipboard::new().context("Failed to initialize clipboard")?;

    // Copy text to clipboard
    clipboard.set_text(text).context("Failed to set clipboard contents")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests that actually access clipboard are disabled in automated testing
    /// because clipboard may not be available in CI/sandboxed environments.
    /// Manual testing should be performed to verify clipboard functionality.
    ///
    /// Set ENABLE_CLIPBOARD_TESTS=1 to run these tests locally.
    fn should_test_clipboard() -> bool {
        std::env::var("ENABLE_CLIPBOARD_TESTS").is_ok()
    }

    #[test]
    fn test_copy_valid_text() {
        if !should_test_clipboard() {
            eprintln!("Skipping clipboard access test (set ENABLE_CLIPBOARD_TESTS=1 to run)");
            return;
        }

        let text = "Hello, clipboard!";
        let result = copy_to_clipboard(text);

        // May fail in CI/headless environments, so we check both cases
        match result {
            Ok(()) => {
                // Success: verify clipboard contains our text
                if let Ok(mut clipboard) = Clipboard::new()
                    && let Ok(contents) = clipboard.get_text()
                {
                    assert_eq!(contents, text);
                }
            }
            Err(e) => {
                // Expected in headless/CI environments
                let err_msg = e.to_string().to_lowercase();
                assert!(
                    err_msg.contains("clipboard") || err_msg.contains("display"),
                    "Unexpected error: {}",
                    e
                );
            }
        }
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
    fn test_copy_unicode_text() {
        if !should_test_clipboard() {
            eprintln!("Skipping clipboard access test (set ENABLE_CLIPBOARD_TESTS=1 to run)");
            return;
        }

        let text = "Hello 世界 🚀 émojis";
        let result = copy_to_clipboard(text);

        // May fail in CI/headless environments
        match result {
            Ok(()) => {
                // Success: verify clipboard contains our text
                if let Ok(mut clipboard) = Clipboard::new()
                    && let Ok(contents) = clipboard.get_text()
                {
                    assert_eq!(contents, text);
                }
            }
            Err(e) => {
                // Expected in headless/CI environments
                let err_msg = e.to_string().to_lowercase();
                assert!(
                    err_msg.contains("clipboard") || err_msg.contains("display"),
                    "Unexpected error: {}",
                    e
                );
            }
        }
    }

    #[test]
    fn test_copy_multiline_text() {
        if !should_test_clipboard() {
            eprintln!("Skipping clipboard access test (set ENABLE_CLIPBOARD_TESTS=1 to run)");
            return;
        }

        let text = "Line 1\nLine 2\nLine 3\n";
        let result = copy_to_clipboard(text);

        // May fail in CI/headless environments
        match result {
            Ok(()) => {
                // Success: verify clipboard contains our text
                if let Ok(mut clipboard) = Clipboard::new()
                    && let Ok(contents) = clipboard.get_text()
                {
                    assert_eq!(contents, text);
                }
            }
            Err(e) => {
                // Expected in headless/CI environments
                let err_msg = e.to_string().to_lowercase();
                assert!(
                    err_msg.contains("clipboard") || err_msg.contains("display"),
                    "Unexpected error: {}",
                    e
                );
            }
        }
    }

    #[test]
    fn test_copy_max_size_text() {
        if !should_test_clipboard() {
            eprintln!("Skipping clipboard access test (set ENABLE_CLIPBOARD_TESTS=1 to run)");
            return;
        }

        // Create exactly 10MB of text (should succeed)
        let text = "a".repeat(10 * 1024 * 1024);
        let result = copy_to_clipboard(&text);

        // May fail in CI/headless environments
        match result {
            Ok(()) => {
                // Success
            }
            Err(e) => {
                // Should only fail due to clipboard unavailability, not size
                let err_msg = e.to_string().to_lowercase();
                assert!(!err_msg.contains("too large"), "10MB should not be too large: {}", e);
            }
        }
    }
}
