# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

AI History Explorer: CLI tool for searching/browsing Claude Code conversation history stored in `~/.claude/`. Parses user prompts from `history.jsonl` and agent conversations from project directories, building searchable indexes.

## Commands

**Build & Run:**

```bash
cargo build
cargo run -- stats
```

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
