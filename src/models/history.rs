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

/// Content block types that can appear in message content arrays
///
/// These variants represent different types of content in Claude API messages,
/// matching the Messages API specification. User messages typically contain
/// text/image blocks, while assistant messages may include text, thinking,
/// tool_use, and tool_result blocks.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    /// Plain text content from user or assistant messages
    ///
    /// Most common content type. Used for regular conversation text,
    /// code snippets, explanations, etc.
    Text { text: String },
    /// Extended thinking content from assistant messages
    ///
    /// Contains the assistant's internal reasoning before generating a response.
    /// Only appears in assistant messages when extended thinking is enabled.
    /// Optional signature field for verification/authentication purposes.
    Thinking {
        thinking: String,
        #[serde(default)]
        signature: Option<String>,
    },
    /// Tool invocation request from assistant
    ///
    /// Represents assistant's request to use a tool/function. Contains tool name
    /// and JSON input parameters. Input is stored as serde_json::Value to handle
    /// varying tool schemas.
    ToolUse { id: String, name: String, input: serde_json::Value },
    /// Tool execution result
    ///
    /// Contains output from executing a tool requested via ToolUse. Content is
    /// stored as serde_json::Value since different tools return different formats
    /// (strings, objects, arrays, etc.). is_error indicates execution failure.
    ToolResult {
        tool_use_id: String,
        content: serde_json::Value,
        #[serde(default)]
        is_error: Option<bool>,
    },
    /// Image content (typically in user messages)
    ///
    /// Represents an image attachment. Source contains image data/URL in various
    /// formats (base64, URL, etc.). Optional alt_text provides textual description
    /// for accessibility or when image cannot be displayed.
    Image {
        source: serde_json::Value,
        #[serde(default)]
        alt_text: Option<String>,
    },
}

/// Message content can be either a simple string or an array of content blocks
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    String(String),
    Array(Vec<ContentBlock>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: MessageContent,
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
