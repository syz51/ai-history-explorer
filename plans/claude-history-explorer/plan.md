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

### Phase 1: Core Infrastructure (Current)

- [ ] Create Rust project structure (`ai-history-explorer/` binary)
- [ ] Add dependencies: serde, serde_json, clap, anyhow, chrono
- [ ] Implement JSONL parser for `history.jsonl` format
- [ ] Implement data structures (HistoryEntry, EntryType enum)
- [ ] Implement path encoding/decoding (`/Users/foo/bar` ↔ `-Users-foo-bar`)
- [ ] Implement project discovery in `~/.claude/projects/*/`
- [ ] Implement agent conversation parser for `agent-*.jsonl` files
- [ ] Build unified index from user prompts + agent messages
- [ ] Add CLI arguments: `--help`, `--version`, `--stats`
- [ ] Implement path display with ~ substitution
- [ ] Handle edge cases: missing files, malformed JSON, empty history
- [ ] Add graceful degradation for format changes (version detection if possible)
- [ ] Add tests for JSONL parsing, path encoding, index building

**Testing Notes:**

- Permission denied test (`test_parse_permission_denied`) uses Unix-specific APIs (`std::os::unix::fs::PermissionsExt`)
- Windows builds require `#[cfg(unix)]` conditional compilation for this test
- Alternative: implement cross-platform permission test or accept Unix-only coverage

### Phase 2+: TUI & Advanced Features (Deferred)

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

**Pending:**

- Preview lazy loading implementation strategy (defer to Phase 2)
- Field filter syntax precedence details (defer to Phase 2+)
