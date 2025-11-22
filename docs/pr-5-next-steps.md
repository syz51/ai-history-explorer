# PR #5 Next Steps: Deferred Items

**Generated from:** docs/pr-5-review.md
**Date:** 2025-11-22

## Overview

Items identified during PR #5 review that are deferred to future work. Organized by priority and phase.

---

## Phase 2 - Work Stream 2/3 Integration

### Worker B (Clipboard) Requirements

**Status message system** (lines 381-419)

- **Need:** Display "✓ Copied to clipboard" feedback
- **Implementation:**

  ```rust
  pub struct App {
      status_message: Option<StatusMessage>,
  }

  struct StatusMessage {
      text: String,
      expires_at: Instant,
      style: MessageStyle,  // Success, Error, Info
  }
  ```

- **Priority:** Required for Worker B
- **Location:** src/tui/app.rs

### Worker C (Filters) Requirements

**Filter state management** (lines 395-401)

- **Need:** Add filter input and active flag to App state
- **Implementation:**

  ```rust
  pub struct App {
      filter_input: String,
      filter_input_active: bool,
  }
  ```

- **Priority:** Required for Worker C
- **Recommendation:** Implement in separate filters/ module

---

## UX Improvements (Phase 3)

### Keybinding Enhancements

**Tab focus toggle** (lines 59, 434, 537, 816)

- **Current:** TODO stub at app.rs:86-89
- **Need:** Switch focus between results list and preview pane
- **Enables:** Independent preview scrolling with Page Up/Down
- **Priority:** Medium

**Ctrl+r refresh** (lines 60, 435, 538, 818)

- **Current:** TODO stub at app.rs:89-92
- **Need:** Rebuild search index without restart
- **Use case:** Pick up new conversations without exiting TUI
- **Priority:** Low

**Esc key behavior** (lines 145-154, 762)

- **Status:** ✅ Implemented in app.rs:73-81 (clears search if active, quits if empty)
- **Discussion needed:** Evaluate if current exit handling is optimal
  - Consider: Should we require explicit quit command (q/Ctrl+C only)?
  - Consider: Multiple Esc presses to quit pattern?
  - Consider: User confusion from dual Esc behavior?
- **Priority:** Medium - revisit based on user feedback

**Ctrl+Q unconditional quit** (lines 174-176)

- **Rationale:** Escape hatch if nucleo matcher hangs
- **Current:** Only Ctrl+C works
- **Priority:** Low

**Page Up/Down preview scrolling** (lines 440-451, 800)

- **Current:** Moves selection ±10 items
- **Plan specified:** Scroll preview pane
- **Note:** Current behavior more useful for keyboard navigation
- **Can add:** When preview pane gets focus (Tab to switch)
- **Priority:** Low

### Visual Improvements

**Display text truncation** (lines 158-172)

- **Issue:** Hardcoded 50 char limit in rendering.rs:48-56
- **Impact:** Wasted space on wide terminals, overflow on narrow
- **Fix:** Calculate based on terminal width
- **Priority:** Low

---

## Performance Optimizations (Future Phase)

### Memory Efficiency

**Triple clone per entry** (lines 333-360, 624, 759)

- **Location:** app.rs:32-34 during nucleo injection
- **Issue:** `entry.clone()` + `display_text.clone()` × 2
- **Impact:** ~30MB extra for 10K entries (acceptable for Phase 2)
- **Better approach:**

  ```rust
  for entry in &entries {
      let display_text = entry.display_text.clone();
      injector.push(entry.clone(), move |_entry, cols| {
          cols[0] = display_text.as_str().into();  // No clone
      });
  }
  ```

- **Priority:** Low (defer until >50K entries common)

### Rendering Efficiency

**Re-render on every tick** (lines 625-641, 760)

- **Issue:** terminal.draw() called even when state unchanged
- **Impact:** Minimal (terminals render ~60 FPS anyway)
- **Optimization:**

  ```rust
  if self.dirty {
      terminal.draw(...)?;
      self.dirty = false;
  }
  ```

- **Priority:** Low

### Search Performance

**Streaming architecture** (lines 286-308, 539)

- **Current:** Batch load all entries in App::new()
- **Alternative:** Stream entries progressively
- **Rationale for deferral:** Simpler for Phase 2, no threading complexity
- **Revisit when:** 100K+ entries become common
- **Priority:** Low

**Multi-threading nucleo** (lines 310-331, 540)

- **Current:** Single thread (app.rs:27, 4th param = 1)
- **May be slow:** For >50K entries
- **Easy to fix:**

  ```rust
  let num_threads = std::thread::available_parallelism()
      .map(|n| n.get())
      .unwrap_or(1);
  ```

- **Priority:** Low (parameterize during performance tuning)

---

## Testing Improvements

### Test Coverage

**Terminal manager low coverage** (lines 192-229, 754-755)

- **Current:** 22.86% line coverage (8/35 lines)
- **Root cause:** TTY-dependent code hard to unit test
- **Uncovered:** Error paths and Drop implementation
- **Action required:** Document manual test results
- **Manual tests:**
  - [ ] Terminal restores after Ctrl+C
  - [ ] Terminal not corrupted after kill -9
  - [ ] Test on macOS and Linux
- **Acceptable:** 22% coverage for TTY code if manually verified
- **Priority:** Document before merge

### Missing Test Cases

**Search edge cases** (lines 255-258)

- Empty string search behavior
- Search with only whitespace
- Search with special regex chars (if nucleo uses regex)

**Rendering edge cases** (lines 260-263)

- Terminal too small (e.g., 10x3)
- Unicode in project paths
- Project path longer than terminal width

**Event handling combinations** (lines 265-267)

- Rapid Ctrl+p/n presses
- Search update while selected_idx > 0

**Verdict:** Acceptable for Phase 2, add in integration testing phase

---

## Documentation

**Module-level docs** (lines 590-605, 761)

- **Missing:** Module doc comments in all src/tui/\*.rs files
- **Example:**

  ```rust
  //! TUI application state and event loop.
  //!
  //! Manages nucleo fuzzy matcher, selection state, and main render loop.
  ```

- **Status:** Not critical for Phase 2, good practice
- **Priority:** Add in documentation sprint

---

## Security

**ANSI code stripping audit** (lines 667-696)

- **Issue:** Display text from SearchEntry rendered directly to terminal
- **Risk:** Malicious ANSI escape codes could manipulate terminal
- **Current mitigation:** strip_ansi_codes() on thinking blocks only
- **Need:** Ensure all display_text construction strips ANSI codes
- **Verdict:** Pre-existing concern, not introduced by PR #5
- **Action:** Separate security review of indexer/builder.rs
- **Priority:** Medium

---

## Summary by Phase

### Phase 2 (Required for Workers B/C)

- Status message system (Worker B)
- Filter state management (Worker C)
- Terminal manager manual testing (before merge)

### Phase 3 (UX Improvements)

- Tab focus toggle
- Esc key behavior (clear search first)
- Display text truncation based on width
- Page Up/Down preview scrolling

### Future Performance Phase

- Triple clone optimization
- Re-render dirty flag
- Streaming architecture (if 100K+ entries)
- Multi-threading nucleo (if >50K entries)

### Future Testing Phase

- Search edge case tests
- Rendering edge case tests
- Event handling combination tests

### Future Documentation Phase

- Module-level doc comments

### Future Security Phase

- ANSI code stripping audit
