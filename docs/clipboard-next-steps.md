# Clipboard Integration - Next Steps

## Worker A Integration (Blocking for Phase 2 TUI)

Per plan `plans/phase2-tui/implementation.md` lines 154-162, Worker A must:

- [ ] Hook Enter key handler to call `copy_to_clipboard()`
- [ ] Pass `SearchEntry.display_text` to clipboard function
- [ ] Display success in status bar: "✓ Copied to clipboard" (3s timeout)
- [ ] Display failure in status bar: "✗ Clipboard error: {reason}"
- [ ] Handle edge cases gracefully in UI

**Integration example:**

```rust
// In TUI event loop
match key_event {
    KeyCode::Enter => {
        match copy_to_clipboard(&selected_entry.display_text) {
            Ok(()) => status_bar.set_message("✓ Copied to clipboard", 3000),
            Err(e) => status_bar.set_error(&format!("✗ {}", e)),
        }
    }
}
```

## Optional Improvements (Non-blocking)

### 1. Explicit trait visibility (minor)

```rust
// Current (implicit private)
trait ClipboardProvider { ... }

// Suggested
pub(crate) trait ClipboardProvider { ... }
```

**Benefit:** Makes internal intent explicit

### 2. Documentation examples

Add usage example to `copy_to_clipboard()` docs:

````rust
/// # Example
/// ```no_run
/// use ai_history_explorer::copy_to_clipboard;
///
/// copy_to_clipboard("Search result text")?;
/// # Ok::<(), anyhow::Error>(())
/// ```
````

## Phase 3+ Features (Deferred)

- [ ] Clipboard history/undo functionality
- [ ] `UndoableClipboard` wrapper using trait-based design
- [ ] Multi-select copy (copy multiple entries at once)

## Status

**Current:** Clipboard library complete, approved, ready to merge
**Next:** Worker A integration in TUI event loop
**Dependencies:** Worker A must complete TUI core (event loop, status bar) first
