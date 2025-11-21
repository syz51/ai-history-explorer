use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub display: String,
    #[serde(deserialize_with = "crate::parsers::deserializers::deserialize_timestamp")]
    pub timestamp: DateTime<Utc>,
    #[serde(default)]
    pub project: Option<String>,
    #[serde(
        rename = "sessionId",
        deserialize_with = "crate::parsers::deserializers::deserialize_session_id"
    )]
    pub session_id: String,
    #[serde(default, skip)]
    pub pasted_contents: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageContent {
    #[serde(rename = "type")]
    pub content_type: String,
    #[serde(default)]
    pub text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: Vec<MessageContent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationEntry {
    #[serde(rename = "type")]
    pub entry_type: String,
    pub message: Message,
    #[serde(deserialize_with = "crate::parsers::deserializers::deserialize_timestamp")]
    pub timestamp: DateTime<Utc>,
    #[serde(
        rename = "sessionId",
        deserialize_with = "crate::parsers::deserializers::deserialize_session_id"
    )]
    pub session_id: String,
    pub uuid: String,
    #[serde(default)]
    pub parent_uuid: Option<String>,
    #[serde(default)]
    pub is_sidechain: Option<bool>,
}
