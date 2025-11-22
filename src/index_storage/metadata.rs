//! Cache metadata structures for staleness detection

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::SystemTime;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Cache schema version for invalidation on format changes
pub const CACHE_VERSION: u32 = 1;

/// Top-level cache metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexMetadata {
    pub version: u32,
    pub history_file: HistoryFileMetadata,
    pub projects: HashMap<String, ProjectMetadata>,
}

/// Metadata for history.jsonl file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryFileMetadata {
    pub mtime_secs: i64,
    pub size: u64,
    pub max_timestamp: Option<DateTime<Utc>>,
}

/// Metadata for each project directory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMetadata {
    pub dir_mtime_secs: i64,
    pub max_timestamp: Option<DateTime<Utc>>,
    pub file_count: usize,
}

impl HistoryFileMetadata {
    /// Create metadata from file path
    pub fn from_path(path: &Path) -> anyhow::Result<Self> {
        let metadata = fs::metadata(path)?;
        let mtime = metadata.modified()?;
        let mtime_secs = mtime.duration_since(SystemTime::UNIX_EPOCH)?.as_secs() as i64;

        Ok(Self { mtime_secs, size: metadata.len(), max_timestamp: None })
    }

    /// Check if file has changed (mtime or size differs)
    pub fn is_stale(&self, path: &Path) -> anyhow::Result<bool> {
        let current = Self::from_path(path)?;
        Ok(self.mtime_secs != current.mtime_secs || self.size != current.size)
    }
}

impl ProjectMetadata {
    /// Create metadata from project directory path
    pub fn from_path(path: &Path, file_count: usize) -> anyhow::Result<Self> {
        let metadata = fs::metadata(path)?;
        let mtime = metadata.modified()?;
        let mtime_secs = mtime.duration_since(SystemTime::UNIX_EPOCH)?.as_secs() as i64;

        Ok(Self { dir_mtime_secs: mtime_secs, max_timestamp: None, file_count })
    }

    /// Check if project directory has changed (mtime differs)
    pub fn is_stale(&self, path: &Path) -> anyhow::Result<bool> {
        let metadata = fs::metadata(path)?;
        let mtime = metadata.modified()?;
        let current_mtime_secs = mtime.duration_since(SystemTime::UNIX_EPOCH)?.as_secs() as i64;

        Ok(self.dir_mtime_secs != current_mtime_secs)
    }
}
