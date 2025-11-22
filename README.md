# AI History Explorer

[![CI](https://github.com/syz51/ai-history-explorer/workflows/CI/badge.svg)](https://github.com/syz51/ai-history-explorer/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

CLI tool for searching and browsing Claude Code conversation history stored in `~/.claude/`.

## Features

- **Interactive fuzzy search TUI** with real-time filtering
- **Advanced filter syntax** for precise searching (project, type, date)
- Parses user prompts from `history.jsonl`
- Extracts agent conversations from project directories
- Builds searchable indexes combining all conversation data
- Supports all message types: text, thinking blocks, tool use/results, images
- Content truncation with DoS protection for large tool inputs/results
- Graceful error handling with configurable failure thresholds
- Security-hardened path validation

## Security & Privacy

**Personal Use Only**: This tool is designed for personal use to search your own Claude Code conversation history. Tool results and conversation content may contain sensitive information (API keys, credentials, file contents, etc.). Do not share search indexes or use on shared systems without understanding the privacy implications.

**Content Handling**:

- **Truncation**: Large content blocks (thinking blocks, tool inputs/results, image alt text) are automatically truncated to prevent denial-of-service attacks while preserving search utility
  - Thinking blocks: Limited to 1KB
  - Tool inputs/results: Limited to 4KB
  - Image alt text: Limited to 1KB
  - Truncated content is marked with `[truncated]` indicators
- **Empty Content Filtering**: Messages with no text content (e.g., images without alt text) are automatically filtered from the search index
- **JSON Size Limits**: Tool inputs/results containing large JSON structures are serialized with 4KB limits to prevent excessive memory allocation

**DoS Protection**: The indexer implements multiple layers of protection against maliciously crafted or corrupted conversation files:

- File size limit: 10MB per file
- JSON serialization limits prevent unbounded allocation
- UTF-8 boundary-safe truncation prevents panics
- Graceful degradation: <50% failure rate tolerated before rejecting data

## Installation

```bash
cargo install --path .
```

## Usage

### Interactive Mode (Recommended)

Launch the interactive fuzzy finder TUI:

```bash
ai-history-explorer interactive
```

### Filter Syntax

Filters use `field:value` syntax. Combine filters with the fuzzy search using the `|` separator:

```
project:name type:user | fuzzy search terms
^^^^^^^^^^^^^^^         ^^^^^^^^^^^^^^^^^^^
Filter portion           Fuzzy portion
```

**Supported Fields:**

- `project:<name>` - Filter by project path (case-insensitive, partial match)
  - Example: `project:ai-history` matches `/Users/you/ai-history-explorer`
  - Supports `~` expansion: `project:~/Documents`
- `type:<user|agent>` - Filter by entry type
  - `type:user` - Only user prompts
  - `type:agent` - Only agent responses
- `since:<YYYY-MM-DD>` - Filter entries after date
  - Example: `since:2024-01-15`

**Operators:**

- **AND** (default between different fields): `project:foo type:user`
- **OR** (default within same field): `project:foo project:bar`
- Explicit operators: `project:foo AND type:user` or `type:user OR type:agent`

**Examples:**

```
project:ai-history | implement tui
type:user | refactor
project:ai-history type:user | search
since:2024-01-01 | recent changes
```

### Keybindings

**Navigation:**
- `↑` / `Ctrl+p` - Previous entry
- `↓` / `Ctrl+n` - Next entry
- `Page Up` / `Page Down` - Scroll preview

**Actions:**
- `Enter` - Apply filters
- `Esc` - Clear input (or quit if empty)
- `Ctrl+C` - Quit

### Stats Mode

Show statistics about your conversation history:

```bash
ai-history-explorer stats
```

## Development

See [CLAUDE.md](CLAUDE.md) for detailed development instructions.

### Quick Start

```bash
# Run tests
cargo test

# Check coverage (requires 90%+)
cargo llvm-cov --all-features --workspace

# Run pre-commit checks
pre-commit run --all-files
```

## License

MIT
