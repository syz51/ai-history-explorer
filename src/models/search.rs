use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntryType {
    UserPrompt,
    AgentMessage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchEntry {
    pub entry_type: EntryType,
    pub display_text: String,
    pub timestamp: DateTime<Utc>,
    pub project_path: Option<PathBuf>,
    pub session_id: String,
}
