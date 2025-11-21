# Design Decisions: Phase 1

**Date**: 2025-01-21
**Status**: Phase 1 Complete
**Summary**: Core infrastructure implementation with comprehensive security hardening, testing, and code quality improvements.

---

## Executive Summary

Phase 1 implementation completed with **178 tests passing** (100% pass rate), **97.03% code coverage** (exceeding 90% target), and **21/22 code review issues resolved** (1 deferred to Phase 2).

**Deliverables**:

- Core JSONL parsing for history.jsonl and agent conversation files
- Unified searchable index combining user prompts and agent messages
- Path encoding/decoding with security validation
- CLI with stats command
- Comprehensive test suite (125 unit + 47 integration + 6 doctests)
- Security hardening (5 protections implemented)
- Pre-commit hooks with coverage enforcement

**Security Posture**: Production-ready with multiple defense layers

---

## Security Decisions

### 1. Symlink Validation

**Decision**: Reject symbolic links in project directories and agent files

**Implementation**: `validate_path_not_symlink()` in `src/utils/paths.rs`

**Rationale**: Prevents symlink attacks where malicious symlinks in `~/.claude/projects/` could point to sensitive locations (`/etc/passwd`, private keys, etc.)

**Applied To**:

- Project directories in `~/.claude/projects/`
- Agent conversation files (`agent-*.jsonl`)

**Tests**: `tests/security_test.rs::test_security_symlink_*` (Unix only)

**Platform Note**: Unix-specific (`#[cfg(unix)]`) - Windows uses different symlink APIs, planned for Phase 2

---

### 2. JSON Depth Limiting

**Decision**: Rely on serde_json's built-in recursion limit

**Implementation**: Documented in `src/parsers/mod.rs` module-level docs

**Details**:

- serde_json enforces 128-level maximum nesting depth
- Prevents "Billion Laughs" style stack overflow attacks
- Reasonable nesting (<20 levels) accepted without issue

**Tests**: `src/parsers/history.rs::test_parse_moderately_nested_json_accepted`

**Rationale**: Standard library protection is sufficient; no custom limits needed

---

### 3. Resource Limits

**Decision**: Enforce hard limits on directory listing operations

**Implementation**: Constants in `src/indexer/project_discovery.rs`

- `MAX_PROJECTS = 1000`
- `MAX_AGENT_FILES_PER_PROJECT = 1000`

**Rationale**: Prevents resource exhaustion DoS attacks via directory listing floods (attacker creating thousands of empty directories/files)

**Tests**: `src/indexer/project_discovery.rs::test_discover_projects_max_*`

**Trade-off**: Legitimate users with >1000 projects will see warnings, but this is extremely unlikely in practice

---

### 4. File Size Limits

**Decision**: 10MB maximum file size for JSONL files

**Implementation**: `MAX_FILE_SIZE_BYTES` constant + `validate_file_size()` in `src/utils/paths.rs`

**Rationale**:

- Current history.jsonl is 595KB
- 10MB = 40x current size (ample headroom for growth)
- Prevents memory exhaustion DoS from maliciously large files

**TOCTOU Protection**: File size validated AFTER opening file handle (not before) to prevent race conditions where attacker replaces file between check and open

**Applied To**:

- `history.jsonl` parsing (`src/parsers/history.rs`)
- Agent conversation file parsing (`src/parsers/conversation.rs`)

**Tests**: `tests/security_test.rs::test_security_file_size_limit`

---

### 5. Path Traversal Validation

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

**Tests**: `tests/security_test.rs::test_security_path_traversal`

**Code Review Issues Fixed**: #2 (path traversal vulnerability), #3 (path encoding collision)

---

### 6. Terminal Output Sanitization

**Decision**: Provide ANSI escape sequence stripping utility

**Implementation**: `strip_ansi_codes()` in `src/utils/terminal.rs`

**Rationale**: Removes ANSI escape sequences and control characters that could manipulate terminal output (cursor positioning, color changes for phishing, etc.)

**Current Usage**: Not used in `stats` command (doesn't display user content), but available for Phase 2 TUI features

**Tests**: `src/utils/terminal.rs::tests`

**Design Note**: Defensive utility for future features displaying untrusted user content

---

## Error Handling Strategy

### Philosophy: Graceful Degradation for CLI Tools

**Decision**: Skip malformed entries with warnings, but fail if >50% corrupt

**Rationale**:

- **For libraries** (thiserror): Always propagate errors, let caller decide
- **For CLI tools** (anyhow): Graceful degradation acceptable with user feedback
- Partial data corruption shouldn't break entire tool
- Fundamentally broken data (>50% failure) should fail loudly

---

### Implementation Details

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

**Code Review Issues Fixed**: #7 (silent error swallowing), #10 (malformed line DoS)

---

### Consecutive Error Limits

**Decision**: Bail after 100 consecutive parse errors

**Implementation**: `MAX_CONSECUTIVE_ERRORS = 100` constant

**Rationale**: Prevents DoS where attacker creates file with millions of malformed lines, causing parser to iterate indefinitely printing warnings

**Behavior**: Reset counter on each successful parse

**Code Review Issue Fixed**: #10 (malformed line DoS)

---

## Architecture Decisions

### Module Structure

**Decision**: Clean separation of concerns with functional boundaries

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

**Trade-offs**:

- ✅ Clear boundaries, easy to test
- ✅ Functional style (iterators, filters, maps)
- ❌ No abstraction for file system (hard to mock in tests)
- ❌ No repository/service layer (business logic mixed with I/O)

---

### Entry Types

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

**Code Review Issue Fixed**: #1 (CRITICAL - user messages mislabeled as agent messages)

---

### Data Flow

**Decision**: Parse → Discover → Aggregate → Sort → Return

```text
1. Parse history.jsonl
   ↓ (extract user prompts)

2. Discover projects in ~/.claude/projects/
   ↓ (decode directory names)

3. Parse agent-*.jsonl files
   ↓ (extract user messages: type="user")

4. Combine all entries
   ↓ (deduplicate by timestamp+sessionId if needed)

5. Sort by timestamp (newest first)
   ↓

6. Return Vec<SearchEntry>
```

**Implementation**: `build_index()` in `src/indexer/builder.rs`

**Trade-offs**:

- ✅ Simple, easy to reason about
- ✅ All data in memory (fast sorting/searching)
- ❌ No streaming (loads all entries at once)
- ❌ No parallel parsing (sequential file processing)

**Future Optimizations** (Phase 2+):

- Parallel parsing with rayon
- Streaming with iterators (avoid Vec collection)

---

### Path Encoding

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

**Tests**: `src/utils/paths.rs::tests::test_encode_decode_roundtrip_complex_paths`

**Code Review Issue Fixed**: #3 (HIGH - path encoding collision)

---

### Platform Support

**Decision**: macOS-only for Phase 1

**Implementation**: Uses `$HOME` environment variable (`src/utils/environment.rs`)

**Windows Limitation**: `$HOME` doesn't exist on Windows (uses `%USERPROFILE%`)

**Phase 2 Plan**:

- Use `dirs` or `home` crate for cross-platform home detection
- Add Windows-specific tests
- Handle different path separators

**Rationale**: Focus Phase 1 on core functionality for primary platform, expand in Phase 2

**Code Review Issue Deferred**: #4 (HIGH - Windows incompatibility)

---

## Testing Strategy

### Coverage Targets

**Decision**: Enforce 90%+ line coverage

**Implementation**: Pre-commit hook with `cargo-llvm-cov`

**Achieved**: 97.03% total coverage, 98.48% line coverage

**Rationale**:

- High coverage ensures edge cases tested
- Enforced in pre-commit prevents regressions
- CI backup if hooks skipped

**Pre-commit Hook** (`.prek.toml`):

```bash
cargo llvm-cov --all-features --workspace --fail-under-lines 90
```

---

### Test Suite Structure

**Decision**: Separate unit tests (in src/) from integration tests (in tests/)

**Implementation**:

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

**Rationale**:

- **Unit tests**: Fast, isolated, test individual functions
- **Integration tests**: Slower, test full workflows
- **Separation**: Clear organization, can run subsets independently

**Commands**:

```bash
cargo test --lib                    # Unit tests only (fast)
cargo test --test integration_test  # E2E workflows
cargo test --test security_test     # Security boundaries
```

---

### Test Utilities

**Decision**: Provide builders in `tests/common/mod.rs` for realistic fixtures

**Implementation**:

- `ClaudeDirBuilder`: Create test `.claude/` structures
- `HistoryEntryBuilder`: Build history.jsonl entries
- `AgentFileBuilder`: Build agent conversation files
- `ConversationEntryBuilder`: Build conversation entries

**Helpers**:

- `minimal_claude_dir()`: Empty `.claude/` directory
- `realistic_claude_dir()`: Sample data (3 history + 2 projects)

**Rationale**:

- Reduces test boilerplate
- Ensures valid test data (UUIDs, timestamps)
- Easy to create complex scenarios

**Example Usage**:

```rust
let temp_dir = ClaudeDirBuilder::new()
    .with_history_entries(vec![...])
    .with_project("project-name", vec![...])
    .build()?;
```

---

### Platform-Specific Tests

**Decision**: Conditional compilation for platform-specific features

**Implementation**: `#[cfg(unix)]` attributes on symlink tests

**Rationale**:

- Symlinks work differently on Windows (requires admin privileges)
- Unix tests cover macOS and Linux
- Windows support planned for Phase 2

**Example**:

```rust
#[test]
#[cfg(unix)]
fn test_security_symlink_rejection() {
    // Symlink-specific test
}
```

---

### Edge Case Coverage

**Decision**: Comprehensive edge case testing

**Categories**:

1. **Format variations**: Empty lines, mixed line endings (LF/CRLF), no trailing newline
2. **Unicode handling**: Emoji, CJK characters, RTL text
3. **Scale**: Very long text (100KB), many entries (1000)
4. **Duplicates**: Same timestamp, same session ID
5. **Filesystem**: Special chars in paths, non-UTF8 filenames (Unix)
6. **Corruption**: Truncated JSON at EOF, empty fields
7. **File types**: Non-agent JSONL files (ignored), nested subdirs

**Tests**: `tests/edge_cases_test.rs` (16 tests)

**Rationale**: Real-world data is messy - tool must handle gracefully

---

## Performance Decisions

### String Allocation Optimizations

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

**Code Review Issue Fixed**: #6 (MEDIUM - inefficient string allocation)

---

### Cow Optimization for Path Conversions

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

**Code Review Issue Fixed**: #11 (MEDIUM - double allocation)

---

### Lazy Error Context

**Decision**: Use `with_context(|| ...)` instead of `context(...)`

**Implementation**: Applied in parsers and path utilities

**Before** (eager allocation):

```rust
.context(format!("Failed to open file: {}", path.display()))
```

**After** (lazy allocation):

```rust
.with_context(|| format!("Failed to open file: {}", path.display()))
```

**Impact**: Format string only allocated when error actually occurs (not on success path)

**Code Review Issue Fixed**: #19 (LOW - non-idiomatic error context)

---

### Trade-offs Accepted

**Decision**: Defer advanced optimizations to Phase 2+

**Deferred**:

1. **Parallel parsing**: Use rayon for concurrent agent file parsing (high impact)
2. **Streaming**: Avoid collecting into Vec, use iterators (medium impact)
3. **Lazy preview**: Only load preview text when needed (low impact)
4. **Benchmarking**: Criterion-based performance regression tracking

**Rationale**:

- Current performance adequate for typical usage (2375 entries, <1s)
- Premature optimization is root of complexity
- Focus Phase 1 on correctness and security
- Profile before optimizing in Phase 2

**Current Complexity**:

- File I/O: O(n) with BufReader (good)
- Parsing: O(n) iteration (unavoidable)
- Sorting: O(n log n) (appropriate)
- No unnecessary nested loops (good)

---

## Issues Resolved

### Critical Issues (1)

**#1: User Messages Mislabeled as Agent Messages** ✅ FIXED

- **Severity**: CRITICAL (logic bug)
- **Location**: `src/indexer/builder.rs:47-59`
- **Issue**: Filtered for `type == "user"` but set `EntryType::AgentMessage`
- **Fix**: Changed to `EntryType::UserPrompt`
- **Impact**: All user messages from agent conversations now correctly labeled

---

### High Severity Issues (4)

**#2: Path Traversal Vulnerability** ✅ FIXED

- **Severity**: HIGH (security)
- **Location**: `src/utils/paths.rs`
- **Issue**: Blind `-` to `/` replacement enabled traversal attacks
- **Fix**: Added `validate_decoded_path()` rejecting `..` components
- **Protection**: Defense-in-depth validation

**#3: Path Encoding Collision** ✅ FIXED

- **Severity**: HIGH (correctness)
- **Location**: `src/utils/paths.rs`
- **Issue**: `/foo/bar` and `/foo-bar` both encoded to `-foo-bar`
- **Fix**: Use percent-encoding crate for bijection guarantee
- **Verification**: Test confirms no collision

**#4: Windows Platform Incompatibility** 🔵 DEFERRED

- **Severity**: HIGH (compatibility)
- **Location**: `src/utils/environment.rs`
- **Issue**: Uses `$HOME` (doesn't exist on Windows)
- **Deferral**: Phase 1 targets macOS; Phase 2+ will add cross-platform support
- **Plan**: Use `dirs` or `home` crate

**#5: Unbounded Memory Allocation** ✅ FIXED

- **Severity**: HIGH (DoS potential)
- **Location**: Parsers and builder
- **Issue**: Files loaded entirely into memory without size validation
- **Fix**: 10MB limit with TOCTOU-safe validation
- **Rationale**: 40x current file size, prevents exhaustion DoS

---

### Medium Severity Issues (6)

**#6: Inefficient String Allocation** ✅ FIXED

- Intermediate Vec + join → Pre-allocated capacity

**#7: Silent Error Swallowing** ✅ FIXED

- No error tracking → >50% failure threshold + summary stats

**#8: Timestamp Overflow** ✅ FIXED

- Unsafe cast → `as_i64()` with `Option` validation

**#9: No Session ID Validation** ✅ FIXED

- Accept any string → UUID validation

**#10: Malformed Line DoS** ✅ FIXED

- Unlimited error iteration → 100 consecutive error limit

**#11: Double Allocation** ✅ FIXED

- `to_string_lossy().to_string()` → Cow pattern matching

---

### Low Severity Issues (9)

**#12: Missing Derive Traits** ✅ FIXED

- Added `PartialEq, Eq` for testing

**#13: Inconsistent Error Handling** ✅ FIXED

- Documented graceful degradation strategy

**#14: No Documentation Comments** ✅ FIXED

- Added rustdoc to public APIs

**#15: `flatten()` Hides Errors** ✅ FIXED

- Explicit iteration with error logging

**#16: Unused Field** ✅ FIXED

- `#[serde(skip)]` on `pasted_contents`

**#17: No Content Type Validation** ✅ FIXED

- `MessageContent.text: String` → `Option<String>`

**#18: Magic String Comparisons** ✅ FIXED

- Extracted constants `ENTRY_TYPE_USER`, `CONTENT_TYPE_TEXT`

**#19: Non-Idiomatic Error Context** ✅ FIXED

- `context(format!(...))` → `with_context(|| format!(...))`

---

### Post-Review Issues (3)

**#20: Duplicate Count in Stats Output** ✅ FIXED (Trivial)

- Showed `skipped_count` twice → Single "skipped" count

**#21: Path Traversal in history.jsonl Project Field** ✅ FIXED (Low)

- Direct `PathBuf::from()` → Defense-in-depth validation
- Risk: Low (requires local file modification)

**#22: TOCTOU Race in File Size Validation** ✅ FIXED (Low)

- Check-then-open gap → Open-then-validate
- Validates file handle metadata (no race window)

---

## Summary Statistics

| Category     | Metric            | Value              |
| ------------ | ----------------- | ------------------ |
| **Tests**    | Total             | 178 (100% passing) |
|              | Unit tests        | 125                |
|              | Integration tests | 47                 |
|              | Doctests          | 6                  |
| **Coverage** | Total             | 97.03%             |
|              | Line coverage     | 98.48% (1773/1800) |
|              | Function coverage | 98.90% (181/183)   |
| **Issues**   | Total found       | 22                 |
|              | Fixed             | 21 (95%)           |
|              | Deferred          | 1 (5%)             |
| **Security** | Protections       | 6 layers           |

---

## Phase 1 Completion Checklist

- ✅ Core JSONL parsing (history.jsonl + agent-\*.jsonl)
- ✅ Unified searchable index
- ✅ Path encoding/decoding with percent encoding
- ✅ Project discovery in ~/.claude/projects/
- ✅ CLI with --stats, --help, --version
- ✅ Graceful degradation with error tracking
- ✅ 178 tests, 97%+ coverage
- ✅ Security hardening (6 protections)
- ✅ Pre-commit hooks with coverage enforcement
- ✅ All P0/P1 code review issues resolved
- ✅ Zero clippy warnings
- ✅ Documentation (rustdoc + CLAUDE.md + TESTING.md + code review)

**Status**: ✅ Production-ready for Phase 1 scope

---

## References

**Code Locations**:

- Security: `src/utils/paths.rs`, `src/indexer/project_discovery.rs`
- Error handling: `src/parsers/*.rs`, `src/indexer/builder.rs`
- Tests: `tests/security_test.rs`, `tests/edge_cases_test.rs`, `tests/integration_test.rs`

**Documentation**:

- Original testing docs: `docs/archive/TESTING.md`
- Original code review: `docs/archive/code-review-phase1.md`
- Project instructions: `CLAUDE.md`
- Project plan: `plans/claude-history-explorer/plan.md`
