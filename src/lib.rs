//! AI History Explorer - Search and browse Claude Code conversation history
//!
//! This library provides tools for parsing and indexing Claude Code's local conversation
//! history stored in `~/.claude/`. It supports:
//!
//! - Parsing user prompts from `history.jsonl`
//! - Discovering and parsing agent conversations from project directories
//! - Building searchable indexes of conversation entries
//! - Path encoding/decoding for Claude's project directory format
//!
//! # Example
//!
//! ```no_run
//! use ai_history_explorer::build_index;
//! use std::path::PathBuf;
//!
//! let claude_dir = PathBuf::from("/Users/alice/.claude");
//! let index = build_index(&claude_dir)?;
//! println!("Indexed {} entries", index.len());
//! # Ok::<(), anyhow::Error>(())
//! ```

pub mod cli;
pub mod filters;
pub mod indexer;
pub mod models;
pub mod parsers;
pub mod utils;

// Re-export commonly used types
pub use indexer::builder::build_index;
pub use models::search::SearchEntry;
pub use parsers::history::parse_history_file;
pub use utils::paths::{decode_path, encode_path, format_path_with_tilde};
