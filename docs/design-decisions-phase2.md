# Design Decisions: Phase 2 (TUI)

**Date**: 2025-11-23
**Status**: Phase 2 In Progress
**Summary**: Interactive TUI implementation with fuzzy search, clipboard integration, and field filters.

---

## Keybinding Decisions

### 1. Clipboard Copy: Ctrl+Y (Not Enter)

**Decision**: Use `Ctrl+Y` for clipboard copy instead of `Enter` key

**Rationale**:
- `Enter` key already assigned to filter application (primary action in search workflow)
- Avoids keybinding conflict and user confusion
- `Ctrl+Y` is mnemonic: "Y" for "yank" (vim terminology for copy)
- Consistent with vim/emacs power-user expectations

**Alternative Considered**: `Enter` for copy, `/` or `Ctrl+F` for filter
- **Rejected**: Filter application is more frequent operation than clipboard copy
- Enter key should trigger the most common action (applying filters)

**Implementation**: `src/tui/events.rs:50`

**Documentation**: Updated in `README.md` keybindings section

**Timeline**: Originally planned as `Enter` in Phase 2 implementation plan, changed during TUI integration work (PR #5-#7)

---

### 2. Quit: Ctrl+C (Not Esc)

**Decision**: Use `Ctrl+C` as primary quit keybinding, `Esc` clears search/filter

**Rationale**:
- `Esc` = cancel/clear (vim/shell convention)
- Prevents accidental exits while exploring filters
- `Ctrl+C` is universal terminal interrupt signal
- Better UX for iterative search refinement

**Behavior**:
- `Esc` when search is active: Clear search query (reset to empty)
- `Esc` when search is empty: Quit application (backward compatible)
- `Ctrl+C`: Always quit immediately

**Implementation**: `src/tui/events.rs:38`, `src/tui/app.rs:152-160`

**Alternative Considered**: Keep `Esc` as quit, use `Ctrl+X` for clear
- **Rejected**: Less discoverable, breaks muscle memory from other TUI tools

---

## Status Message Design

### 3. Transient Status Messages with Auto-Expiry

**Decision**: Implement time-based auto-expiring status messages

**Design**:
```rust
pub struct StatusMessage {
    pub text: String,
    pub message_type: MessageType,  // Success | Error
    pub expires_at: Instant,
}
```

**Rationale**:
- Provides immediate feedback without blocking UI
- Auto-cleanup prevents stale messages
- Non-intrusive (doesn't require dismissal action)
- Industry standard (GitHub: 3s, VS Code: 3-5s)

**Duration Choices**:
- Success messages: 3000ms (3 seconds)
  - Quick positive feedback
  - Doesn't linger unnecessarily
- Error messages: 5000ms (5 seconds)
  - More time to read error details
  - Important messages warrant longer display

**Implementation**: `src/tui/app.rs:17-30`, constants at lines 18-20

**Expiry Mechanism**:
- Uses `Instant::now()` (monotonic clock) for reliability
- Checked every frame (~100ms polling) with negligible overhead
- Graceful handling when no message present

**Priority**: Status messages take precedence over filter errors in status bar rendering (`src/tui/rendering.rs:178-191`)

---

## Filter Integration Architecture

### 4. Single Input Field with Pipe Separator

**Decision**: Use `|` character to separate filter and fuzzy search portions

**Format**: `project:name type:user | fuzzy search terms`

**Rationale**:
- Inspired by fzf/telescope.nvim (proven UX patterns)
- Single field = simpler UI, no focus management
- Pipe character intuitive (shell pipeline mental model)
- Fast power-user workflow (no mode switching)

**Parsing**:
- Left of `|`: Structured filters (project, type, since)
- Right of `|`: Fuzzy search on filtered results
- No `|` present: Entire input is fuzzy search (backward compatible)

**Implementation**: `src/tui/app.rs:260-272` (parse_input method)

**Alternative Considered**: Separate filter and fuzzy input fields
- **Rejected**: More complex UI, requires field navigation, slower workflow

---

### 5. Filter Application Trigger: Enter Key with Debounce

**Decision**: Apply filters on `Enter` key press with 150ms debounce

**Rationale**:
- Allows composing complex filters without intermediate parse errors
- User controls when filter is applied (explicit action)
- Reduces parse overhead (once per Enter vs every keystroke)
- Debounce prevents duplicate processing on rapid presses

**Debounce Timing**: 150ms
- Research shows 100-200ms is standard for TUI tools
- Balances responsiveness vs preventing accidental double-triggers

**Implementation**: `src/tui/app.rs:167-179`

**Alternative Considered**: Real-time filter application on keystroke
- **Rejected for Phase 2**: Too aggressive, shows errors mid-typing
- **Deferred to Phase 3**: Can add with debounce if requested

---

## Display Format Decisions

### 6. Status Bar Message Priority

**Decision**: Hierarchy for status bar content display

**Priority Order** (highest to lowest):
1. Transient status messages (clipboard success/error)
2. Filter parse errors (with syntax help)
3. Empty state message (no entries)
4. Normal state (counts, filters, keybindings)

**Rationale**:
- Immediate feedback (clipboard ops) most urgent
- Parse errors need attention before continuing
- Empty state prevents user confusion
- Normal state is informational baseline

**Implementation**: `src/tui/rendering.rs:178-232` (cascading if-else)

---

### 7. Status Message Duration Constants

**Decision**: Extract duration values as named constants

**Implementation**:
```rust
const STATUS_SUCCESS_DURATION_MS: u64 = 3000;  // 3 seconds
const STATUS_ERROR_DURATION_MS: u64 = 5000;    // 5 seconds
```

**Rationale**:
- Self-documenting code (intent clear from name)
- Easy to adjust globally if UX testing suggests changes
- Avoids "magic numbers" code smell

**Location**: `src/tui/app.rs:18-20`

**Decided**: 2025-11-23 (post-PR #7 review)

---

## Future Enhancements (Deferred to Phase 3+)

### Considered but Deferred:

1. **Real-time filter application**
   - Apply filters on keystroke with debounce
   - Requires more sophisticated error handling
   - Planned for Phase 3 if user feedback requests it

2. **Advanced filter syntax**
   - Parentheses: `project:foo AND (type:user OR type:agent)`
   - Negation: `NOT project:foo`
   - Regex: `project:/ai-.*-explorer/`
   - Documented in `plans/phase2-tui/implementation.md` lines 457-475

3. **Filter history and autocomplete**
   - ↑/↓ navigation through previous filters
   - Autocomplete for field names and values
   - Save/load filter presets

4. **Enhanced status messages**
   - Additional message types: Info, Warning
   - Action hints ("Copied! Press Ctrl+V to paste")
   - Configurable durations via settings

---

## Design Rationale Summary

**Core Philosophy**: Prioritize power-user efficiency while maintaining discoverability

**Key Principles**:
1. **Vim/Emacs compatibility**: Keybindings familiar to terminal power users
2. **Non-blocking feedback**: Status messages don't interrupt workflow
3. **Explicit actions**: Important operations (filter apply) triggered intentionally
4. **Progressive disclosure**: Simple use cases work without knowing advanced features
5. **Industry standards**: Follow proven UX patterns (fzf, GitHub, VS Code)

---

## References

- [Phase 2 Implementation Plan](../plans/phase2-tui/implementation.md)
- [PR #5: TUI Core + Fuzzy Search](https://github.com/syz51/ai-history-explorer/pull/5)
- [PR #6: Filter Integration](https://github.com/syz51/ai-history-explorer/pull/6)
- [PR #7: Clipboard Status Messages](https://github.com/syz51/ai-history-explorer/pull/7)
- [fzf Extended Search Mode](https://github.com/junegunn/fzf#search-syntax)
- [GitHub Toast Notifications UX](https://primer.style/design/components/toast)

---

**Last Updated**: 2025-11-23
**Status**: Active (Phase 2 ongoing)
**Next Review**: After Phase 2 completion
