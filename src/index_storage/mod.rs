//! Persistent index storage with incremental updates
//!
//! Caches search index to disk for fast startup. Uses two-file approach:
//! - `index-metadata.json`: JSON metadata (mtime, size, timestamps)
//! - `search-index.bin`: Bincode-serialized search entries
//!
//! Cache location: platform-specific cache directories
//! - macOS: `~/Library/Caches/ai-history-explorer/`
//! - Linux: `~/.cache/ai-history-explorer/`
//! - Windows: `%LOCALAPPDATA%\ai-history-explorer\cache\`

pub mod metadata;
pub mod persistence;

pub use metadata::{HistoryFileMetadata, IndexMetadata, ProjectMetadata};
pub use persistence::{load_index, save_index};
