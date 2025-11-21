//! Data models for Claude Code conversation history.
//!
//! This module defines the data structures used throughout the application:
//!
//! - [`HistoryEntry`] - User prompts from history.jsonl
//! - [`ConversationEntry`] - Messages from agent conversation files
//! - [`SearchEntry`] - Unified index entry combining user prompts and messages
//! - [`ProjectInfo`] - Discovered project metadata and file paths
//!
//! These models use serde for JSON deserialization with custom deserializers
//! for special fields (timestamps, session IDs) in the `deserializers` module.

pub mod history;
pub mod project;
pub mod search;

pub use history::{ConversationEntry, HistoryEntry, Message, MessageContent};
pub use project::ProjectInfo;
pub use search::{EntryType, SearchEntry};
