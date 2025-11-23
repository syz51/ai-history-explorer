# AI History Explorer - Project Plan

**Status**: Phase 2 In Progress
**Last Updated**: 2025-11-23

---

## Table of Contents

1. [Project Overview](#project-overview)
2. [Phase 1: Core Infrastructure](#phase-1-core-infrastructure-complete)
3. [Phase 2: TUI & Advanced Features](#phase-2-tui--advanced-features-in-progress)
4. [Phase 3: Future Enhancements](#phase-3-future-enhancements)
5. [Architecture Reference](#architecture-reference)

---

## Project Overview

### Spec

A terminal-based tool named `ai-history-explorer` that allows users to search through their Claude Code conversation history using fuzzy finding. Users can browse all past prompts and agent conversations, preview the context (project, timestamp, session), and copy selected prompts to the system clipboard for re-use in new Claude Code sessions. Part of a growing suite of AI development tools.

The tool reads from `~/.claude/history.jsonl` for user prompts and `~/.claude/projects/*/agent-*.jsonl` for agent sub-conversations, providing a fast fuzzy search interface. Selected text is automatically copied to the system clipboard, enabling quick iteration on previous prompts.

Initial version focuses on searching user prompts and agent conversations, with full conversation history (including Claude responses) planned for future enhancement.

### Data Model Source (IMPORTANT)

The data model is **reverse-engineered from local files**, not from official documentation. Claude Code's official docs do not publish specifications for the local storage format. This is the only available approach for building history browsing tools, as the Agent SDK is designed for building agents, not accessing historical conversations. The format may change in future Claude Code versions without notice.

**Claude Code Data Storage:**

- `~/.claude/history.jsonl` - Global user prompt history
  - Format: One JSON object per line
  - Fields: `display`, `timestamp`, `project`, `sessionId`, `pastedContents`

- `~/.claude/projects/<encoded-project-path>/` - Per-project conversation storage
  - Path encoding: `/Users/foo/bar` → `-Users-foo-bar`
  - `<sessionId>.jsonl` - Main conversation thread
  - `agent-<agentId>.jsonl` - Agent/subagent conversations

- `~/.claude/settings.json` - Global settings
- `~/.claude/debug/*.txt` - Debug logs per session

**Message Structure in JSONL files:**

```json
{
  "type": "user" | "assistant",
  "message": {
    "role": "user" | "assistant",
    "content": [{"type": "text", "text": "..."}]
  },
  "timestamp": "ISO 8601",
  "sessionId": "uuid",
  "uuid": "uuid",
  "parentUuid": "uuid",
  "isSidechain": boolean
}
```

**Data flow:**

1. Parse history.jsonl → extract user prompts
2. Discover projects in ~/.claude/projects/
3. Parse agent conversation files → extract user messages (type="user")
4. Combine all entries → sort by timestamp (newest first)
5. Return unified SearchEntry index

---

## Phase 1: Core Infrastructure (✅ COMPLETE)

**Status**: Production-ready for macOS (2025-01-21, updated 2025-11-21)

### Completion Stats

- **253 tests** (247 active + 6 ignored, 100% pass rate)
- **97.61% code coverage**
- **Zero clippy warnings**
- **21/22 issues fixed** (1 deferred to Phase 2: Windows support)
- **Production-ready** for macOS

### Deliverables

- ✅ Core JSONL parsing (history.jsonl + agent-*.jsonl)
- ✅ Unified searchable index
- ✅ Path encoding/decoding with percent encoding
- ✅ Project discovery in ~/.claude/projects/
- ✅ CLI with --stats, --help, --version
- ✅ Graceful degradation with error tracking
- ✅ Security hardening (6 protections)
- ✅ Pre-commit hooks with coverage enforcement
- ✅ Comprehensive test suite (201 tests)
- ✅ Content block integration tests

### Security Decisions

#### 1. Symlink Validation

**Decision**: Reject symbolic links in project directories and agent files

**Implementation**: `validate_path_not_symlink()` in `src/utils/paths.rs`

**Rationale**: Prevents symlink attacks where malicious symlinks in `~/.claude/projects/` could point to sensitive locations (`/etc/passwd`, private keys, etc.)

**Applied To**:
- Project directories in `~/.claude/projects/`
- Agent conversation files (`agent-*.jsonl`)

**Tests**: `tests/security_test.rs::test_security_symlink_*` (Unix only)

**Platform Note**: Unix-specific (`#[cfg(unix)]`) - Windows uses different symlink APIs, planned for Phase 2

#### 2. JSON Depth Limiting

**Decision**: Rely on serde_json's built-in recursion limit

**Implementation**: Documented in `src/parsers/mod.rs` module-level docs

**Details**:
- serde_json enforces 128-level maximum nesting depth
- Prevents "Billion Laughs" style stack overflow attacks
- Reasonable nesting (<20 levels) accepted without issue

**Tests**: `src/parsers/history.rs::test_parse_moderately_nested_json_accepted`

#### 3. Resource Limits

**Decision**: Enforce hard limits on directory listing operations

**Implementation**: Constants in `src/indexer/project_discovery.rs`
- `MAX_PROJECTS = 1000`
- `MAX_AGENT_FILES_PER_PROJECT = 1000`

**Rationale**: Prevents resource exhaustion DoS attacks via directory listing floods

**Trade-off**: Legitimate users with >1000 projects will see warnings, but this is extremely unlikely

#### 4. File Size Limits

**Decision**: 10MB maximum file size for JSONL files

**Implementation**: `MAX_FILE_SIZE_BYTES` constant + `validate_file_size()` in `src/utils/paths.rs`

**Rationale**:
- Current history.jsonl is 595KB
- 10MB = 40x current size (ample headroom for growth)
- Prevents memory exhaustion DoS from maliciously large files

**TOCTOU Protection**: File size validated AFTER opening file handle (not before) to prevent race conditions

**Applied To**:
- `history.jsonl` parsing (`src/parsers/history.rs`)
- Agent conversation file parsing (`src/parsers/conversation.rs`)

#### 5. Path Traversal Validation

**Decision**: Validate all decoded paths to prevent traversal attacks

**Implementation**: Multi-layer validation in `src/utils/paths.rs`

**Protections**:
1. **Percent encoding**: Use `percent-encoding` crate (not simple `-` replacement)
   - Prevents collision: `/foo/bar` vs `/foo-bar` map to different encoded names
2. **Traversal check**: Reject paths with `..` components
3. **Absolute path requirement**: All paths must be absolute
4. **Defense-in-depth**: Validate project paths from `history.jsonl` (even though trusted)

**Example Attack Prevented**:
```text
Input: "-Users-foo-bar-..-etc-passwd"
Decode: /Users/foo/bar/../etc/passwd
Result: REJECTED (contains ..)
```

#### 6. Terminal Output Sanitization

**Decision**: Provide ANSI escape sequence stripping utility

**Implementation**: `strip_ansi_codes()` in `src/utils/terminal.rs`

**Rationale**: Removes ANSI escape sequences and control characters that could manipulate terminal output

**Current Usage**: Not used in `stats` command, but available for Phase 2 TUI features

### Error Handling Strategy

**Philosophy**: Graceful degradation for CLI tools

**Decision**: Skip malformed entries with warnings, but fail if >50% corrupt

**Rationale**:
- **For libraries** (thiserror): Always propagate errors, let caller decide
- **For CLI tools** (anyhow): Graceful degradation acceptable with user feedback
- Partial data corruption shouldn't break entire tool
- Fundamentally broken data (>50% failure) should fail loudly

**Implementation Details**:

**File-Level Errors**: Log warnings, continue with empty results
- Missing files → warning + empty Vec
- Permission denied → warning + empty Vec

**Parse Errors**: Skip line, track error rate
- Malformed JSON → skip line, increment `skipped_count`
- Invalid timestamp → skip entry
- Missing required fields → skip entry

**Failure Thresholds**:
1. **>50% parse failures**: Fail with error (prevents accepting fundamentally broken data)
2. **>100 consecutive errors**: Bail early (prevents DoS from unlimited malformed lines)

**User Feedback**: Summary statistics on stderr

```text
Parsed history.jsonl: 2375 entries (12 skipped)
Indexed 2387 entries total
```

**Code Locations**:
- `src/parsers/history.rs:45-56` - Error rate tracking
- `src/parsers/conversation.rs:45-56` - Error rate tracking
- `src/indexer/builder.rs:100-120` - Summary statistics

### Architecture Decisions

#### Module Structure

```text
src/
├── cli/commands.rs       # CLI interface (stats command)
├── indexer/
│   ├── builder.rs        # Aggregates history + agent messages
│   └── project_discovery.rs  # Scans ~/.claude/projects/
├── models/
│   ├── history.rs        # HistoryEntry, MessageContent
│   ├── conversation.rs   # ConversationEntry
│   ├── search.rs         # SearchEntry, EntryType
│   └── project.rs        # ProjectInfo
├── parsers/
│   ├── history.rs        # history.jsonl parser
│   ├── conversation.rs   # agent-*.jsonl parser
│   ├── deserializers.rs  # Custom timestamp/sessionId handling
│   └── mod.rs            # Module docs
└── utils/
    ├── environment.rs    # Home directory detection
    ├── paths.rs          # Encoding/decoding/validation
    └── terminal.rs       # ANSI sanitization
```

**Rationale**:
- **parsers/**: Data layer - JSONL → Rust structs
- **indexer/**: Business logic - discovery + aggregation
- **models/**: Pure data structures
- **utils/**: Reusable helpers
- **cli/**: User interface

#### Entry Types

**Decision**: Filter to user prompts only in Phase 1

**Implementation**: `EntryType` enum in `src/models/search.rs`

```rust
pub enum EntryType {
    UserPrompt,    // User messages from history.jsonl and agent files
    AgentMessage,  // Currently filtered out
}
```

**Rationale**:
- Phase 1 scope: Search user prompts for reuse
- Agent responses deferred to Phase 2 (full conversation history)
- Simplifies indexing logic

**Sources**:
- `history.jsonl` entries (all are user prompts)
- `agent-*.jsonl` entries with `type == "user"`

#### Path Encoding

**Decision**: Use percent encoding (URL encoding)

**Implementation**: `percent-encoding` crate v2.3 in `src/utils/paths.rs`

**Examples**:
```text
/Users/foo/bar → -Users%2Ffoo%2Fbar
/Users/foo-bar → -Users%2Ffoo-bar
```

**Rationale**:
- **Bijection guarantee**: Different paths → different encoded names
- **No collisions**: `/foo/bar` vs `/foo-bar` are distinct
- **Standard encoding**: Well-tested library (used in URL encoding)

**Previous Approach** (rejected): Simple `-` replacement
- Collision: `/foo/bar` and `/foo-bar` both → `-foo-bar`
- Security risk: ambiguous decoding

#### Platform Support

**Decision**: macOS-only for Phase 1

**Implementation**: Uses `$HOME` environment variable (`src/utils/environment.rs`)

**Windows Limitation**: `$HOME` doesn't exist on Windows (uses `%USERPROFILE%`)

**Phase 2 Plan**:
- Use `dirs` or `home` crate for cross-platform home detection
- Add Windows-specific tests
- Handle different path separators

### Testing Strategy

#### Coverage Targets

**Decision**: Enforce 90%+ line coverage

**Implementation**: Pre-commit hook with `cargo-llvm-cov`

**Achieved**: 97.03% total coverage, 98.48% line coverage

**Pre-commit Hook** (`.prek.toml`):
```bash
cargo llvm-cov --all-features --workspace --fail-under-lines 90
```

#### Test Suite Structure

```text
src/                      # 125 unit tests
└── */tests.rs

tests/                    # 47 integration tests
├── common/mod.rs         # Shared utilities
├── integration_test.rs   # 11 E2E workflows
├── cli_test.rs          # 9 CLI binary tests
├── security_test.rs     # 11 security boundaries
└── edge_cases_test.rs   # 16 edge cases
```

**Doctests**: 6 examples in rustdoc comments

**Total**: 178 tests, 100% passing

**Commands**:
```bash
cargo test --lib                    # Unit tests only (fast)
cargo test --test integration_test  # E2E workflows
cargo test --test security_test     # Security boundaries
```

#### Test Utilities

**Implementation**:
- `ClaudeDirBuilder`: Create test `.claude/` structures
- `HistoryEntryBuilder`: Build history.jsonl entries
- `AgentFileBuilder`: Build agent conversation files
- `ConversationEntryBuilder`: Build conversation entries

**Helpers**:
- `minimal_claude_dir()`: Empty `.claude/` directory
- `realistic_claude_dir()`: Sample data (3 history + 2 projects)

**Example Usage**:
```rust
let temp_dir = ClaudeDirBuilder::new()
    .with_history_entries(vec![...])
    .with_project("project-name", vec![...])
    .build()?;
```

#### Edge Case Coverage

**Categories**:
1. **Format variations**: Empty lines, mixed line endings (LF/CRLF), no trailing newline
2. **Unicode handling**: Emoji, CJK characters, RTL text
3. **Scale**: Very long text (100KB), many entries (1000)
4. **Duplicates**: Same timestamp, same session ID
5. **Filesystem**: Special chars in paths, non-UTF8 filenames (Unix)
6. **Corruption**: Truncated JSON at EOF, empty fields
7. **File types**: Non-agent JSONL files (ignored), nested subdirs

### Performance Decisions

#### String Allocation Optimizations

**Decision**: Pre-allocate capacity for string concatenation

**Implementation**: `src/indexer/builder.rs:63-77`

**Before** (inefficient):
```rust
let text_parts: Vec<&str> = entry.message.content.iter()
    .filter(|c| c.content_type == "text")
    .filter_map(|c| c.text.as_deref())
    .collect();
let display_text = text_parts.join(" ");  // Allocates intermediate Vec
```

**After** (optimized):
```rust
// Calculate total capacity needed
let total_capacity: usize = entry.message.content.iter()
    .filter(|c| c.content_type == "text")
    .filter_map(|c| c.text.as_deref())
    .map(|t| t.len())
    .sum::<usize>() + newline_count;

let mut display_text = String::with_capacity(total_capacity);
// Build string directly (no intermediate Vec)
```

**Impact**: Avoids reallocation during string growth, eliminates intermediate Vec

#### Cow Optimization for Path Conversions

**Decision**: Use `Cow` (Clone-on-Write) to avoid double allocation

**Implementation**: `src/utils/paths.rs:151-154`

**Pattern**:
```rust
match path.to_string_lossy() {
    Cow::Borrowed(s) => s.to_string(),  // UTF-8 path: one allocation
    Cow::Owned(s) => s,                 // Non-UTF-8: already allocated
}
```

**Rationale**:
- `to_string_lossy()` returns `Cow<str>`
- If path is valid UTF-8: `Cow::Borrowed` (no allocation yet)
- Direct `.to_string()` would allocate twice
- Match on Cow avoids second allocation

#### Lazy Error Context

**Decision**: Use `with_context(|| ...)` instead of `context(...)`

**Before** (eager allocation):
```rust
.context(format!("Failed to open file: {}", path.display()))
```

**After** (lazy allocation):
```rust
.with_context(|| format!("Failed to open file: {}", path.display()))
```

**Impact**: Format string only allocated when error actually occurs (not on success path)

### Issues Resolved

**Total**: 22 issues identified, 21 fixed, 1 deferred

#### Critical Issues (1)

**#1: User Messages Mislabeled as Agent Messages** ✅ FIXED
- **Location**: `src/indexer/builder.rs:47-59`
- **Issue**: Filtered for `type == "user"` but set `EntryType::AgentMessage`
- **Fix**: Changed to `EntryType::UserPrompt`

#### High Severity Issues (4)

**#2: Path Traversal Vulnerability** ✅ FIXED
- Added `validate_decoded_path()` rejecting `..` components

**#3: Path Encoding Collision** ✅ FIXED
- Use percent-encoding crate for bijection guarantee

**#4: Windows Platform Incompatibility** 🔵 DEFERRED
- **Deferral**: Phase 1 targets macOS; Phase 2+ will add cross-platform support
- **Plan**: Use `dirs` or `home` crate

**#5: Unbounded Memory Allocation** ✅ FIXED
- 10MB limit with TOCTOU-safe validation

#### Medium Severity Issues (6)

- #6: Inefficient String Allocation ✅ FIXED
- #7: Silent Error Swallowing ✅ FIXED
- #8: Timestamp Overflow ✅ FIXED
- #9: No Session ID Validation ✅ FIXED
- #10: Malformed Line DoS ✅ FIXED
- #11: Double Allocation ✅ FIXED

#### Low Severity Issues (9)

- #12-#19: All fixed (derive traits, docs, flatten errors, unused fields, etc.)

#### Post-Review Issues (3)

- #20-#22: All fixed (duplicate count, path traversal in history.jsonl, TOCTOU race)

### Phase 1 Retrospective

**What Went Well**:
- ✅ All 17 planned tasks completed
- ✅ Added significant security hardening beyond original scope
- ✅ Comprehensive test suite (178 tests, 97%+ coverage)
- ✅ Code review identified and fixed 22 issues (21 fixed, 1 deferred)
- ✅ Production-ready for macOS

**Key Outcomes**:
- **Path encoding**: Switched from simple `-` replacement to percent encoding (fixes collision bug)
- **Security**: 6 protection layers (symlink, JSON depth, resource limits, file size, path traversal, terminal sanitization)
- **Error handling**: Graceful degradation with >50% failure threshold and consecutive error limits
- **Testing**: Test utilities in `tests/common/mod.rs` enable easy fixture creation
- **Platform**: macOS-only Phase 1; Windows support planned for Phase 2

**Issues Deferred to Phase 2**:
- Windows platform support (#4 - requires dirs/home crate)
- Parallel parsing (performance enhancement)
- Streaming architecture (memory optimization)
- Fuzzing and benchmarking (advanced testing)

---

## Phase 2: TUI & Advanced Features (🔄 IN PROGRESS)

**Status**: Active development
**Architecture**: Feature-based parallel development with 3 independent work streams

### Overview

Phase 2 delivers an interactive terminal user interface (TUI) for ai-history-explorer, transforming the CLI from a stats-only tool into a fully functional fuzzy-finding search interface. Users can:

- Launch interactive mode with `ai-history-explorer interactive`
- Search through conversation history with real-time fuzzy matching
- Preview full entry context (timestamp, project, truncated content) in split-pane layout
- Filter results by project, type, or date before fuzzy matching
- Copy selected entries to system clipboard with single keypress
- Navigate with Vim-style keybindings (Ctrl+p/n)

**Target platforms**: macOS (primary), Linux (secondary). Windows support deferred to Phase 3+.

### Work Streams

**Work Stream 1 (TUI Core)**: Research nucleo-picker streaming API (see [nucleo-streaming-research.md](nucleo-streaming-research.md)). Implement split-pane TUI with results list (left) and preview pane (right). Add tiered timestamps, color scheme, status bar, keybindings.

**Work Stream 2 (Clipboard)**: Add arboard dependency for cross-platform clipboard access. Implement copy-on-Enter behavior, handle clipboard errors gracefully, provide user feedback via status messages.

**Work Stream 3 (Filters)**: Build filter syntax parser supporting `field:value` patterns and AND/OR operators. Supported fields: `project:path`, `type:user|agent`, `since:YYYY-MM-DD`. Apply filters before fuzzy matching to reduce search space.

**Integration**: All streams converge on new `interactive` subcommand in CLI.

### Design Decisions

#### 1. Clipboard Copy: Ctrl+Y (Not Enter)

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

**Timeline**: Originally planned as `Enter` in Phase 2 implementation plan, changed during TUI integration work (PR #5-#7)

#### 2. Quit: Ctrl+C (Not Esc)

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

#### 3. Transient Status Messages with Auto-Expiry

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

#### 4. Status Message Duration Constants

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

#### 5. Single Input Field with Pipe Separator

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

#### 6. Filter Application Trigger: Enter Key with Debounce

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

#### 7. Status Bar Message Priority

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

### Work Stream Details

#### Work Stream 1: TUI & Fuzzy Search

**Prerequisites:**
- [x] Research nucleo-picker streaming API
  - **Finding**: Nucleo supports streaming via injector API
  - **Decision**: Use custom ratatui + nucleo (not nucleo-picker) for full layout control
  - **Documentation**: See [nucleo-streaming-research.md](nucleo-streaming-research.md)

**Dependencies & Setup:**
- [x] Add `nucleo-picker` to Cargo.toml (fuzzy finder)
- [x] Add `ratatui` to Cargo.toml (TUI framework)
- [x] Add `crossterm` to Cargo.toml (terminal backend for ratatui)

**Core Fuzzy Search Integration:**
- [x] Create `src/tui/` module structure
- [x] Implement `nucleo` integration wrapper
- [x] Create basic event loop (keyboard input → nucleo → render)

**TUI Layout & Rendering:**
- [x] Design split-pane layout with ratatui (60/40 split)
- [x] Implement results list rendering (type icons, timestamps, paths)
- [x] Implement preview pane rendering
- [x] Implement tiered timestamps (relative <7d, absolute ≥7d)

**Visual Design:**
- [x] Implement color scheme (zinc bg, emerald accents)
- [x] Style status bar (filter indicator, counts, keybindings)

**Keybindings:**
- [x] Navigation (Ctrl+p/n, arrows)
- [x] Actions (Enter for copy, / for filter)
- [x] Control (Ctrl+c/Esc quit, Tab toggle focus, Ctrl+r refresh)

**CLI Integration:**
- [x] Add `interactive` subcommand to `cli/commands.rs`
- [x] Handle graceful shutdown (restore terminal state)

**Testing:**
- [x] Unit tests for timestamp formatting
- [x] Unit tests for layout calculations
- [x] Integration test: launch TUI with test data
- [x] Manual testing with real ~/.claude data

**Acceptance Criteria**: ✅ Complete
- Launches interactive mode with fuzzy search
- Results update in real-time as user types
- Preview shows selected entry details
- Navigation works smoothly
- Proper terminal cleanup on exit
- No crashes with large datasets (10K+ entries)

#### Work Stream 2: Clipboard Integration

**Dependencies & Setup:**
- [x] Add `arboard` to Cargo.toml (clipboard library)
- [x] Review arboard docs for macOS/Linux clipboard APIs

**Core Clipboard Implementation:**
- [x] Create `src/clipboard/` module
- [x] Implement `copy_to_clipboard(text: &str) -> Result<()>`
- [x] Implement clipboard feedback in TUI (success/error messages)

**TUI Integration:**
- [x] Hook Enter key handler to copy function
- [x] Pass `SearchEntry.display_text` to clipboard
- [x] Update status bar with feedback message
- [x] Handle edge cases (empty text, large content, clipboard unavailable)

**Testing:**
- [x] Unit tests for clipboard module
- [x] Integration test: verify clipboard contains expected text
- [x] Manual testing: copy on macOS, verify in other apps
- [x] Error case testing: clipboard locked, permission denied

**Platform Support:**
- [x] Primary: macOS (test with pbpaste)
- [x] Secondary: Linux (test with xclip/wl-paste if available)
- [ ] Document Windows limitations (deferred to Phase 3)

**Acceptance Criteria**: ✅ Complete
- Enter key copies selected entry to system clipboard
- Status bar shows success/failure feedback
- Copied text is paste-able in external apps
- Graceful error handling (no crashes on clipboard errors)
- Works on macOS and Linux

#### Work Stream 3: Field Filters

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
- [x] Create `src/filters/` module
- [x] Define filter AST structs (FilterField, FilterOperator, FieldFilter, FilterExpr)
- [x] Implement tokenizer (field:value, AND, OR, quoted values)
- [x] Implement parser (precedence: field filters → OR → AND)

**Filter Application:**
- [x] Implement `apply_filters(entries, filter) -> Vec<SearchEntry>`
- [x] Apply filters BEFORE fuzzy matching (reduce search space)

**TUI Integration:**
- [x] Add filter input box (toggles with `/` key)
- [x] Show current filter in status bar
- [x] Update results in real-time as filter changes
- [x] Show filtered count
- [x] Clear filter with Esc (when filter input focused)

**Error Handling:**
- [x] Show syntax errors in status bar (red)
- [x] Highlight invalid tokens in filter input
- [x] Provide helpful error messages (unknown field, invalid date, unexpected token)

**Testing:**
- [x] Unit tests for tokenizer (edge cases)
- [x] Unit tests for parser (valid/invalid syntax)
- [x] Unit tests for filter application (each field type, AND/OR logic)
- [x] Integration test: filter input → parsing → application → display

**Documentation:**
- [x] Document filter syntax in help text (interactive mode)
- [x] Add examples to README
- [x] Document Phase 3 features (parentheses, negation, regex)

**Acceptance Criteria**: ✅ Complete
- Filter syntax parses correctly (field:value, AND/OR)
- Filters apply before fuzzy matching
- Filter input box works (/ to open, Esc to clear)
- Status bar shows filter status and counts
- Syntax errors display helpful messages
- Tests cover all filter fields and operators

### Deferred Items from PR Reviews

#### Phase 2 Work Stream Integration Requirements

**Status message system** (Worker B requirement)
- **Status**: ✅ Implemented
- **Location**: src/tui/app.rs
- **Implementation**: StatusMessage struct with expiry mechanism

**Filter state management** (Worker C requirement)
- **Status**: ✅ Implemented
- **Location**: src/tui/app.rs
- **Implementation**: filter_input and filter_input_active fields

#### UX Improvements (Deferred to Phase 3)

**Tab focus toggle**
- **Current**: TODO stub at app.rs:86-89
- **Need**: Switch focus between results list and preview pane
- **Enables**: Independent preview scrolling with Page Up/Down
- **Priority**: Medium

**Ctrl+r refresh**
- **Current**: TODO stub at app.rs:89-92
- **Need**: Rebuild search index without restart
- **Use case**: Pick up new conversations without exiting TUI
- **Priority**: Low

**Esc key behavior evaluation**
- **Status**: ✅ Implemented (clears search if active, quits if empty)
- **Action**: Evaluate based on user feedback
- **Priority**: Medium - revisit in Phase 3

**Ctrl+Q unconditional quit**
- **Rationale**: Escape hatch if nucleo matcher hangs
- **Current**: Only Ctrl+C works
- **Priority**: Low

**Page Up/Down preview scrolling**
- **Current**: Moves selection ±10 items
- **Plan specified**: Scroll preview pane
- **Note**: Current behavior useful for keyboard navigation
- **Can add**: When preview pane gets focus (Tab to switch)
- **Priority**: Low

**Display text truncation**
- **Issue**: Hardcoded 50 char limit in rendering.rs:48-56
- **Impact**: Wasted space on wide terminals, overflow on narrow
- **Fix**: Calculate based on terminal width
- **Priority**: Low

#### Performance Optimizations (Deferred to Future Phase)

**Triple clone per entry**
- **Location**: app.rs:32-34 during nucleo injection
- **Issue**: `entry.clone()` + `display_text.clone()` × 2
- **Impact**: ~30MB extra for 10K entries (acceptable for Phase 2)
- **Priority**: Low (defer until >50K entries common)

**Re-render on every tick**
- **Issue**: terminal.draw() called even when state unchanged
- **Impact**: Minimal (terminals render ~60 FPS anyway)
- **Priority**: Low

**Streaming architecture**
- **Current**: Batch load all entries in App::new()
- **Alternative**: Stream entries progressively
- **Rationale for deferral**: Simpler for Phase 2, no threading complexity
- **Revisit when**: 100K+ entries become common
- **Priority**: Low

**Multi-threading nucleo**
- **Current**: Single thread (app.rs:27, 4th param = 1)
- **May be slow**: For >50K entries
- **Priority**: Low (parameterize during performance tuning)

#### Testing Improvements

**Terminal manager low coverage**
- **Current**: 22.86% line coverage (8/35 lines)
- **Root cause**: TTY-dependent code hard to unit test
- **Action required**: Document manual test results
- **Manual tests**:
  - [ ] Terminal restores after Ctrl+C
  - [ ] Terminal not corrupted after kill -9
  - [ ] Test on macOS and Linux
- **Acceptable**: 22% coverage for TTY code if manually verified
- **Priority**: Document before merge

**Missing Test Cases** (Acceptable for Phase 2, add in integration testing):
- Empty string search behavior
- Search with only whitespace
- Search with special regex chars
- Terminal too small (e.g., 10x3)
- Unicode in project paths
- Project path longer than terminal width
- Rapid Ctrl+p/n presses
- Search update while selected_idx > 0

#### Documentation

**Module-level docs**
- **Missing**: Module doc comments in all src/tui/*.rs files
- **Status**: Not critical for Phase 2, good practice
- **Priority**: Add in documentation sprint

#### Security

**ANSI code stripping audit**
- **Issue**: Display text from SearchEntry rendered directly to terminal
- **Risk**: Malicious ANSI escape codes could manipulate terminal
- **Current mitigation**: strip_ansi_codes() on thinking blocks only
- **Need**: Ensure all display_text construction strips ANSI codes
- **Verdict**: Pre-existing concern, not introduced by PR #5
- **Action**: Separate security review of indexer/builder.rs
- **Priority**: Medium

### Design Philosophy

**Core Philosophy**: Prioritize power-user efficiency while maintaining discoverability

**Key Principles**:
1. **Vim/Emacs compatibility**: Keybindings familiar to terminal power users
2. **Non-blocking feedback**: Status messages don't interrupt workflow
3. **Explicit actions**: Important operations (filter apply) triggered intentionally
4. **Progressive disclosure**: Simple use cases work without knowing advanced features
5. **Industry standards**: Follow proven UX patterns (fzf, GitHub, VS Code)

### References

- [Nucleo Streaming Research](nucleo-streaming-research.md)
- [PR #5: TUI Core + Fuzzy Search](https://github.com/syz51/ai-history-explorer/pull/5)
- [PR #6: Filter Integration](https://github.com/syz51/ai-history-explorer/pull/6)
- [PR #7: Clipboard Status Messages](https://github.com/syz51/ai-history-explorer/pull/7)
- [fzf Extended Search Mode](https://github.com/junegunn/fzf#search-syntax)
- [GitHub Toast Notifications UX](https://primer.style/design/components/toast)

---

## Phase 3: Future Enhancements

### Platform Support

**Windows support** (deferred from Phase 1 #4)
- [ ] Cross-platform home directory detection (dirs or home crate)
- [ ] Windows path handling tests
- [ ] Different HOME env var behavior (USERPROFILE)
- [ ] Symlink tests for Windows (different API)

### Performance

**Performance baseline benchmarking** (prerequisite for optimizations)
- [ ] Install criterion crate for formal benchmarks
- [ ] Benchmark realistic datasets (10K, 100K, 1M entries)
- [ ] Document baseline metrics
- [ ] Establish thresholds for optimization ROI

**Parallel agent file parsing** (high impact)
- [ ] Use rayon for concurrent parsing
- [ ] Benchmark performance gain vs baseline

**Streaming instead of full memory load** (medium impact)
- [ ] Stream entries instead of collect() into Vec
- [ ] Lazy evaluation for large datasets

**Preview text lazy loading** (low impact)
- [ ] Only load preview when displayed

### Advanced Filters

**Complex expressions**
- [ ] Parentheses: `project:foo AND (type:user OR type:agent)`
- [ ] Negation: `NOT project:foo`
- [ ] Regex: `project:/foo.*bar/`
- [ ] Date ranges: `since:2024-01-01 until:2024-12-31`

### Advanced Features

**Real-time filter application**
- Apply filters on keystroke with debounce
- Requires more sophisticated error handling
- Planned if user feedback requests it

**Filter history and autocomplete**
- ↑/↓ navigation through previous filters
- Autocomplete for field names and values
- Save/load filter presets

**Enhanced status messages**
- Additional message types: Info, Warning
- Action hints ("Copied! Press Ctrl+V to paste")
- Configurable durations via settings

**Multi-select support**
- [ ] Select multiple entries (copy batch)
- [ ] Concatenate selected entries to clipboard

**Export functionality**
- [ ] Export to JSON
- [ ] Export to CSV

**Full conversation history**
- [ ] Include Claude responses (not just prompts/agent messages)
- [ ] Session threading (link related entries via parentUuid)

### Testing & Quality

**Advanced Testing**
- [ ] Fuzzing with cargo-fuzz (AFL/libfuzzer)
- [ ] Target: malformed JSONL, extreme nesting, encoding attacks

**Formal Performance Testing**
- [ ] Baseline metrics: 10K, 100K, 1M entries
- [ ] Sorting performance at scale
- [ ] Serial vs parallel file processing
- [ ] Memory usage profiling

**Windows Compatibility Notes**
- Current security/edge tests focus on Unix systems
- Windows support deferred to Phase 3+ will require:
  - Symlink tests: Windows CreateSymbolicLink API differs from Unix
  - Path handling: NULL byte behavior may vary
  - Concurrent file access: Different locking semantics
  - All tests marked `#[cfg(unix)]` need Windows equivalents

### Distribution

- [ ] Write comprehensive README with installation and usage instructions
- [ ] Package for cargo install (initial release)
- [ ] (Future) Add homebrew distribution
- [ ] (Future) Add apt distribution
- [ ] (Future) Add scoop distribution (Windows)

---

## Architecture Reference

### Module Structure

```text
src/
├── cli/
│   ├── commands.rs       # CLI interface (stats, interactive)
│   └── mod.rs
├── clipboard/
│   └── mod.rs            # Clipboard operations (arboard wrapper)
├── filters/
│   ├── parser.rs         # Filter syntax tokenizer and parser
│   ├── ast.rs            # FilterExpr AST structs
│   ├── apply.rs          # Filter application logic
│   └── mod.rs
├── indexer/
│   ├── builder.rs        # Aggregates history + agent messages
│   └── project_discovery.rs  # Scans ~/.claude/projects/
├── models/
│   ├── history.rs        # HistoryEntry, MessageContent
│   ├── conversation.rs   # ConversationEntry
│   ├── search.rs         # SearchEntry, EntryType
│   └── project.rs        # ProjectInfo
├── parsers/
│   ├── history.rs        # history.jsonl parser
│   ├── conversation.rs   # agent-*.jsonl parser
│   ├── deserializers.rs  # Custom timestamp/sessionId handling
│   └── mod.rs
├── tui/
│   ├── app.rs            # Main TUI application state and event loop
│   ├── layout.rs         # Split-pane layout calculations
│   ├── rendering.rs      # Ratatui rendering logic
│   ├── events.rs         # Keyboard event handling
│   ├── timestamps.rs     # Tiered timestamp formatting
│   └── mod.rs
└── utils/
    ├── environment.rs    # Home directory detection
    ├── paths.rs          # Encoding/decoding/validation
    └── terminal.rs       # ANSI sanitization
```

### Key Data Structures

**SearchEntry** (core data model):
```rust
pub struct SearchEntry {
    pub entry_type: EntryType,       // UserPrompt or AgentMessage
    pub display_text: String,        // Already truncated (1KB thinking, 4KB tool)
    pub timestamp: DateTime<Utc>,
    pub project_path: Option<String>,
    pub session_id: Option<String>,
}
```

**FilterExpr** (filter AST):
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

**StatusMessage** (TUI feedback):
```rust
pub struct StatusMessage {
    pub text: String,
    pub message_type: MessageType,  // Success | Error
    pub expires_at: Instant,
}
```

### Dependencies

**Phase 1:**
```toml
serde = "1.0"
serde_json = "1.0"
clap = { version = "4.0", features = ["derive"] }
anyhow = "1.0"
chrono = { version = "0.4", features = ["serde"] }
percent-encoding = "2.3"
uuid = { version = "1.0", features = ["serde"] }
```

**Phase 2:**
```toml
nucleo-picker = "0.3"      # Fuzzy finder (6-8x faster than skim)
ratatui = "0.29"           # TUI framework
crossterm = "0.28"         # Terminal backend
arboard = "3.4"            # Cross-platform clipboard
```

### Unresolved Design Questions

**Performance benchmarks for decision-making:**
- **Issue**: Current performance test has arbitrary <2s threshold for 1000 entries
- **Need**: Establish baseline performance metrics with larger datasets before making optimization decisions
- **Action items**:
  - [ ] Run performance benchmarks on realistic datasets (10K, 100K, 1M entries)
  - [ ] Document baseline metrics (parse time, memory usage, throughput)
  - [ ] Set performance thresholds based on actual data
  - [ ] Use benchmarks to evaluate parallel parsing ROI (rayon)
- **Status**: Deferred to Phase 3+ performance optimization

**Whitespace-only content filtering:**
- **Current behavior**: Filters `is_empty()` but NOT `trim().is_empty()`
- **Result**: Whitespace-only entries (" ") are indexed
- **Options**:
  - Keep current behavior (preserves whitespace-only messages)
  - Filter `trim().is_empty()` (removes whitespace-only messages)
- **Decision**: Keep current behavior for now
- **Reason**: May be edge case in real data; change if users report issues
- **Location**: `src/indexer/builder.rs:338`

**Filter precedence edge cases:**
- **Question**: How to handle ambiguous filter combos like `project:foo type:user OR type:agent`?
- **Current plan**:
  - Same-field OR: `project:foo project:bar` → (foo OR bar)
  - Cross-field AND: `project:foo type:user` → (foo AND user)
  - Explicit operators override: `type:user OR type:agent` → (user OR agent)
- **Edge case**: `project:foo type:user OR type:agent`
  - Interpretation A: `(project:foo AND type:user) OR type:agent`
  - Interpretation B: `project:foo AND (type:user OR type:agent)`
- **Decision**: Document behavior in Phase 2, refine in Phase 3 with parentheses support

---

**Last Updated**: 2025-11-23
**Status**: Phase 2 In Progress (Work Streams 1, 2, 3 complete)
**Next Milestone**: Phase 2 completion, Phase 3 planning
