# AI History Explorer

[![CI](https://github.com/syz51/ai-history-explorer/workflows/CI/badge.svg)](https://github.com/syz51/ai-history-explorer/actions)
[![codecov](https://codecov.io/gh/syz51/ai-history-explorer/branch/main/graph/badge.svg)](https://codecov.io/gh/syz51/ai-history-explorer)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

CLI tool for searching and browsing Claude Code conversation history stored in `~/.claude/`.

## Features

- Parses user prompts from `history.jsonl`
- Extracts agent conversations from project directories
- Builds searchable indexes combining all conversation data
- Graceful error handling with configurable failure thresholds
- Security-hardened path validation

## Installation

```bash
cargo install --path .
```

## Usage

```bash
# Show statistics about your Claude Code history
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
