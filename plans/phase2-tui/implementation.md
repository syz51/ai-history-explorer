# Phase 2: TUI Implementation Plan

## Spec

Phase 2 delivers an interactive terminal user interface (TUI) for ai-history-explorer, transforming the CLI from a stats-only tool into a fully functional fuzzy-finding search interface. Users can:

- Launch interactive mode with `ai-history-explorer interactive`
- Search through conversation history with real-time fuzzy matching
- Preview full entry context (timestamp, project, truncated content) in split-pane layout
- Filter results by project, type, or date before fuzzy matching
- Copy selected entries to system clipboard with single keypress
- Navigate with Vim-style keybindings (Ctrl+p/n)

The interface uses nucleo-picker for blazing-fast fuzzy search (6-8x faster than alternatives), ratatui for TUI rendering, and arboard for cross-platform clipboard support. Target platforms: macOS (primary), Linux (secondary). Windows support deferred to Phase 3+.

## Plan

**Architecture approach:** Feature-based parallel development with 3 independent work streams converging on shared CLI integration point.

**Work Stream 1 (TUI Core):** Research nucleo-picker streaming API to determine if we can redesign index building from batch (`Vec<SearchEntry>`) to streaming architecture. Implement split-pane TUI with results list (left) and preview pane (right) showing truncated content from existing indexer. Preview displays `SearchEntry.display_text` as-is (already truncated: 1KB thinking, 4KB tool content). Add tiered timestamps, color scheme, status bar, keybindings.

**Work Stream 2 (Clipboard):** Add arboard dependency for cross-platform clipboard access. Implement copy-on-Enter behavior, handle clipboard errors gracefully (macOS focus, Linux secondary), provide user feedback via status messages.

**Work Stream 3 (Filters):** Build filter syntax parser supporting `field:value` patterns and AND/OR operators. Supported fields: `project:path`, `type:user|agent`, `since:YYYY-MM-DD`. Filter logic: same-field OR (e.g., `project:foo project:bar`), cross-field AND (e.g., `project:foo type:agent`). Apply filters before fuzzy matching to reduce search space. Defer parentheses/complex expressions to Phase 3.

**Integration:** All streams converge on new `interactive` subcommand in CLI. Final integration testing ensures fuzzy search + clipboard + filters work together seamlessly.

**Key decisions:**

- Preview: Show truncated content (no file re-reading)
- Filters: AND/OR in Phase 2, parentheses in Phase 3
- Keybindings: Ctrl+p/n (Vim/Emacs style)
- Streaming: Research nucleo-picker API (may change architecture)

## Tasks

### Work Stream 1: TUI & Fuzzy Search (Worker A)

**Prerequisites:**

- [ ] **Research nucleo-picker streaming API**
  - [ ] Check if nucleo supports incremental entry addition (streaming)
  - [ ] Test if display updates progressively as entries arrive
  - [ ] Verify timestamp-based sorting works in streaming mode
  - [ ] **Decision point:** If streaming supported, redesign indexer to stream instead of building Vec. If not, proceed with current Vec approach.
  - **Acceptance:** Document findings in `docs/nucleo-streaming-research.md` with recommendation

**Dependencies & Setup:**

- [ ] Add `nucleo-picker` to Cargo.toml (fuzzy finder)
- [ ] Add `ratatui` to Cargo.toml (TUI framework)
- [ ] Add `crossterm` to Cargo.toml (terminal backend for ratatui)

**Core Fuzzy Search Integration:**

- [ ] Create `src/tui/` module structure
- [ ] Implement `nucleo` integration wrapper
  - [ ] Configure nucleo with SearchEntry items
  - [ ] Set up fuzzy matching on display_text field
  - [ ] Handle user input streaming to nucleo
- [ ] Create basic event loop (keyboard input → nucleo → render)

**TUI Layout & Rendering:**

- [ ] Design split-pane layout with ratatui
  - [ ] Left pane: Results list (60% width)
  - [ ] Right pane: Preview (40% width)
  - [ ] Status bar at bottom
- [ ] Implement results list rendering
  - [ ] Show entry type icon (👤 user, 🤖 agent)
  - [ ] Show tiered timestamps:
    - [ ] Relative for <7 days: "2h ago", "3d ago"
    - [ ] Absolute for ≥7 days: "Jan 15", "Dec 3, 2024"
  - [ ] Show project path (~ substitution)
  - [ ] Highlight selected entry
- [ ] Implement preview pane rendering
  - [ ] Display `SearchEntry.display_text` (already truncated)
  - [ ] Show metadata header (timestamp, project, session ID)
  - [ ] Scrollable content (if truncated text > pane height)
  - [ ] Wrap long lines

**Visual Design:**

- [ ] Implement color scheme
  - [ ] Dark zinc background (#18181b)
  - [ ] Emerald accents for highlights (#10b981)
  - [ ] Muted text for metadata (#71717a)
  - [ ] Bright text for selected entry (#fafafa)
- [ ] Style status bar
  - [ ] Left: Filter indicator (if active)
  - [ ] Center: Entry counts ("Showing 42 / 1,234 entries")
  - [ ] Right: Keybinding hints ("Enter: copy | /: filter | Esc: quit")

**Keybindings:**

- [ ] Implement navigation
  - [ ] Ctrl+p / ↑ - Previous entry
  - [ ] Ctrl+n / ↓ - Next entry
  - [ ] Page Up/Down - Scroll preview
- [ ] Implement actions (stubs for now, Worker B implements copy)
  - [ ] Enter - Copy to clipboard (stub → Worker B fills in)
  - [ ] / - Focus filter input (stub → Worker C fills in)
- [ ] Implement control
  - [ ] Tab - Toggle focus (results ↔ preview)
  - [ ] Ctrl+c / Esc - Quit
  - [ ] Ctrl+r - Refresh index

**CLI Integration:**

- [ ] Add `interactive` subcommand to `cli/commands.rs`
- [ ] Wire up to TUI entry point
- [ ] Handle graceful shutdown (restore terminal state)

**Testing:**

- [ ] Unit tests for timestamp formatting (relative/absolute)
- [ ] Unit tests for layout calculations
- [ ] Integration test: launch TUI with test data, verify rendering
- [ ] Manual testing: real ~/.claude data, verify performance

**Acceptance Criteria:**

- Launches interactive mode with fuzzy search
- Results update in real-time as user types
- Preview shows selected entry details
- Navigation works smoothly (Ctrl+p/n, arrows)
- Proper terminal cleanup on exit
- No crashes with large datasets (10K+ entries)

**Dependencies on other streams:** None (clipboard/filter are stubs initially)

---

### Work Stream 2: Clipboard Integration (Worker B)

**Dependencies & Setup:**

- [ ] Add `arboard` to Cargo.toml (clipboard library)
- [ ] Review arboard docs for macOS/Linux clipboard APIs

**Core Clipboard Implementation:**

- [ ] Create `src/clipboard/` module
- [ ] Implement `copy_to_clipboard(text: &str) -> Result<()>`
  - [ ] Use arboard to set clipboard text
  - [ ] Handle platform-specific errors
  - [ ] Add error context (anyhow)
- [ ] Implement clipboard feedback in TUI
  - [ ] Success: Show "✓ Copied to clipboard" in status bar (3s timeout)
  - [ ] Failure: Show "✗ Clipboard error: \<reason\>" in status bar
  - [ ] Clear message after timeout or next action

**TUI Integration:**

- [ ] Hook Enter key handler to copy function
- [ ] Pass `SearchEntry.display_text` to clipboard
- [ ] Update status bar with feedback message
- [ ] Handle edge cases:
  - [ ] Empty display_text
  - [ ] Very large content (>10MB)
  - [ ] Clipboard unavailable (headless, permissions)

**Testing:**

- [ ] Unit tests for clipboard module (mock arboard in tests)
- [ ] Integration test: verify clipboard contains expected text after copy
- [ ] Manual testing: copy on macOS, verify in other apps (e.g., paste into editor)
- [ ] Error case testing: clipboard locked, permission denied

**Platform Support:**

- [ ] Primary: macOS (test with pbpaste)
- [ ] Secondary: Linux (test with xclip/wl-paste if available)
- [ ] Document Windows limitations (deferred to Phase 3)

**Acceptance Criteria:**

- Enter key copies selected entry to system clipboard
- Status bar shows success/failure feedback
- Copied text is paste-able in external apps
- Graceful error handling (no crashes on clipboard errors)
- Works on macOS and Linux

**Dependencies on other streams:** Requires Worker A's TUI event loop and status bar

---

### Work Stream 3: Field Filters (Worker C)

**Filter Syntax Specification:**

```text
Supported fields:
  project:<path>    - Filter by project path (supports ~ and partial matches)
  type:<user|agent> - Filter by entry type
  since:<date>      - Filter entries after date (YYYY-MM-DD format)

Operators (Phase 2):
  AND - Both conditions must match (default between different fields)
  OR  - Either condition matches (default within same field)

Examples:
  project:ai-history         → entries from project matching "ai-history"
  type:user                  → only user prompts
  since:2024-01-15           → entries after Jan 15, 2024
  project:foo type:agent     → implicitly AND (project=foo AND type=agent)
  project:foo project:bar    → implicitly OR (project=foo OR project=bar)
  project:foo AND type:user  → explicitly AND
  type:user OR type:agent    → explicitly OR (matches all entries)

Deferred to Phase 3:
  Parentheses: project:foo AND (type:user OR type:agent)
  Negation: NOT project:foo
  Regex: project:/foo.*bar/
```

**Parser Implementation:**

- [ ] Create `src/filters/` module
- [ ] Define filter AST structs
  - [ ] `enum FilterField { Project, Type, Since }`
  - [ ] `enum FilterOperator { And, Or }`
  - [ ] `struct FieldFilter { field, value }`
  - [ ] `struct FilterExpr` (supports AND/OR without parentheses)
- [ ] Implement tokenizer
  - [ ] Split input into tokens: `field:value`, `AND`, `OR`, whitespace
  - [ ] Handle quoted values: `project:"foo bar"`
- [ ] Implement parser (precedence: field filters → OR → AND)
  - [ ] Parse field:value pairs
  - [ ] Parse AND/OR operators
  - [ ] Return FilterExpr tree
  - [ ] Error on invalid syntax (unknown fields, malformed dates)

**Filter Application:**

- [ ] Implement `apply_filters(entries: Vec<SearchEntry>, filter: FilterExpr) -> Vec<SearchEntry>`
  - [ ] Evaluate FilterExpr against each entry
  - [ ] Project filter: case-insensitive substring match
  - [ ] Type filter: exact match on EntryType enum
  - [ ] Since filter: timestamp >= parsed date
  - [ ] AND/OR logic: combine filters correctly
- [ ] Apply filters BEFORE fuzzy matching (reduce search space)

**TUI Integration:**

- [ ] Add filter input box (toggles with `/` key)
- [ ] Show current filter in status bar
- [ ] Update results in real-time as filter changes
- [ ] Show filtered count: "Showing 42 / 1,234 (filtered from 5,678)"
- [ ] Clear filter with Esc (when filter input focused)

**Error Handling:**

- [ ] Show syntax errors in status bar (red)
- [ ] Highlight invalid tokens in filter input
- [ ] Provide helpful error messages:
  - [ ] "Unknown field: foo" (suggest valid fields)
  - [ ] "Invalid date format: foo" (show expected format)
  - [ ] "Unexpected token: (" (parentheses not supported yet)

**Testing:**

- [ ] Unit tests for tokenizer (edge cases: quotes, whitespace, special chars)
- [ ] Unit tests for parser (valid/invalid syntax)
- [ ] Unit tests for filter application (each field type, AND/OR logic)
- [ ] Integration test: filter input → parsing → application → display
- [ ] Property tests (if time): random filter combos don't crash

**Documentation:**

- [ ] Document filter syntax in help text (interactive mode)
- [ ] Add examples to README
- [ ] Document Phase 3 features (parentheses, negation, regex)

**Acceptance Criteria:**

- Filter syntax parses correctly (field:value, AND/OR)
- Filters apply before fuzzy matching
- Filter input box works (/ to open, Esc to clear)
- Status bar shows filter status and counts
- Syntax errors display helpful messages
- Tests cover all filter fields and operators

**Dependencies on other streams:** Requires Worker A's TUI input handling and status bar

---

### Shared: Integration & Testing (All Workers)

**CLI Integration:**

- [ ] Add `Commands::Interactive` variant to `cli/commands.rs`
- [ ] Wire up to `tui::run_interactive(index: Vec<SearchEntry>)`
- [ ] Handle --help text for interactive mode

**End-to-End Testing:**

- [ ] Integration test: launch interactive, type query, verify results
- [ ] Integration test: apply filter, verify filtered results
- [ ] Integration test: copy entry, verify clipboard
- [ ] Integration test: all features together (filter + fuzzy + copy)
- [ ] Performance test: 10K entries, verify <1s load time
- [ ] Memory test: 100K entries, verify stable memory usage

**Documentation:**

- [ ] Update README with interactive mode usage
- [ ] Document keybindings (table format)
- [ ] Document filter syntax (examples)
- [ ] Add GIF/demo of interactive mode (optional)

**Code Quality:**

- [ ] Run pre-commit hooks (fmt, clippy, test, coverage)
- [ ] Enforce 90%+ coverage target
- [ ] Zero clippy warnings
- [ ] Update CLAUDE.md if architecture changes

**Acceptance Criteria:**

- All 3 work streams integrate cleanly
- `cargo test` passes (including new integration tests)
- Coverage ≥90%
- No clippy warnings
- Manual testing with real ~/.claude data successful

---

## Context

### Existing Files (Phase 1 - do not modify unless necessary)

**Core modules:**

- `src/main.rs` - Entry point, calls cli::run()
- `src/lib.rs` - Library exports (build_index, SearchEntry, parsers, utils)
- `src/cli/commands.rs` - CLI args (currently only Stats subcommand)
- `src/cli/mod.rs` - CLI module exports
- `src/models/search.rs` - SearchEntry struct (display\_text, timestamp, project\_path, entry\_type, session\_id)
- `src/models/history.rs` - HistoryEntry, ConversationEntry, ContentBlock, Message
- `src/models/project.rs` - ProjectInfo (encoded\_name, decoded\_path, project\_dir, agent\_files)
- `src/indexer/builder.rs` - build_index() function (parses history + agent files → Vec\<SearchEntry\>)
- `src/indexer/project_discovery.rs` - discover_projects() (scans ~/.claude/projects/)
- `src/parsers/history.rs` - parse_history_file() (history.jsonl → Vec\<HistoryEntry\>)
- `src/parsers/conversation.rs` - parse_conversation_file() (agent-\*.jsonl → Vec\<ConversationEntry\>)
- `src/utils/paths.rs` - encode/decode paths, safe_open_file/dir (security validation)
- `src/utils/environment.rs` - get_claude_dir() (resolves ~/.claude)
- `src/utils/terminal.rs` - strip_ansi_codes()

**Test infrastructure:**

- `tests/common/mod.rs` - Test utilities (ClaudeDirBuilder, HistoryEntryBuilder, etc.)
- `tests/cli_test.rs` - CLI command tests
- `tests/integration_test.rs` - E2E pipeline tests
- `tests/security_test.rs` - Path traversal, symlink, file size tests
- `tests/edge_cases_test.rs` - UTF-8, JSON nesting, large files
- `tests/memory_test.rs` - Large dataset tests

**Configuration:**

- `Cargo.toml` - Dependencies, edition 2024
- `.prek/hooks.toml` - Pre-commit hooks (fmt, clippy, test, coverage)

### New Files (Phase 2 - to be created)

**TUI modules:**

- `src/tui/mod.rs` - TUI entry point and module exports
- `src/tui/app.rs` - Main TUI application state and event loop
- `src/tui/layout.rs` - Split-pane layout calculations
- `src/tui/rendering.rs` - Ratatui rendering logic (results, preview, status bar)
- `src/tui/events.rs` - Keyboard event handling
- `src/tui/timestamps.rs` - Tiered timestamp formatting (relative/absolute)

**Clipboard module:**

- `src/clipboard/mod.rs` - Clipboard operations (arboard wrapper)

**Filter modules:**

- `src/filters/mod.rs` - Filter module exports
- `src/filters/parser.rs` - Filter syntax tokenizer and parser
- `src/filters/ast.rs` - FilterExpr AST structs
- `src/filters/apply.rs` - Filter application logic (filter Vec\<SearchEntry\>)

**Tests:**

- `tests/tui_test.rs` - TUI integration tests
- `tests/clipboard_test.rs` - Clipboard tests
- `tests/filter_test.rs` - Filter parsing and application tests

**Documentation:**

- `docs/nucleo-streaming-research.md` - Research findings on nucleo-picker API
- `docs/keybindings.md` - Interactive mode keybindings reference

### Dependencies (to be added to Cargo.toml)

**Work Stream 1:**

```toml
nucleo-picker = "0.3"  # Fuzzy finder (6-8x faster than skim)
ratatui = "0.29"       # TUI framework
crossterm = "0.28"     # Terminal backend
```

**Work Stream 2:**

```toml
arboard = "3.4"        # Cross-platform clipboard
```

**Work Stream 3:**
No new dependencies (use existing std, regex if needed)

### Key Data Structures

**SearchEntry (existing - do not modify):**

```rust
pub struct SearchEntry {
    pub entry_type: EntryType,       // UserPrompt or AgentMessage
    pub display_text: String,        // Already truncated (1KB thinking, 4KB tool)
    pub timestamp: DateTime<Utc>,
    pub project_path: Option<String>,
    pub session_id: Option<String>,
}
```

**FilterExpr (new - to be created):**

```rust
pub enum FilterField {
    Project,
    Type,
    Since,
}

pub enum FilterOperator {
    And,
    Or,
}

pub struct FieldFilter {
    pub field: FilterField,
    pub value: String,
}

pub struct FilterExpr {
    pub filters: Vec<FieldFilter>,
    pub operators: Vec<FilterOperator>,
}
```

---

## Deferred to Phase 3+

**Platform Support:**

- Windows support (dirs crate, path handling, symlinks, clipboard)

**Performance:**

- Parallel agent file parsing (rayon)
- Streaming architecture (if nucleo supports)
- Formal benchmarking (criterion crate)

**Advanced Filters:**

- Parentheses for complex expressions: `project:foo AND (type:user OR type:agent)`
- Negation: `NOT project:foo`
- Regex: `project:/foo.*bar/`
- Date ranges: `since:2024-01-01 until:2024-12-31`

**Advanced Features:**

- Multi-select (copy multiple entries at once)
- Export to file (JSON, CSV)
- Full conversation history (not just prompts/agent messages)
- Session threading (link related entries via parentUuid)

---

## Success Metrics

**Functional:**

- ✅ Interactive mode launches and displays search results
- ✅ Fuzzy search updates in real-time
- ✅ Clipboard copy works on Enter
- ✅ Filters apply correctly (project, type, since)
- ✅ AND/OR operators work as specified
- ✅ Keybindings work (Ctrl+p/n, Enter, /, Esc, Tab)

**Quality:**

- ✅ All tests pass (cargo test)
- ✅ Coverage ≥90% (cargo llvm-cov)
- ✅ Zero clippy warnings (cargo clippy)
- ✅ Pre-commit hooks pass (prek run --all-files)

**Performance:**

- ✅ 10K entries load in <1s
- ✅ Fuzzy search latency <50ms
- ✅ Stable memory usage (no leaks)

**UX:**

- ✅ Terminal state restored on exit (no corrupted terminal)
- ✅ Helpful error messages (filter syntax, clipboard errors)
- ✅ Intuitive keybindings (documented in help)

---

## Open Questions & Research

### 1. Nucleo-picker streaming API (Critical - blocks Work Stream 1 design)

**Question:** Does nucleo-picker support incremental entry addition?

**Research needed:**

- Read nucleo-picker docs and examples
- Test incremental add API (if exists)
- Benchmark streaming vs batch performance
- Verify timestamp sorting in streaming mode

**Decision impact:**

- **If yes:** Redesign indexer to stream entries (major architecture change, better memory profile)
- **If no:** Keep current Vec\<SearchEntry\> approach (simpler, no changes to indexer)

**Owner:** Worker A (TUI stream)

---

### 2. Filter precedence edge cases (Low priority - can refine in Phase 3)

**Question:** How to handle ambiguous filter combos like `project:foo type:user OR type:agent`?

**Current plan:**

- Same-field OR: `project:foo project:bar` → (foo OR bar)
- Cross-field AND: `project:foo type:user` → (foo AND user)
- Explicit operators override: `type:user OR type:agent` → (user OR agent)

**Edge case:** `project:foo type:user OR type:agent`

- Interpretation A: `(project:foo AND type:user) OR type:agent`
- Interpretation B: `project:foo AND (type:user OR type:agent)`

**Decision:** Document behavior in Phase 2, refine in Phase 3 with parentheses support

**Owner:** Worker C (Filters stream)

---

### 3. Timestamp formatting localization (Optional - defer to post-Phase 2)

**Question:** Should relative timestamps ("2h ago") respect locale/timezone?

**Current plan:**

- UTC timestamps from data
- Relative: "2h ago", "3d ago" (always English)
- Absolute: "Jan 15" (month abbreviations in English)

**Future:** i18n support (German: "vor 2 Std", Spanish: "hace 2h")

**Decision:** English-only for Phase 2, i18n deferred

---

## Coordination Notes

**Branch strategy:**

- Worker A: `feat/tui-fuzzy-search`
- Worker B: `feat/clipboard`
- Worker C: `feat/filters`
- Integration: `feat/phase2-integration` (merges all 3)

**Communication:**

- Worker A creates TUI stubs for clipboard/filter integration
- Workers B & C implement against those stubs
- Final integration PR merges all streams

**Merge order:**

1. Worker A (TUI core) → main
2. Worker B (clipboard) → main (builds on TUI)
3. Worker C (filters) → main (builds on TUI)
4. Integration PR (final testing)

**Conflicts:**

- All workers touch `Cargo.toml` (dependencies)
- Workers B & C both modify TUI event handlers
- Resolve conflicts during integration phase
