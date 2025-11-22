# Filter Integration Plan

## Overview

Integration of Work Stream 1 (TUI Core) and Work Stream 3 (Field Filters) for Phase 2 of ai-history-explorer.

**Status:** In progress
**Date:** 2025-01-22

## Architecture Decision

**Pattern:** Single input field with pipe `|` separator (inspired by fzf/telescope.nvim)

**Input format:**
```
project:ai-history type:user | fuzzy search terms
^^^^^^^^^^^^^^^^^^^^^^^^^      ^^^^^^^^^^^^^^^^^^^
  Filter portion (left)        Fuzzy portion (right)
```

**Behavior:**
- Left of `|`: Structured filters (`project:name`, `type:user|agent`, `since:YYYY-MM-DD`)
- Right of `|`: Fuzzy search on filtered results
- No `|` present: Treat entire input as fuzzy search (backward compatible)

## Data Flow

```
build_index() → Vec<SearchEntry> (all_entries)
                        ↓
              run_interactive(all_entries)
                        ↓
              ┌─────────┴─────────┐
              ↓                   ↓
        User types input    Current state
              ↓                   ↓
        Split on `|`         all_entries
         ↙        ↘              ↓
    Filters      Fuzzy      On Enter:
      ↓                      parse filters
 parse_filter()                  ↓
      ↓                    apply_filters()
 FilterExpr                      ↓
                          filtered_entries
                                 ↓
                        Re-inject to nucleo
                                 ↓
                           Fuzzy match
                           (right of |)
                                 ↓
                          Display results
```

## Implementation Details

### Input Parsing

Split user input on first `|` character:

```rust
let parts: Vec<&str> = input.splitn(2, '|').collect();
let filter_part = parts.get(0).map(|s| s.trim());
let fuzzy_part = parts.get(1).map(|s| s.trim()).unwrap_or(input);
```

**Examples:**
- `"project:foo | search"` → filter=`"project:foo"`, fuzzy=`"search"`
- `"project:foo"` → filter=`None`, fuzzy=`"project:foo"`
- `"| search"` → filter=`""` (empty), fuzzy=`"search"`

### Filter Application Trigger

**Trigger:** Enter key pressed (150ms debounce)

**Rationale:**
- Allows composing complex filters without intermediate parse errors
- Debounce prevents duplicate processing on rapid Enter presses
- Research shows 100-200ms debounce is standard for TUI tools

**Alternative considered:** Real-time on keystroke (rejected - too aggressive for filter parsing)

### App State Extensions

**Added to `App` struct:**

```rust
pub struct App {
    // Existing fields...

    // Filter integration fields
    all_entries: Vec<SearchEntry>,        // Original full dataset
    filtered_entries: Vec<SearchEntry>,   // After filter application
    current_filter: Option<FilterExpr>,   // Successfully parsed filter
    filter_error: Option<String>,         // Parse error message
}
```

**Initialization:**
- `all_entries`: Passed from `run_interactive(entries)`
- `filtered_entries`: Initially clone of `all_entries`
- `current_filter`: None
- `filter_error`: None

### Filter Lifecycle

**On Enter key:**

1. Extract filter portion (left of `|`)
2. If empty/unchanged: skip filter parsing
3. Parse with `filters::parse_filter(filter_part)`
4. **On parse error:**
   - Set `filter_error = Some(error_msg)`
   - Keep previous `filtered_entries`
   - Display error in status bar with syntax help
5. **On parse success:**
   - Clear `filter_error`
   - Apply `filters::apply_filters(all_entries, parsed_filter)`
   - Store result in `filtered_entries`
   - Store `current_filter = Some(parsed_filter)`
   - Clear nucleo injector
   - Re-inject `filtered_entries` to nucleo
6. Continue fuzzy matching on right of `|`

### Status Bar Display

**Format:**

```
[FUZZY] {matched}/{filtered} ({total}) | {active_filters} | {keybindings}
```

**Examples:**

```
[FUZZY] 12/156 (1234 total) | project:ai-history type:user | Esc:clear
```

```
[ERROR] Invalid field: foo | Try: project:name type:user | search
```

```
[FUZZY] 1234/1234 | Esc:clear Ctrl-C:quit
```

**Error display** (red text):
- Show parse error message
- Suggest valid syntax: `Try: project:name type:user | search`
- Keep results from last valid filter (don't clear on error)

### Keybindings Changes

**Modified:**
- **Esc:** Clear filter portion (remove left of `|`) - NO LONGER EXITS
- **Ctrl+C:** Quit program (new primary exit)

**Unchanged:**
- Enter: Apply filters (now with debounce)
- Ctrl+p/n, ↑/↓: Navigate results
- Tab: Toggle preview focus

**Rationale for Esc change:**
- Consistent with vim/shell behavior (Esc = cancel/clear)
- Prevents accidental exits while exploring filters
- Ctrl+C is standard terminal quit signal

## File Changes

### Modified Files

1. **`src/lib.rs`**
   - Add `pub mod filters;`

2. **`src/tui/app.rs`**
   - Extend `App` struct with filter state
   - Add input parsing logic (split on `|`)
   - Implement filter application in event handler
   - Add 150ms debounce for Enter key

3. **`src/tui/rendering.rs`**
   - Update status bar to show:
     - Match counts: `{matched}/{filtered} ({total})`
     - Active filters: parsed filter display
     - Errors: parse errors with help text

4. **`src/tui/events.rs`**
   - Change Esc: clear filter instead of quit
   - Add Ctrl+C: quit program

5. **`Cargo.toml`**
   - Add `dirs = "6.0"` (required by filters module)

### New Files

1. **`src/filters/mod.rs`** (from search-implementation)
   - Public exports for filter module

2. **`src/filters/ast.rs`** (from search-implementation)
   - FilterField, FilterOperator, FieldFilter, FilterExpr

3. **`src/filters/parser.rs`** (from search-implementation)
   - Tokenizer and parser for filter syntax

4. **`src/filters/apply.rs`** (from search-implementation)
   - `apply_filters(entries, filter) -> Result<Vec<SearchEntry>>`

5. **`tests/filter_integration_test.rs`** (new)
   - Integration tests for filter + TUI

6. **`docs/filter-integration-plan.md`** (this file)
   - Integration plan and decisions

## Testing Strategy

### Unit Tests (existing in filters module)

- ✓ Tokenizer edge cases
- ✓ Parser valid/invalid syntax
- ✓ Filter application (project, type, since)
- ✓ AND/OR operator logic

### Integration Tests (new)

**`tests/filter_integration_test.rs`:**

1. **Parse filter from input:**
   - Input: `"project:foo | search"`
   - Verify: filter parsed, fuzzy applied

2. **Apply filter reduces results:**
   - Input: `"type:user |"`
   - Verify: only user entries in filtered set

3. **Invalid filter shows error:**
   - Input: `"invalid:field |"`
   - Verify: error stored, help text shown

4. **Clear filter restores dataset:**
   - Sequence: apply filter → Esc → verify full dataset

5. **No pipe separator (backward compat):**
   - Input: `"search terms"`
   - Verify: no filter applied, fuzzy only

6. **Empty filter portion:**
   - Input: `"| search"`
   - Verify: no filter applied

### Manual Testing

**Test with real data:**
1. Run `cargo run -- interactive` with ~/.claude data
2. Test filters: `project:ai-history |`
3. Test fuzzy: `| tui implementation`
4. Test combined: `type:user | refactor`
5. Test error: `invalid:foo |`
6. Test clear: press Esc
7. Test quit: press Ctrl+C

**Performance check:**
- Load time with 10k+ entries
- Filter application latency
- Fuzzy search responsiveness

## Future Optimizations

### Phase 3+ Enhancements

1. **Filter Caching**
   - Cache parsed `FilterExpr` to avoid re-parsing identical filters
   - Use hash of filter string as cache key
   - Invalidate on filter change

2. **Real-time Filter Application**
   - Apply filters on keystroke (with debounce)
   - Requires more aggressive error handling (don't show error mid-typing)
   - Use 150-200ms debounce for parse + apply

3. **Advanced Filter Syntax**
   - Parentheses: `project:foo AND (type:user OR type:agent)`
   - Negation: `NOT project:foo`
   - Regex: `project:/ai-.*-explorer/`
   - Date ranges: `since:2024-01-01 until:2024-12-31`

4. **Performance Optimizations**
   - Parallel filter application (rayon) for large datasets
   - Streaming filter results to nucleo
   - Pre-filter index at startup (common filters)

5. **UX Improvements**
   - Filter history (↑/↓ in filter mode)
   - Filter suggestions/autocomplete
   - Save/load filter presets
   - Syntax highlighting in input field

## Design Rationale

### Why Single Input with `|` Separator?

**Research findings:**
- fzf, skim, telescope.nvim all use single input field
- Pipe `|` separator is intuitive (shell pipeline mental model)
- Avoids field-switching complexity (Tab navigation)
- Fast power-user workflow (no mode switching)

**Alternative considered:** Separate filter and fuzzy inputs
- Rejected: Requires field focus management, more complex UI
- Less efficient for rapid iteration

### Why Enter to Apply (Not Real-time)?

**Pros:**
- Allows composing complex filters without errors
- User controls when filter is applied
- Reduces parse overhead (once per Enter vs every keystroke)

**Cons:**
- Less responsive than real-time (but debounce helps)

**Decision:** Start with Enter, add real-time in Phase 3 if requested

### Why Change Esc Behavior?

**Rationale:**
- Vim/Emacs users expect Esc = clear/cancel, not quit
- Prevents accidental exits during exploration
- Ctrl+C is universal terminal quit

**Alternative:** Keep Esc as quit, use Ctrl+X for clear
- Rejected: Less discoverable, breaks muscle memory

## Success Metrics

**Functional:**
- ✅ Filter syntax parses correctly
- ✅ Filters apply before fuzzy matching
- ✅ Status bar shows filter state and errors
- ✅ Esc clears filter, Ctrl+C quits
- ✅ Backward compatible (no `|` = fuzzy only)

**Quality:**
- ✅ All tests pass (cargo test)
- ✅ Coverage ≥90% (cargo llvm-cov)
- ✅ Zero clippy warnings

**Performance:**
- ✅ Filter application <100ms (10k entries)
- ✅ Fuzzy search latency <50ms
- ✅ No memory leaks

**UX:**
- ✅ Helpful error messages with syntax examples
- ✅ Intuitive keybindings (Esc = clear, Ctrl+C = quit)
- ✅ Clear visual feedback (status bar)

## Open Questions

**Resolved:**

1. ✅ Debounce timing: 150ms on Enter
2. ✅ Esc behavior: Clear filter (Ctrl+C exits)
3. ✅ Error display: Status bar with syntax help
4. ✅ Filter caching: Defer to Phase 3 (documented above)

**Still open:**

1. Should `|` character be escaped if user wants literal pipe in fuzzy search?
   - **Current:** No escaping (rare edge case)
   - **Future:** Add `\|` escape sequence if requested

2. Should status bar show filter syntax help by default (before first filter)?
   - **Current:** Only show on error
   - **Future:** Add `?` key to toggle help overlay

## References

- [Phase 2 Implementation Plan](../plans/phase2-tui/implementation.md)
- [TUI PR #5](https://github.com/user/ai-history-explorer/pull/5)
- [Filter Module (search-implementation branch)](../.conductor/algiers/src/filters/)
- [fzf Extended Search Mode](https://github.com/junegunn/fzf#search-syntax)
- [telescope.nvim Filtering](https://github.com/nvim-telescope/telescope.nvim)

---

**Last updated:** 2025-01-22
**Status:** In progress
**Next steps:** Phase 1 - Merge filter module
