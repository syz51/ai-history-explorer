# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

AI History Explorer: CLI tool for searching/browsing Claude Code conversation history stored in `~/.claude/`. Parses user prompts from `history.jsonl` and agent conversations from project directories, building searchable indexes.

## Commands

**Build & Run:**

```bash
cargo build
cargo run -- stats

# Force rebuild ignoring cache
cargo run -- --no-cache stats
```

**Cache:**

Index is cached for fast startup (~50-200ms vs 2-5s). Each Claude directory gets isolated cache using path hash:
- macOS: `~/Library/Caches/ai-history-explorer/<hash>/`
- Linux: `~/.cache/ai-history-explorer/<hash>/`
- Windows: `%LOCALAPPDATA%\ai-history-explorer\<hash>\`

Where `<hash>` is first 12 chars of path hash (e.g., `abc123def456`). This ensures:
- Test isolation (temp dirs get separate caches)
- Multiple Claude directories supported without conflicts
- Cache automatically invalidates when source files change

Use `--no-cache` flag to force rebuild (useful for debugging).

**Testing:**

```bash
cargo test
```

**Coverage:**

```bash
# Install cargo-llvm-cov (one-time setup)
cargo install cargo-llvm-cov

# Run tests with coverage report
cargo llvm-cov --all-features --workspace

# Generate LCOV report for CI/coverage tools
cargo llvm-cov --all-features --workspace --lcov --output-path lcov.info

# Generate HTML report for local viewing
cargo llvm-cov --all-features --workspace --html
open target/llvm-cov/html/index.html
```

Target: 90%+ line coverage (enforced in pre-commit hooks and CI)

**Linting:**

```bash
cargo clippy
```

**Type checking:**

```bash
cargo check
```

## Pre-commit Hooks

**Setup:**

```bash
# Install nightly toolchain (required for rustfmt with nightly features)
rustup toolchain install nightly

# Install prek (Rust-based pre-commit tool)
cargo install prek

# Install git hooks
prek install
```

**Pre-commit checks (auto-run on commit):**

- `cargo +nightly fmt --all -- --check` - formatting (uses nightly for advanced features)
- `cargo clippy --all-targets -- -D warnings` - linting
- `cargo check --workspace` - type checking
- `cargo test --lib` - fast unit tests
- `cargo llvm-cov` - coverage check (enforces 90%+ line coverage)

**Manual execution:**

```bash
prek run --all-files  # run all pre-commit checks
```

**Skip hooks (discouraged):**

```bash
git commit --no-verify
```

All checks enforced in CI as backup.

## Architecture

**Module structure:**

- `parsers/`: JSONL parsers for history.jsonl and agent conversation files
  - Uses graceful degradation: skips malformed lines, fails if >50% of lines fail or >100 consecutive errors
  - Custom deserializers handle timestamp formats and optional session IDs
- `indexer/`: Builds searchable index combining user prompts + agent messages
  - `project_discovery`: Scans ~/.claude/projects/ for encoded project directories
  - `builder`: Aggregates entries from history.jsonl + all agent files, validates >50% success rate
- `models/`: Core data structures (HistoryEntry, ConversationEntry, SearchEntry, ProjectInfo)
- `utils/paths`: Path encoding/decoding for Claude's percent-encoded project directories
  - Validates paths to prevent traversal attacks (rejects `..` components, non-absolute paths)
  - Enforces 10MB file size limit to prevent DoS
- `cli/`: Command-line interface (currently only `stats` command)

**Error handling philosophy:**
Graceful degradation suitable for CLI tools: skip malformed individual entries/files with warnings, but fail if >50% corrupt to prevent accepting fundamentally broken data. All errors logged to stderr with summary statistics.

**Path security:**
Project directories use percent encoding (e.g., `/Users/foo/bar` → `-Users%2Ffoo%2Fbar`). All decoded paths validated against traversal attacks and must be absolute.

**Persistent Index:**

- Index cached to disk after building (bincode binary + JSON metadata)
- Per-directory cache isolation: each Claude directory gets separate cache via path hash
- Incremental updates: only parses changed files on subsequent runs
- Staleness detection via mtime/size checks on history.jsonl and project directories
- Stale project cleanup: cached entries for removed projects automatically purged
- Automatic fallback to full rebuild if cache corrupted/version mismatch
- Cache invalidation: content-change only (no time-based expiration)

**Entry types:**

- `EntryType::UserPrompt`: User messages from both history.jsonl and agent conversation files
- `EntryType::AgentMessage`: Currently filtered out (not included in index)

**Data flow:**

1. Parse history.jsonl → extract user prompts
2. Discover projects in ~/.claude/projects/
3. Parse agent conversation files → extract user messages (type="user")
4. Combine all entries → sort by timestamp (newest first)
5. Return unified SearchEntry index

- For cargo.toml, the latest edition is 2024
