//! JSONL parsers for Claude Code history and conversation files
//!
//! # Error Handling Strategy
//!
//! This module follows a **graceful degradation** approach suitable for CLI tools:
//!
//! - **Individual line failures**: Malformed JSON lines are logged to stderr and skipped,
//!   allowing parsing to continue. This prevents a single bad line from breaking the entire index.
//!
//! - **Catastrophic failure detection**: If >50% of lines fail to parse, or if >100 consecutive
//!   errors occur, the parser returns an error. This prevents accepting severely corrupted files.
//!
//! - **User feedback**: Summary statistics are printed showing successful entries, warnings, and
//!   failures, giving users visibility into parse quality.
//!
//! - **Error propagation**: Uses `anyhow::Result` for error handling with context. Since this is
//!   a binary/CLI tool (not a library), errors are boxed and consumers don't match on error types.
//!
//! This strategy balances robustness (tolerating minor corruption) with safety (rejecting
//! fundamentally broken files).

pub mod conversation;
pub mod deserializers;
pub mod history;

pub use conversation::parse_conversation_file;
pub use history::parse_history_file;
