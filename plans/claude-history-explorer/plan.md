# Claude Code History Explorer

## Spec

A terminal-based tool named `ai-history-explorer` that allows users to search through their Claude Code conversation history using fuzzy finding. Users can browse all past prompts and agent conversations, preview the context (project, timestamp, session), and copy selected prompts to the system clipboard for re-use in new Claude Code sessions. Part of a growing suite of AI development tools.

The tool reads from `~/.claude/history.jsonl` for user prompts and `~/.claude/projects/*/agent-*.jsonl` for agent sub-conversations, providing a fast fuzzy search interface. Selected text is automatically copied to the system clipboard, enabling quick iteration on previous prompts.

Initial version focuses on searching user prompts and agent conversations, with full conversation history (including Claude responses) planned for future enhancement.

## Plan

**Phase 1: Core Infrastructure**
Build a Rust CLI tool that parses the JSONL history files and presents them in a searchable interface. Start with user prompts from `history.jsonl` and agent messages from `agent-*.jsonl` files across all projects.

**Phase 2: Fuzzy Finding Interface**
Research and compare fuzzy finder libraries (skim, nucleo, fzf integration, custom with ratatui). Document comparison in plan for future decision. Initially implement with a chosen library (likely skim or nucleo) that provides good performance and UX.

**Phase 3: Clipboard Integration**
Implement system clipboard copying using platform-specific tools (pbcopy on macOS, xclip/wl-copy on Linux, clip.exe on Windows) or use a Rust clipboard library like `arboard` for cross-platform support.

**Data Model:**

- Parse `~/.claude/history.jsonl` for user prompts
- Discover project directories in `~/.claude/projects/`
- Parse `agent-*.jsonl` files for agent conversations
- Build searchable index with: display text, timestamp, project path, session ID
- Handle project path encoding (slashes → dashes)

## Tasks

### Phase 1: Core Infrastructure ✅ COMPLETE

- ✅ Create Rust project structure (`ai-history-explorer/` binary)
- ✅ Add dependencies: serde, serde_json, clap, anyhow, chrono, percent-encoding, uuid
- ✅ Implement JSONL parser for `history.jsonl` format
- ✅ Implement data structures (HistoryEntry, EntryType enum)
- ✅ Implement path encoding/decoding with percent encoding (security fix)
- ✅ Implement project discovery in `~/.claude/projects/*/`
- ✅ Implement agent conversation parser for `agent-*.jsonl` files
- ✅ Build unified index from user prompts + agent messages
- ✅ Add CLI arguments: `--help`, `--version`, `--stats`
- ✅ Implement path display with ~ substitution
- ✅ Handle edge cases: missing files, malformed JSON, empty history
- ✅ Add graceful degradation with >50% failure threshold
- ✅ Add tests for JSONL parsing, path encoding, index building
- ✅ **Security hardening** (6 protections: symlink validation, JSON depth, resource limits, file size, path traversal, terminal sanitization)
- ✅ **Comprehensive test suite** (178 tests: 125 unit + 47 integration + 6 doctests)
- ✅ **Test coverage enforcement** (97.03% achieved, 90%+ target in pre-commit hooks)
- ✅ **Code review fixes** (21/22 issues resolved, 1 deferred)

**Phase 1 Completion Stats** (2025-01-21):

- **178 tests passing** (100% pass rate)
- **97.03% code coverage** (98.48% line coverage)
- **Zero clippy warnings**
- **21/22 issues fixed** (1 deferred to Phase 2: Windows support)
- **Production-ready** for macOS

**Testing Notes:**

- Platform-specific tests use `#[cfg(unix)]` for symlinks
- Windows support deferred to Phase 2+
- Test utilities in `tests/common/mod.rs` for fixture builders

### Phase 2+: TUI & Advanced Features

**Platform Support** (deferred from Phase 1):

- [ ] **Windows support** (#4 from code review)
  - [ ] Cross-platform home directory detection (dirs or home crate)
  - [ ] Windows path handling tests
  - [ ] Different HOME env var behavior (USERPROFILE)
  - [ ] Symlink tests for Windows (different API)

**TUI Features**:

- [ ] Add dependencies: nucleo-picker, ratatui, arboard
- [ ] Integrate nucleo-picker for fuzzy search
- [ ] Design preview window layout (scrollable, lazy loading strategy TBD)
- [ ] Implement TUI with split-pane (results + preview)
- [ ] Implement tiered timestamps (relative <7d: "2h ago", absolute older: "Jan 15")
- [ ] Implement status bar (filter count + total entries, project, shortcuts)
- [ ] Implement terminal color scheme (dark zinc bg, emerald accents)
- [ ] Implement clipboard copying (arboard)
- [ ] Implement field filter parsing (`project:foo`, `type:agent`, `since:date`)
- [ ] Implement field filter application before fuzzy match
- [ ] Add advanced search: date ranges, negation, regex
- [ ] Add multi-select support for batch copying multiple prompts

**Performance Enhancements**:

- [ ] **Parallel agent file parsing** (high impact)
  - [ ] Use rayon for concurrent parsing
  - [ ] Benchmark performance gain
- [ ] **Streaming instead of full memory load** (medium impact)
  - [ ] Stream entries instead of collect() into Vec
  - [ ] Lazy evaluation for large datasets
- [ ] **Preview text lazy loading** (low impact)
  - [ ] Only load preview when displayed

**Testing & Quality** (optional):

- [ ] **Performance benchmarks**
  - [ ] Install criterion crate
  - [ ] Benchmark parsing 10K, 100K, 1M entries
  - [ ] Track regression in CI
- [ ] **Concurrent access tests**
  - [ ] Multiple processes reading ~/.claude/ simultaneously
  - [ ] File modification during parsing edge cases
- [ ] **Fuzzing** (advanced)
  - [ ] Install cargo-fuzz
  - [ ] Fuzz parsers with AFL/libfuzzer
  - [ ] Target: malformed JSONL, extreme nesting, encoding attacks

**Distribution**:

- [ ] Write README with installation and usage instructions
- [ ] Package for cargo install (initial release)
- [ ] (Future) Add homebrew, apt, scoop distribution

## Context

### Data Model Source (IMPORTANT)

The data model below is **reverse-engineered from local files**, not from official documentation. Claude Code's official docs do not publish specifications for the local storage format. This is the only available approach for building history browsing tools, as the Agent SDK is designed for building agents, not accessing historical conversations. The format may change in future Claude Code versions without notice.

**Claude Code Data Storage:**

- `~/.claude/history.jsonl` - Global user prompt history (595KB, 2375 lines)

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

**Key Implementation Notes:**

- User prompts: Easy access via single `history.jsonl` file
- Agent conversations: Require scanning all `~/.claude/projects/*/agent-*.jsonl` files
- Full conversations (future): Requires parsing `<sessionId>.jsonl` and threading via `parentUuid`
- All files use JSONL format (one JSON object per line) - use `serde_json` streaming parser

---

## Decisions

**Resolved:**

- **Tool name**: `ai-history-explorer` (extensible for future AI dev tools)
- **Repository**: Use current repo (ai-history-explorer)
- **Copy method**: System clipboard (via arboard crate)
- **Search scope**: User prompts + agent conversations initially; full conversation history later
- **Distribution**: Cargo install initially; homebrew/apt/scoop later
- **Preview layout**: Design after project skeleton setup
- **Multi-select**: Deferred to future (allows selecting multiple prompts to copy at once, concatenated)
- **Search syntax**: Basic fuzzy matching initially; advanced syntax (exact match `"..."`, regex `/.../"`, field filters `project:foo`) deferred to future

**Phase 1 Decisions (2025-01-21):**

- **Fuzzy finder library**: Nucleo (pure Rust, 6-8x faster than skim, native integration, future columnar matching support)
- **Architecture pivot**: Rust CLI with TUI (from TypeScript prototype reusing UI/UX concepts)
- **Preview content**: Scrollable display, lazy loading strategy TBD for Phase 2 implementation
- **Timestamp format**: Tiered (relative for <7 days: "2h ago", absolute for older: "Jan 15, 2024")
- **Project path display**: Full path with ~ substitution for home directory
- **Status bar**: Show filter count + total entries count
- **Phase 1 scope**: Core infrastructure only (parsing, indexing, CLI stats)
  - NO TUI, fuzzy search, clipboard, or field filters in Phase 1
  - Pure fuzzy matching initially, field filters (`project:foo`, `type:agent`) deferred to Phase 2+

**Phase 1 Retrospective (Completed 2025-01-21):**

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

**Documentation**:

- Consolidated design decisions: `docs/design-decisions-phase1.md`
- Archived original docs: `docs/archive/TESTING.md`, `docs/archive/code-review-phase1.md`

**Pending:**

- Preview lazy loading implementation strategy (defer to Phase 2)
- Field filter syntax precedence details (defer to Phase 2+)
