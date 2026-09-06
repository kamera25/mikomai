use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AttachmentType {
    Text,
    Image,
    File,
}

impl AttachmentType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Image => "image",
            Self::File => "file",
        }
    }
}

impl std::fmt::Display for AttachmentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Attachment {
    pub name: String,
    #[serde(rename = "type")]
    pub mime_type: AttachmentType,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
}

/// A source supplied by the webview before it is turned into a persisted attachment.
/// Keeping this conversion in Rust means file size/type policy is identical for the
/// file picker, native drag-and-drop and pasted browser data.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AttachmentSource {
    Path {
        path: String,
    },
    Inline {
        name: String,
        content: String,
        #[serde(default)]
        media_type: Option<String>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentRejection {
    pub name: String,
    pub reason: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentPreparation {
    pub attachments: Vec<Attachment>,
    pub rejected: Vec<AttachmentRejection>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Ai,
}

impl MessageRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Ai => "ai",
        }
    }
}

impl std::fmt::Display for MessageRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecutionStatus {
    Running,
    Success,
    Failed,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum UserInputStatus {
    Pending,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct BaseMessage {
    pub role: MessageRole,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(rename = "isToolLoading", skip_serializing_if = "Option::is_none")]
    pub is_tool_loading: Option<bool>,
    #[serde(rename = "isHidden", skip_serializing_if = "Option::is_none")]
    pub is_hidden: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<uuid::Uuid>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "event_type")]
pub enum Message {
    UserInput {
        #[serde(flatten)]
        base: BaseMessage,
        #[serde(skip_serializing_if = "Option::is_none")]
        status: Option<UserInputStatus>,
        #[serde(skip_serializing_if = "Option::is_none")]
        attachments: Option<Vec<Attachment>>,
    },
    ToolExecution {
        #[serde(flatten)]
        base: BaseMessage,
        status: ExecutionStatus,
        #[serde(skip_serializing_if = "Option::is_none")]
        action_name: Option<String>,
        tool_id: crate::mcp::ToolKind,
        summary_text: String,
        raw_data: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        args: Option<serde_json::Value>,
        #[serde(rename = "saved_path", skip_serializing_if = "Option::is_none")]
        saved_path: Option<PathBuf>,
        #[serde(rename = "is_cached", skip_serializing_if = "Option::is_none")]
        is_cached: Option<bool>,
        #[serde(rename = "cache_time", skip_serializing_if = "Option::is_none")]
        cache_time: Option<String>,
        #[serde(rename = "waitingForApproval", skip_serializing_if = "Option::is_none")]
        waiting_for_approval: Option<bool>,
    },
    AgentResponse {
        #[serde(flatten)]
        base: BaseMessage,
        #[serde(skip_serializing_if = "Option::is_none")]
        summary_text: Option<String>,
    },
    SystemMessage {
        #[serde(flatten)]
        base: BaseMessage,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChatSession {
    pub id: uuid::Uuid,
    pub title: String,
    pub messages: Vec<Message>,
    #[serde(rename = "recentIps", skip_serializing_if = "Option::is_none")]
    pub recent_ips: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SummaryItem {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub content: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    pub id: uuid::Uuid,
    pub name: String,
    pub items: Vec<HistoryItem>,
    pub is_open: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum HistoryItem {
    Session(ChatSession),
    Folder(Folder),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum HistoryMutation {
    #[serde(rename_all = "camelCase")]
    CreateSession {
        title: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    CreateFolder {
        name: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    RenameSession {
        session_id: uuid::Uuid,
        title: String,
    },
    #[serde(rename_all = "camelCase")]
    DeleteSession {
        session_id: uuid::Uuid,
    },
    #[serde(rename_all = "camelCase")]
    ToggleFolder {
        folder_id: uuid::Uuid,
    },
    #[serde(rename_all = "camelCase")]
    UpdateSessionMessages {
        session_id: uuid::Uuid,
        messages: Vec<Message>,
    },
    #[serde(rename_all = "camelCase")]
    UpdateSessionRecentIps {
        session_id: uuid::Uuid,
        recent_ips: Vec<String>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HistorySnapshot {
    pub history: Vec<HistoryItem>,
    pub active_session_id: String,
}

#[derive(Debug, thiserror::Error)]
pub enum HistoryError {
    #[error("File I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization/Deserialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Database error: {0}")]
    Database(String),
}

