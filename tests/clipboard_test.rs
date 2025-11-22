use ai_history_explorer::copy_to_clipboard;
use arboard::Clipboard;

/// Tests that actually access clipboard are disabled in automated testing
/// Set ENABLE_CLIPBOARD_TESTS=1 to run these tests locally.
fn should_test_clipboard() -> bool {
    std::env::var("ENABLE_CLIPBOARD_TESTS").is_ok()
}

#[test]
fn test_clipboard_integration_basic() {
    if !should_test_clipboard() {
        eprintln!("Skipping clipboard access test (set ENABLE_CLIPBOARD_TESTS=1 to run)");
        return;
    }

    let test_text = "Integration test: basic clipboard copy";
    let result = copy_to_clipboard(test_text);

    match result {
        Ok(()) => {
            // Verify clipboard contents if clipboard is available
            if let Ok(mut clipboard) = Clipboard::new()
                && let Ok(contents) = clipboard.get_text()
            {
                assert_eq!(contents, test_text, "Clipboard should contain the copied text");
            }
        }
        Err(e) => {
            // Expected in CI/headless environments
            let err_msg = e.to_string().to_lowercase();
            assert!(
                err_msg.contains("clipboard") || err_msg.contains("display"),
                "Unexpected error type: {}",
                e
            );
        }
    }
}

#[test]
fn test_clipboard_integration_overwrite() {
    if !should_test_clipboard() {
        eprintln!("Skipping clipboard access test (set ENABLE_CLIPBOARD_TESTS=1 to run)");
        return;
    }

    // Test that new copy operations overwrite previous clipboard contents
    let text1 = "First text";
    let text2 = "Second text - should overwrite first";

    let result1 = copy_to_clipboard(text1);
    let result2 = copy_to_clipboard(text2);

    match (result1, result2) {
        (Ok(()), Ok(())) => {
            // Verify clipboard contains the second text
            if let Ok(mut clipboard) = Clipboard::new()
                && let Ok(contents) = clipboard.get_text()
            {
                assert_eq!(contents, text2, "Clipboard should contain the most recent text");
            }
        }
        (Err(e), _) | (_, Err(e)) => {
            // Expected in CI/headless environments
            let err_msg = e.to_string().to_lowercase();
            assert!(
                err_msg.contains("clipboard") || err_msg.contains("display"),
                "Unexpected error type: {}",
                e
            );
        }
    }
}

#[test]
fn test_clipboard_integration_special_characters() {
    if !should_test_clipboard() {
        eprintln!("Skipping clipboard access test (set ENABLE_CLIPBOARD_TESTS=1 to run)");
        return;
    }

    // Test with various special characters that might appear in conversation history
    let test_cases = vec![
        "Text with\nnewlines\nand\ttabs",
        "Unicode: 世界 🚀 émoji",
        "Code: fn main() { println!(\"Hello\"); }",
        "JSON: {\"key\": \"value\", \"nested\": {\"a\": 1}}",
        "Markdown: **bold** _italic_ `code` [link](url)",
    ];

    for test_text in test_cases {
        let result = copy_to_clipboard(test_text);

        match result {
            Ok(()) => {
                // Verify clipboard contents if clipboard is available
                if let Ok(mut clipboard) = Clipboard::new()
                    && let Ok(contents) = clipboard.get_text()
                {
                    assert_eq!(contents, test_text, "Clipboard should preserve special characters");
                }
            }
            Err(e) => {
                // Expected in CI/headless environments
                let err_msg = e.to_string().to_lowercase();
                assert!(
                    err_msg.contains("clipboard") || err_msg.contains("display"),
                    "Unexpected error type for text '{}': {}",
                    test_text,
                    e
                );
            }
        }
    }
}

// Error validation tests are in unit tests (src/clipboard/mod.rs)
// where they use MockClipboard to avoid system clipboard dependency

#[test]
fn test_clipboard_integration_realistic_conversation_entry() {
    if !should_test_clipboard() {
        eprintln!("Skipping clipboard access test (set ENABLE_CLIPBOARD_TESTS=1 to run)");
        return;
    }

    // Simulate a realistic conversation entry that would be copied from the TUI
    let realistic_entry = r#"User prompt from 2024-11-22 01:15:32 UTC
Project: ~/Documents/ai-history-explorer
Session: abc123-def456-789

How do I implement clipboard functionality for the TUI? I need to:
1. Add arboard dependency
2. Create clipboard module with copy function
3. Handle errors gracefully
4. Support macOS and Linux

Please provide a complete implementation with tests."#;

    let result = copy_to_clipboard(realistic_entry);

    match result {
        Ok(()) => {
            // Verify clipboard contents if clipboard is available
            if let Ok(mut clipboard) = Clipboard::new()
                && let Ok(contents) = clipboard.get_text()
            {
                assert_eq!(contents, realistic_entry, "Clipboard should contain the full entry");
            }
        }
        Err(e) => {
            // Expected in CI/headless environments
            let err_msg = e.to_string().to_lowercase();
            assert!(
                err_msg.contains("clipboard") || err_msg.contains("display"),
                "Unexpected error type: {}",
                e
            );
        }
    }
}

#[test]
fn test_clipboard_integration_boundary_size() {
    if !should_test_clipboard() {
        eprintln!("Skipping clipboard access test (set ENABLE_CLIPBOARD_TESTS=1 to run)");
        return;
    }

    // Test text at exactly the 10MB boundary
    let boundary_text = "a".repeat(10 * 1024 * 1024); // Exactly 10MB
    let result = copy_to_clipboard(&boundary_text);

    match result {
        Ok(()) => {
            // Success - 10MB should be accepted
        }
        Err(e) => {
            // Should only fail due to clipboard unavailability, not size
            let err_msg = e.to_string().to_lowercase();
            assert!(
                !err_msg.contains("too large"),
                "10MB exactly should not be rejected for size: {}",
                e
            );
        }
    }

    // Test just over the boundary
    let over_boundary_text = "a".repeat(10 * 1024 * 1024 + 1); // 10MB + 1 byte
    let result = copy_to_clipboard(&over_boundary_text);
    assert!(result.is_err(), "Should reject text just over 10MB boundary");
    assert!(result.unwrap_err().to_string().contains("too large"));
}
