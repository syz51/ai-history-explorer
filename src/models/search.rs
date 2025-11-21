use std::path::PathBuf;

use chrono::{DateTime, Utc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryType {
    UserPrompt,
    AgentMessage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchEntry {
    pub entry_type: EntryType,
    pub display_text: String,
    pub timestamp: DateTime<Utc>,
    pub project_path: Option<PathBuf>,
    pub session_id: String,
}
