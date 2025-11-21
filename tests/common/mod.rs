//! Shared test utilities for integration tests
#![allow(dead_code)]

use std::fs;
use std::io::Write;
use std::path::Path;

use tempfile::TempDir;

/// Builder for creating test .claude directory structures
pub struct ClaudeDirBuilder {
    temp_dir: TempDir,
}

impl ClaudeDirBuilder {
    /// Create a new builder with an empty .claude directory
    pub fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        Self { temp_dir }
    }

    /// Get the path to the .claude directory
    pub fn path(&self) -> &Path {
        self.temp_dir.path()
    }

    /// Add a history.jsonl file with the given content
    pub fn with_history(self, content: &str) -> Self {
        let history_path = self.temp_dir.path().join("history.jsonl");
        let mut file = fs::File::create(history_path).expect("Failed to create history.jsonl");
        file.write_all(content.as_bytes()).expect("Failed to write history.jsonl");
        self
    }

    /// Add history entries programmatically
    pub fn with_history_entries(self, entries: &[HistoryEntryBuilder]) -> Self {
        let content = entries.iter().map(|e| e.to_json()).collect::<Vec<_>>().join("\n");
        self.with_history(&content)
    }

    /// Add a project directory with the given encoded name and agent files
    pub fn with_project(self, encoded_name: &str, agent_files: &[AgentFileBuilder]) -> Self {
        let projects_dir = self.temp_dir.path().join("projects");
        fs::create_dir_all(&projects_dir).expect("Failed to create projects dir");

        let project_dir = projects_dir.join(encoded_name);
        fs::create_dir(&project_dir).expect("Failed to create project dir");

        for agent_file in agent_files {
            agent_file.create_in(&project_dir);
        }

        self
    }

    /// Build and return the temp directory (consumes self)
    pub fn build(self) -> TempDir {
        self.temp_dir
    }
}

impl Default for ClaudeDirBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for history.jsonl entries
pub struct HistoryEntryBuilder {
    display: String,
    timestamp: i64,
    session_id: String,
    project: Option<String>,
}

impl HistoryEntryBuilder {
    /// Create a new history entry with default values
    pub fn new() -> Self {
        Self {
            display: "Test entry".to_string(),
            timestamp: 1234567890,
            session_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            project: None,
        }
    }

    /// Set the display text
    pub fn display(mut self, display: &str) -> Self {
        self.display = display.to_string();
        self
    }

    /// Set the timestamp
    pub fn timestamp(mut self, timestamp: i64) -> Self {
        self.timestamp = timestamp;
        self
    }

    /// Set the session ID (expects a valid UUID string)
    pub fn session_id(mut self, session_id: &str) -> Self {
        self.session_id = session_id.to_string();
        self
    }

    /// Set the project path
    pub fn project(mut self, project: &str) -> Self {
        self.project = Some(project.to_string());
        self
    }

    /// Convert to JSON string
    pub fn to_json(&self) -> String {
        let project_field =
            self.project.as_ref().map(|p| format!(r#","project":"{}""#, p)).unwrap_or_default();

        format!(
            r#"{{"display":"{}","timestamp":{},"sessionId":"{}"{}}}"#,
            self.display, self.timestamp, self.session_id, project_field
        )
    }
}

impl Default for HistoryEntryBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for agent conversation files
pub struct AgentFileBuilder {
    filename: String,
    entries: Vec<ConversationEntryBuilder>,
}

impl AgentFileBuilder {
    /// Create a new agent file with the given filename
    pub fn new(filename: &str) -> Self {
        Self { filename: filename.to_string(), entries: Vec::new() }
    }

    /// Add a conversation entry
    pub fn with_entry(mut self, entry: ConversationEntryBuilder) -> Self {
        self.entries.push(entry);
        self
    }

    /// Create the file in the given directory
    pub fn create_in(&self, dir: &Path) {
        let file_path = dir.join(&self.filename);
        let mut file = fs::File::create(file_path).expect("Failed to create agent file");

        let content = self.entries.iter().map(|e| e.to_json()).collect::<Vec<_>>().join("\n");

        file.write_all(content.as_bytes()).expect("Failed to write agent file");
    }
}

/// Builder for conversation entries in agent files
pub struct ConversationEntryBuilder {
    entry_type: String,
    role: String,
    content: ContentType,
    timestamp: i64,
    session_id: String,
    uuid: String,
}

/// Content type for conversation entries
enum ContentType {
    Text(String),
    ContentBlocks(Vec<String>),
}

impl ConversationEntryBuilder {
    /// Create a new user message
    pub fn user() -> Self {
        Self {
            entry_type: "user".to_string(),
            role: "user".to_string(),
            content: ContentType::Text("Test message".to_string()),
            timestamp: 1234567890,
            session_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            uuid: "550e8400-e29b-41d4-a716-446655440001".to_string(),
        }
    }

    /// Create a new assistant message
    pub fn assistant() -> Self {
        Self {
            entry_type: "assistant".to_string(),
            role: "assistant".to_string(),
            content: ContentType::Text("Test response".to_string()),
            timestamp: 1234567891,
            session_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            uuid: "550e8400-e29b-41d4-a716-446655440002".to_string(),
        }
    }

    /// Set the message text (simple text content)
    pub fn text(mut self, text: &str) -> Self {
        self.content = ContentType::Text(text.to_string());
        self
    }

    /// Set content blocks (advanced content with thinking, tool_use, etc.)
    pub fn content_blocks(mut self, blocks: Vec<String>) -> Self {
        self.content = ContentType::ContentBlocks(blocks);
        self
    }

    /// Add a thinking block
    pub fn thinking_block(text: &str) -> String {
        format!(r#"{{"type":"thinking","thinking":"{}"}}"#, text)
    }

    /// Add a tool_use block
    pub fn tool_use_block(id: &str, name: &str, input_json: &str) -> String {
        format!(r#"{{"type":"tool_use","id":"{}","name":"{}","input":{}}}"#, id, name, input_json)
    }

    /// Add a tool_result block
    pub fn tool_result_block(tool_use_id: &str, content_json: &str, is_error: bool) -> String {
        format!(
            r#"{{"type":"tool_result","tool_use_id":"{}","content":{},"is_error":{}}}"#,
            tool_use_id, content_json, is_error
        )
    }

    /// Add an image block
    pub fn image_block(source_json: &str, alt_text: Option<&str>) -> String {
        if let Some(alt) = alt_text {
            format!(r#"{{"type":"image","source":{},"alt_text":"{}"}}"#, source_json, alt)
        } else {
            format!(r#"{{"type":"image","source":{}}}"#, source_json)
        }
    }

    /// Add a text block (for use in content blocks array)
    pub fn text_block(text: &str) -> String {
        format!(r#"{{"type":"text","text":"{}"}}"#, text)
    }

    /// Set the timestamp
    pub fn timestamp(mut self, timestamp: i64) -> Self {
        self.timestamp = timestamp;
        self
    }

    /// Set the session ID
    pub fn session_id(mut self, session_id: &str) -> Self {
        self.session_id = session_id.to_string();
        self
    }

    /// Set the UUID
    pub fn uuid(mut self, uuid: &str) -> Self {
        self.uuid = uuid.to_string();
        self
    }

    /// Convert to JSON string
    pub fn to_json(&self) -> String {
        let content_json = match &self.content {
            ContentType::Text(text) => {
                format!(r#"[{{"type":"text","text":"{}"}}]"#, text)
            }
            ContentType::ContentBlocks(blocks) => {
                format!("[{}]", blocks.join(","))
            }
        };

        format!(
            r#"{{"type":"{}","message":{{"role":"{}","content":{}}},"timestamp":{},"sessionId":"{}","uuid":"{}"}}"#,
            self.entry_type, self.role, content_json, self.timestamp, self.session_id, self.uuid
        )
    }
}

/// Helper to create a minimal valid .claude directory
pub fn minimal_claude_dir() -> TempDir {
    ClaudeDirBuilder::new().with_history("").build()
}

/// Helper to create a realistic .claude directory with sample data
pub fn realistic_claude_dir() -> TempDir {
    ClaudeDirBuilder::new()
        .with_history_entries(&[
            HistoryEntryBuilder::new()
                .display("First prompt")
                .timestamp(1234567890)
                .session_id("550e8400-e29b-41d4-a716-446655440000"),
            HistoryEntryBuilder::new()
                .display("Second prompt")
                .timestamp(1234567891)
                .session_id("550e8400-e29b-41d4-a716-446655440001"),
            HistoryEntryBuilder::new()
                .display("Third prompt")
                .timestamp(1234567892)
                .session_id("550e8400-e29b-41d4-a716-446655440002"),
        ])
        .with_project(
            "-Users%2Ftest%2Fproject1",
            &[AgentFileBuilder::new("agent-1.jsonl")
                .with_entry(ConversationEntryBuilder::user().text("Hello from project"))
                .with_entry(ConversationEntryBuilder::assistant().text("Hi there"))],
        )
        .with_project(
            "-Users%2Ftest%2Fproject2",
            &[AgentFileBuilder::new("agent-2.jsonl")
                .with_entry(ConversationEntryBuilder::user().text("Another project"))],
        )
        .build()
}
