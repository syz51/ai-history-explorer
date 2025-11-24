# Manual Testing Checklist

**Purpose**: Tests for TTY-dependent and platform-specific functionality that cannot be fully automated

**Platform**: macOS/Linux (Windows support deferred to Phase 3+)

---

## Terminal Restoration Tests

### Test 1: Normal Exit (Ctrl+Q)

- [ ] Launch TUI: `cargo run`
- [ ] Type some search queries, navigate results
- [ ] Exit with `Ctrl+Q`
- [ ] **Expected**: Terminal cursor visible, no leftover UI artifacts
- [ ] **Expected**: Terminal not corrupted, can type commands normally

### Test 2: Interrupt (Ctrl+C)

- [ ] Launch TUI: `cargo run`
- [ ] Type some search queries
- [ ] Press `Ctrl+C`
- [ ] **Expected**: Terminal restored cleanly
- [ ] **Expected**: No error messages
- [ ] **Expected**: Can type commands normally

### Test 3: Force Kill (kill -9)

- [ ] Launch TUI: `cargo run`
- [ ] In another terminal: `ps aux | grep ai-history`
- [ ] `kill -9 <PID>`
- [ ] Return to original terminal
- [ ] **Expected**: Terminal might be corrupted (expected behavior for SIGKILL)
- [ ] Run `reset` to restore terminal
- [ ] **Known Limitation**: SIGKILL bypasses cleanup handlers (OS limitation)

---

## Clipboard Tests

### macOS Clipboard

- [ ] Launch TUI: `cargo run`
- [ ] Navigate to an entry
- [ ] Press `y` to copy
- [ ] **Expected**: See "Copied to clipboard!" status message (2s)
- [ ] Open TextEdit or terminal
- [ ] Paste (Cmd+V)
- [ ] **Expected**: Entry text pasted correctly
- [ ] **Expected**: Multiline text preserves newlines

### Linux Clipboard

- [ ] Ensure `xclip` or `wl-clipboard` installed
- [ ] Launch TUI: `cargo run`
- [ ] Navigate to an entry
- [ ] Press `y` to copy
- [ ] **Expected**: See "Copied to clipboard!" status message
- [ ] Open text editor
- [ ] Paste (Ctrl+V)
- [ ] **Expected**: Entry text pasted correctly
- [ ] **Expected**: Multiline text preserves newlines

### Edge Cases

- [ ] Copy entry with very long text (>10KB)
  - **Expected**: Success or error message
  - **Expected**: No crash
- [ ] Copy entry with Unicode characters (emoji, CJK)
  - **Expected**: Unicode preserved in clipboard
- [ ] Copy entry with special characters (`"`, `\n`, `\t`)
  - **Expected**: Special chars preserved correctly

---

## Terminal Size Tests

### Minimum Size

- [ ] Resize terminal to 80x24 (standard minimum)
- [ ] Launch TUI: `cargo run`
- [ ] **Expected**: UI renders without artifacts
- [ ] **Expected**: Status bar visible
- [ ] **Expected**: Results list visible
- [ ] **Expected**: Preview pane visible

### Very Small Size (Edge Case)

- [ ] Resize terminal to 40x10
- [ ] Launch TUI: `cargo run`
- [ ] **Expected**: UI degrades gracefully (may clip content)
- [ ] **Expected**: No panic or crash
- [ ] Press `j`, `k`, `/` keys
- [ ] **Expected**: Still responsive

### Large Size

- [ ] Resize terminal to fullscreen (1920x1080 or larger)
- [ ] Launch TUI: `cargo run`
- [ ] **Expected**: UI scales properly
- [ ] **Expected**: No wasted space
- [ ] **Expected**: Text wraps correctly in preview

### Dynamic Resizing

- [ ] Launch TUI: `cargo run`
- [ ] Resize terminal while TUI running (drag corner)
- [ ] **Expected**: UI redraws correctly
- [ ] **Expected**: No artifacts or corruption
- [ ] **Expected**: Selection index preserved

---

## Stress Tests

### Rapid Input

- [ ] Launch TUI: `cargo run`
- [ ] Rapidly press `j` and `k` (navigation)
- [ ] **Expected**: Selection updates smoothly
- [ ] **Expected**: No crashes
- [ ] Type search query rapidly: `asdfghjkl;`
- [ ] **Expected**: All characters registered
- [ ] **Expected**: Fuzzy search updates

### Filter Toggle Spam

- [ ] Launch TUI: `cargo run`
- [ ] Rapidly press `Ctrl+F` (toggle filter mode)
- [ ] **Expected**: Mode toggles correctly each time
- [ ] **Expected**: Status bar updates
- [ ] **Expected**: No crashes

---

## Platform-Specific Tests

### macOS Only

- [ ] Test with macOS native clipboard (pbcopy/pbpaste)
- [ ] Test in Terminal.app
- [ ] Test in iTerm2
- [ ] Test with macOS Monterey or later
- [ ] **Expected**: Clipboard works in all terminal apps

### Linux Only

- [ ] Test with X11 (xclip)
- [ ] Test with Wayland (wl-clipboard)
- [ ] Test in gnome-terminal
- [ ] Test in konsole
- [ ] Test in alacritty
- [ ] **Expected**: Clipboard works in all terminal apps

---

## Security Tests (Manual Verification)

### ANSI Injection

- [ ] Create test history entry with ANSI codes:

  ```bash
  echo '{"display":"Malicious \x1b[31mRed\x1b[0m text","timestamp":1234567890,"sessionId":"550e8400-e29b-41d4-a716-446655440000"}' >> ~/.claude/history.jsonl
  ```

- [ ] Launch TUI: `cargo run`
- [ ] **Expected**: ANSI codes stripped (no red text)
- [ ] **Expected**: Plain text displayed: "Malicious Red text"

### Terminal Control Sequences

- [ ] Create entry with cursor movement codes:

  ```bash
  echo '{"display":"Test \x1b[2J\x1b[H cleared","timestamp":1234567890,"sessionId":"550e8400-e29b-41d4-a716-446655440001"}' >> ~/.claude/history.jsonl
  ```

- [ ] Launch TUI: `cargo run`
- [ ] **Expected**: Control codes stripped
- [ ] **Expected**: Terminal not cleared
- [ ] **Expected**: Text displays as "Test cleared"

---

## Test Results Summary

| Test Category        | macOS | Linux | Notes                         |
| -------------------- | ----- | ----- | ----------------------------- |
| Terminal Restoration | ☐     | ☐     |                               |
| Clipboard Basic      | ☐     | ☐     |                               |
| Clipboard Edge Cases | ☐     | ☐     |                               |
| Terminal Size Min    | ☐     | ☐     |                               |
| Terminal Size Small  | ☐     | ☐     | Graceful degradation expected |
| Terminal Size Large  | ☐     | ☐     |                               |
| Dynamic Resizing     | ☐     | ☐     |                               |
| Rapid Input          | ☐     | ☐     |                               |
| Filter Toggle Spam   | ☐     | ☐     |                               |
| ANSI Injection       | ☐     | ☐     | CRITICAL: Must strip codes    |
| Terminal Control Seq | ☐     | ☐     | CRITICAL: Must sanitize       |

---

## Testing Notes

### Date Tested

**Tester**:
**Platform**: (e.g., macOS 14.2, Ubuntu 22.04)
**Terminal**: (e.g., iTerm2 3.4.20, gnome-terminal 3.44)

### Issues Found

<!-- Document any issues here -->

### Pass/Fail

<!-- Overall assessment -->
