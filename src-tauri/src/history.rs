use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::Manager;
use crate::graph::SurrealDbState;
use crate::history_store;

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

use crate::error::TauriError;

#[derive(Debug, thiserror::Error)]
pub enum HistoryError {
    #[error("File I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization/Deserialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Database error: {0}")]
    Database(String),
}

pub fn sanitize_history_items(items: &mut Vec<HistoryItem>) -> bool {
    let mut modified = false;
    for item in items.iter_mut() {
        match item {
            HistoryItem::Session(ref mut session) => {
                for msg in session.messages.iter_mut() {
                    match msg {
                        Message::ToolExecution {
                            ref mut status,
                            ref mut base,
                            ref mut summary_text,
                            ref mut raw_data,
                            ref action_name,
                            ref tool_id,
                            ..
                        } => {
                            if *status == ExecutionStatus::Running
                                || base.is_tool_loading == Some(true)
                            {
                                *status = ExecutionStatus::Failed;
                                base.is_tool_loading = Some(false);
                                let label =
                                    action_name.as_deref().unwrap_or_else(|| tool_id.label());
                                *summary_text = format!("{} 失敗", label);
                                if raw_data.is_none()
                                    || raw_data.as_deref().unwrap_or("").trim().is_empty()
                                {
                                    *raw_data = Some(
                                        "アプリケーションが終了したため、MCPの実行が失敗しました。"
                                            .to_string(),
                                    );
                                }
                                modified = true;
                            }
                        }
                        Message::AgentResponse { ref mut base, .. }
                        | Message::UserInput { ref mut base, .. }
                        | Message::SystemMessage { ref mut base } => {
                            if base.is_tool_loading == Some(true) {
                                base.is_tool_loading = Some(false);
                                modified = true;
                            }
                        }
                    }
                }
            }
            HistoryItem::Folder(ref mut folder) => {
                if sanitize_history_items(&mut folder.items) {
                    modified = true;
                }
            }
        }
    }
    modified
}

async fn save_history_to_store(
    db: &SurrealDbState,
    history: &[HistoryItem],
) -> Result<(), TauriError> {
    let value = serde_json::to_value(history)?;
    history_store::save(db, value)
        .await
        .map_err(HistoryError::Database)?;
    Ok(())
}

async fn load_history_from_store(
    db: &SurrealDbState,
) -> Result<Vec<HistoryItem>, TauriError> {
    let stored = history_store::load(db)
        .await
        .map_err(HistoryError::Database)?;
    let mut history = match stored {
        Some(value) => serde_json::from_value(value)?,
        None => vec![],
    };
    if sanitize_history_items(&mut history) {
        save_history_to_store(db, &history).await?;
    }
    Ok(history)
}

#[tauri::command]
pub async fn cleanup_running_history_on_exit(app: tauri::AppHandle) -> Result<(), TauriError> {
    let db = app.state::<SurrealDbState>();
    let mut history = load_history_from_store(&db).await?;
    if sanitize_history_items(&mut history) {
        save_history_to_store(&db, &history).await?;
    }
    Ok(())
}

#[tauri::command]
pub async fn load_history(
    db: tauri::State<'_, SurrealDbState>,
) -> Result<Vec<HistoryItem>, TauriError> {
    load_history_from_store(&db).await
}

#[tauri::command]
pub async fn save_history(
    db: tauri::State<'_, SurrealDbState>,
    history: Vec<HistoryItem>,
) -> Result<(), TauriError> {
    save_history_to_store(&db, &history).await
}

fn first_session_id(items: &[HistoryItem]) -> Option<uuid::Uuid> {
    for item in items {
        match item {
            HistoryItem::Session(session) => return Some(session.id),
            HistoryItem::Folder(folder) => {
                if let Some(id) = first_session_id(&folder.items) {
                    return Some(id);
                }
            }
        }
    }
    None
}

fn mutate_items(items: &mut Vec<HistoryItem>, mutation: &HistoryMutation) -> bool {
    match mutation {
        HistoryMutation::RenameSession { session_id, title } => {
            items.iter_mut().any(|item| match item {
                HistoryItem::Session(session) if session.id == *session_id => {
                    session.title = title.trim().to_string();
                    true
                }
                HistoryItem::Folder(folder) => mutate_items(&mut folder.items, mutation),
                _ => false,
            })
        }
        HistoryMutation::DeleteSession { session_id } => {
            let before = items.len();
            items.retain(
                |item| !matches!(item, HistoryItem::Session(session) if session.id == *session_id),
            );
            if items.len() != before {
                return true;
            }
            items.iter_mut().any(|item| match item {
                HistoryItem::Folder(folder) => mutate_items(&mut folder.items, mutation),
                _ => false,
            })
        }
        HistoryMutation::ToggleFolder { folder_id } => items.iter_mut().any(|item| match item {
            HistoryItem::Folder(folder) if folder.id == *folder_id => {
                folder.is_open = !folder.is_open;
                true
            }
            HistoryItem::Folder(folder) => mutate_items(&mut folder.items, mutation),
            _ => false,
        }),
        HistoryMutation::UpdateSessionMessages {
            session_id,
            messages,
        } => items.iter_mut().any(|item| match item {
            HistoryItem::Session(session) if session.id == *session_id => {
                session.messages = messages.clone();
                true
            }
            HistoryItem::Folder(folder) => mutate_items(&mut folder.items, mutation),
            _ => false,
        }),
        HistoryMutation::UpdateSessionRecentIps {
            session_id,
            recent_ips,
        } => items.iter_mut().any(|item| match item {
            HistoryItem::Session(session) if session.id == *session_id => {
                session.recent_ips = Some(recent_ips.clone());
                true
            }
            HistoryItem::Folder(folder) => mutate_items(&mut folder.items, mutation),
            _ => false,
        }),
        HistoryMutation::CreateSession { .. } | HistoryMutation::CreateFolder { .. } => false,
    }
}

/// Applies a single tree mutation and saves only a valid history snapshot.  The UI
/// still owns selection and rendering, while identifiers and persisted tree
/// invariants are owned by the backend.
#[tauri::command]
pub async fn mutate_history(
    db: tauri::State<'_, SurrealDbState>,
    mutation: HistoryMutation,
) -> Result<HistorySnapshot, TauriError> {
    let mut history = load_history_from_store(&db).await?;
    match &mutation {
        HistoryMutation::CreateSession { title } => {
            history.push(HistoryItem::Session(ChatSession {
                id: uuid::Uuid::new_v4(),
                title: title
                    .clone()
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| "New Session".to_string()),
                messages: vec![],
                recent_ips: None,
            }))
        }
        HistoryMutation::CreateFolder { name } => history.push(HistoryItem::Folder(Folder {
            id: uuid::Uuid::new_v4(),
            name: name
                .clone()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "New Folder".to_string()),
            items: vec![],
            is_open: true,
        })),
        _ => {
            mutate_items(&mut history, &mutation);
        }
    }
    let active_session_id = first_session_id(&history)
        .map(|id| id.to_string())
        .unwrap_or_default();
    save_history_to_store(&db, &history).await?;
    Ok(HistorySnapshot {
        history,
        active_session_id,
    })
}

#[tauri::command]
pub async fn initialize_history(
    db: tauri::State<'_, SurrealDbState>,
) -> Result<HistorySnapshot, TauriError> {
    let history = load_history_from_store(&db).await?;
    Ok(HistorySnapshot {
        active_session_id: first_session_id(&history)
            .map(|id| id.to_string())
            .unwrap_or_default(),
        history,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_serialization() {
        let msg = Message::UserInput {
            base: BaseMessage {
                role: MessageRole::User,
                content: "Hello".to_string(),
                timestamp: None,
                is_tool_loading: None,
                is_hidden: None,
                task_id: None,
            },
            status: None,
            attachments: None,
        };
        let serialized = serde_json::to_string(&msg).unwrap();
        assert!(serialized.contains(r#""role":"user""#));
        assert!(serialized.contains(r#""content":"Hello""#));
        assert!(serialized.contains(r#""event_type":"UserInput""#));
    }

    #[test]
    fn test_history_item_session_serialization() {
        let session_id = uuid::Uuid::new_v4();
        let session = ChatSession {
            id: session_id,
            title: "Test Session".to_string(),
            messages: vec![Message::UserInput {
                base: BaseMessage {
                    role: MessageRole::User,
                    content: "Hi".to_string(),
                    timestamp: None,
                    is_tool_loading: None,
                    is_hidden: None,
                    task_id: None,
                },
                status: None,
                attachments: None,
            }],
            recent_ips: None,
        };
        let item = HistoryItem::Session(session);
        let serialized = serde_json::to_string(&item).unwrap();
        assert!(serialized.contains(r#""type":"session""#));
        assert!(serialized.contains(&format!(r#""id":"{}""#, session_id)));
    }

    #[test]
    fn test_history_item_folder_serialization() {
        let folder_id = uuid::Uuid::new_v4();
        let folder = Folder {
            id: folder_id,
            name: "Test Folder".to_string(),
            items: vec![],
            is_open: true,
        };
        let item = HistoryItem::Folder(folder);
        let serialized = serde_json::to_string(&item).unwrap();
        assert!(serialized.contains(r#""type":"folder""#));
        assert!(serialized.contains(r#""name":"Test Folder""#));
        assert!(serialized.contains(r#""isOpen":true"#));
        assert!(serialized.contains(&format!(r#""id":"{}""#, folder_id)));
    }

    #[test]
    fn test_summary_item_serialization() {
        let summary = SummaryItem {
            timestamp: "2023-10-27T10:00:00Z"
                .parse::<chrono::DateTime<chrono::Utc>>()
                .unwrap(),
            content: "Test summary".to_string(),
        };
        let serialized = serde_json::to_string(&summary).unwrap();
        assert!(serialized.contains(r#""timestamp":"2023-10-27T10:00:00Z""#));
        assert!(serialized.contains(r#""content":"Test summary""#));
    }

    #[test]
    fn test_sanitize_history_items_running_mcp() {
        let session_id = uuid::Uuid::new_v4();
        let mut history = vec![HistoryItem::Session(ChatSession {
            id: session_id,
            title: "Test Session".to_string(),
            messages: vec![Message::ToolExecution {
                base: BaseMessage {
                    role: MessageRole::Ai,
                    content: "".to_string(),
                    timestamp: None,
                    is_tool_loading: Some(true),
                    is_hidden: None,
                    task_id: Some(uuid::Uuid::new_v4()),
                },
                status: ExecutionStatus::Running,
                action_name: Some("インターフェース選択".to_string()),
                tool_id: crate::mcp::ToolKind::AskInterfaceChoice,
                summary_text: "ask_interface_choice を実行中...".to_string(),
                raw_data: None,
                args: None,
                saved_path: None,
                is_cached: None,
                cache_time: None,
                waiting_for_approval: None,
            }],
            recent_ips: None,
        })];

        let modified = sanitize_history_items(&mut history);
        assert!(modified);

        if let HistoryItem::Session(session) = &history[0] {
            if let Message::ToolExecution {
                status,
                base,
                summary_text,
                raw_data,
                ..
            } = &session.messages[0]
            {
                assert_eq!(*status, ExecutionStatus::Failed);
                assert_eq!(base.is_tool_loading, Some(false));
                assert_eq!(summary_text, "インターフェース選択 失敗");
                assert_eq!(
                    raw_data.as_deref(),
                    Some("アプリケーションが終了したため、MCPの実行が失敗しました。")
                );
            } else {
                panic!("Expected ToolExecution message");
            }
        } else {
            panic!("Expected Session item");
        }
    }

    #[test]
    fn test_attachment_serialization_with_path() {
        let att = Attachment {
            name: "firmware.bin".to_string(),
            mime_type: AttachmentType::File,
            content: "[ファイル: firmware.bin (サイズ: 1.2 MB)]".to_string(),
            path: Some(PathBuf::from("/tmp/firmware.bin")),
        };
        let json = serde_json::to_string(&att).unwrap();
        assert!(json.contains(r#""name":"firmware.bin""#));
        assert!(json.contains(r#""type":"file""#));
        assert!(json.contains(r#""path":"/tmp/firmware.bin""#));

        let deserialized: Attachment = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "firmware.bin");
        assert_eq!(deserialized.mime_type, AttachmentType::File);
        assert_eq!(deserialized.path, Some(PathBuf::from("/tmp/firmware.bin")));
    }

    #[test]
    fn test_read_files_as_attachments_text_and_binary() {
        use std::io::Write;
        let temp_dir = std::env::temp_dir();
        let text_path = temp_dir.join("test_text_file.txt");
        let bin_path = temp_dir.join("test_bin_file.bin");

        let mut f_text = fs::File::create(&text_path).unwrap();
        writeln!(f_text, "hostname Switch1\ninterface GigabitEthernet0/1").unwrap();

        let mut f_bin = fs::File::create(&bin_path).unwrap();
        // Write invalid utf-8 binary bytes
        f_bin
            .write_all(&[0xDE, 0xAD, 0xBE, 0xEF, 0xFF, 0xFE, 0x00, 0x01])
            .unwrap();

        let paths = vec![
            text_path.to_string_lossy().to_string(),
            bin_path.to_string_lossy().to_string(),
        ];

        let atts = prepare_attachments_for_sources(
            paths.into_iter().map(|path| AttachmentSource::Path { path }).collect(),
            true,
        ).attachments;
        assert_eq!(atts.len(), 2);

        let text_att = &atts[0];
        assert_eq!(text_att.name, "test_text_file.txt");
        assert_eq!(text_att.mime_type, AttachmentType::Text);
        assert!(text_att.content.contains("hostname Switch1"));
        assert_eq!(text_att.path, Some(text_path.clone()));

        let bin_att = &atts[1];
        assert_eq!(bin_att.name, "test_bin_file.bin");
        assert_eq!(bin_att.mime_type, AttachmentType::File);
        assert!(bin_att.content.contains("test_bin_file.bin"));
        assert_eq!(bin_att.path, Some(bin_path.clone()));

        let _ = fs::remove_file(text_path);
        let _ = fs::remove_file(bin_path);
    }

    #[test]
    fn prepares_inline_attachments_and_rejects_duplicates_and_large_content() {
        let result = prepare_attachments_for_sources(vec![
            AttachmentSource::Inline {
                name: "note.txt".to_string(),
                content: "router configuration".to_string(),
                media_type: Some("text/plain".to_string()),
            },
            AttachmentSource::Inline {
                name: "note.txt".to_string(),
                content: "duplicate".to_string(),
                media_type: Some("text/plain".to_string()),
            },
            AttachmentSource::Inline {
                name: "large.txt".to_string(),
                content: "x".repeat(MAX_INLINE_ATTACHMENT_SIZE + 1),
                media_type: Some("text/plain".to_string()),
            },
        ], true);
        assert_eq!(result.attachments.len(), 1);
        assert_eq!(result.attachments[0].mime_type, AttachmentType::Text);
        assert_eq!(result.rejected.len(), 2);
    }

    #[test]
    fn rejects_images_when_vision_is_not_ready() {
        let result = prepare_attachments_for_sources(
            vec![AttachmentSource::Inline {
                name: "diagram.png".to_string(),
                content: "data:image/png;base64,AA==".to_string(),
                media_type: Some("image/png".to_string()),
            }],
            false,
        );
        assert!(result.attachments.is_empty());
        assert_eq!(result.rejected.len(), 1);
        assert!(result.rejected[0].reason.contains("Vision"));
    }

    #[test]
    fn rejects_oversized_inline_attachments() {
        let result = prepare_attachments_for_sources(
            vec![AttachmentSource::Inline {
                name: "large.txt".to_string(),
                content: "x".repeat(MAX_INLINE_ATTACHMENT_SIZE + 1),
                media_type: Some("text/plain".to_string()),
            }],
            true,
        );
        assert!(result.attachments.is_empty());
        assert_eq!(result.rejected.len(), 1);
        assert!(result.rejected[0].reason.contains("512 KB"));
    }

    #[test]
    fn deleting_the_last_session_leaves_an_empty_history() {
        let id = uuid::Uuid::new_v4();
        let mut items = vec![HistoryItem::Session(ChatSession {
            id,
            title: "Session".to_string(),
            messages: vec![],
            recent_ips: None,
        })];
        assert!(mutate_items(&mut items, &HistoryMutation::DeleteSession { session_id: id }));
        assert!(items.is_empty());
        assert!(first_session_id(&items).is_none());
    }

    #[test]
    fn test_mutation_deserialization() {
        let json_data = r#"{
            "type": "updateSessionMessages",
            "sessionId": "a0000000-0000-0000-0000-000000000001",
            "messages": [
                {
                    "role": "user",
                    "content": "fitelnetのVLAN設定を教えて",
                    "event_type": "UserInput",
                    "task_id": "a0000000-0000-0000-0000-000000000002"
                },
                {
                    "role": "ai",
                    "content": "VLAN設定には、アクセスVLANとトランクVLANの2種類があります。",
                    "event_type": "AgentResponse",
                    "task_id": "a0000000-0000-0000-0000-000000000003",
                    "summary_text": "エージェントによる解析を開始"
                },
                {
                    "role": "ai",
                    "content": "",
                    "event_type": "ToolExecution",
                    "task_id": "a0000000-0000-0000-0000-000000000004",
                    "status": "Success",
                    "action_name": "設定取得",
                    "tool_id": "fetch_config",
                    "summary_text": "設定取得 完了",
                    "raw_data": "interface GigaEthernet 1/1",
                    "waitingForApproval": false
                }
            ]
        }"#;
        let mutation: Result<HistoryMutation, _> = serde_json::from_str(json_data);
        assert!(mutation.is_ok(), "Failed to deserialize mutation: {:?}", mutation.err());
    }

    #[tokio::test]
    async fn test_full_surrealdb_history_lifecycle() {
        let temp_dir = std::env::temp_dir().join(format!("mikomai-full-test-{}", uuid::Uuid::new_v4()));
        let state = SurrealDbState::initialize_at(&temp_dir).await.unwrap();
        history_store::initialize(&state).await.unwrap();

        // 1. Initial state is empty
        let initial = load_history_from_store(&state).await.unwrap();
        assert!(initial.is_empty());

        // 2. Create session and save messages
        let session_id = uuid::Uuid::new_v4();
        let user_task_id = uuid::Uuid::new_v4();
        let ai_task_id = uuid::Uuid::new_v4();
        let user_msg = Message::UserInput {
            base: BaseMessage {
                role: MessageRole::User,
                content: "fitelnetのVLAN設定を教えて".to_string(),
                timestamp: Some(chrono::Utc::now()),
                is_tool_loading: None,
                is_hidden: None,
                task_id: Some(user_task_id),
            },
            status: None,
            attachments: None,
        };
        let ai_msg = Message::AgentResponse {
            base: BaseMessage {
                role: MessageRole::Ai,
                content: "VLAN設定の手順です".to_string(),
                timestamp: Some(chrono::Utc::now()),
                is_tool_loading: Some(false),
                is_hidden: Some(false),
                task_id: Some(ai_task_id),
            },
            summary_text: Some("解析完了".to_string()),
        };

        let session = ChatSession {
            id: session_id,
            title: "Fitelnet VLAN設定".to_string(),
            messages: vec![user_msg.clone(), ai_msg.clone()],
            recent_ips: Some(vec!["192.168.1.1".to_string()]),
        };
        let history = vec![HistoryItem::Session(session)];

        save_history_to_store(&state, &history).await.unwrap();

        // 3. Load back from SurrealDB and verify contents
        let loaded = load_history_from_store(&state).await.unwrap();
        assert_eq!(loaded.len(), 1);
        if let HistoryItem::Session(s) = &loaded[0] {
            assert_eq!(s.id, session_id);
            assert_eq!(s.title, "Fitelnet VLAN設定");
            assert_eq!(s.recent_ips, Some(vec!["192.168.1.1".to_string()]));
            assert_eq!(s.messages.len(), 2);
            assert_eq!(s.messages[0], user_msg);
            assert_eq!(s.messages[1], ai_msg);
        } else {
            panic!("Expected session");
        }

        // 4. Test mutation serialization via JSON (matching frontend Tauri invoke)
        let rename_json = format!(r#"{{
            "type": "renameSession",
            "sessionId": "{session_id}",
            "title": "更新されたタイトル"
        }}"#);
        let mutation: HistoryMutation = serde_json::from_str(&rename_json).unwrap();
        let mut mutated_history = loaded;
        let mutated = mutate_items(&mut mutated_history, &mutation);
        assert!(mutated);
        save_history_to_store(&state, &mutated_history).await.unwrap();

        // 5. Simulate reopening SurrealDB (app restart)
        drop(state);
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        let reopened_state = SurrealDbState::initialize_at(&temp_dir).await.unwrap();
        let reloaded = load_history_from_store(&reopened_state).await.unwrap();
        assert_eq!(reloaded.len(), 1);
        if let HistoryItem::Session(s) = &reloaded[0] {
            assert_eq!(s.title, "更新されたタイトル");
            assert_eq!(s.messages.len(), 2);
        } else {
            panic!("Expected session");
        }

        drop(reopened_state);
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_invalid_task_id_fails_deserialization() {
        let json_data = r#"{
            "role": "user",
            "content": "test",
            "event_type": "UserInput",
            "task_id": "not-a-valid-uuid"
        }"#;
        let result: Result<Message, _> = serde_json::from_str(json_data);
        assert!(result.is_err(), "Expected error when task_id is not a valid UUID, but got: {:?}", result);
    }
}

fn get_summaries_path(app: &tauri::AppHandle) -> PathBuf {
    let path = app
        .path()
        .app_data_dir()
        .expect("Failed to get app data dir");
    if !path.exists() {
        let _ = fs::create_dir_all(&path);
    }
    path.join("summaries.json")
}

#[tauri::command]
pub fn load_summaries(app: tauri::AppHandle) -> Result<Vec<SummaryItem>, TauriError> {
    let path = get_summaries_path(&app);
    if !path.exists() {
        return Ok(vec![]);
    }
    let data = fs::read_to_string(path)?;
    let summaries: Vec<SummaryItem> = serde_json::from_str(&data)?;
    Ok(summaries)
}

#[tauri::command]
pub fn save_summary(app: tauri::AppHandle, summary: SummaryItem) -> Result<(), TauriError> {
    let mut summaries = load_summaries(app.clone()).unwrap_or_default();
    summaries.push(summary);

    // Keep only the last 100 summaries to prevent the file from growing indefinitely
    if summaries.len() > 100 {
        let skip = summaries.len() - 100;
        summaries = summaries.into_iter().skip(skip).collect();
    }

    let path = get_summaries_path(&app);
    let data = serde_json::to_string_pretty(&summaries)?;
    fs::write(path, data)?;
    Ok(())
}

use base64::{engine::general_purpose, Engine as _};

const MAX_TEXT_ATTACHMENT_SIZE: u64 = 512 * 1024;
const MAX_INLINE_ATTACHMENT_SIZE: usize = 512 * 1024;
const MAX_IMAGE_ATTACHMENT_SIZE: u64 = 10 * 1024 * 1024;

fn attachment_from_path(path_str: String) -> Result<Attachment, AttachmentRejection> {
    let path = std::path::Path::new(&path_str);
    if !path.exists() || !path.is_file() {
        return Err(AttachmentRejection {
            name: path_str,
            reason: "ファイルが見つかりません".to_string(),
        });
    }

    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file")
        .to_string();

    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let is_image = matches!(
        extension.as_str(),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "svg"
    );

    if is_image {
        let file_size = fs::metadata(&path).map(|metadata| metadata.len()).unwrap_or(0);
        if file_size > MAX_IMAGE_ATTACHMENT_SIZE {
            return Err(AttachmentRejection {
                name: file_name,
                reason: "画像ファイルが大きすぎます (最大 10 MB)".to_string(),
            });
        }
        if let Ok(bytes) = fs::read(&path) {
            let mime = match extension.as_str() {
                "png" => "image/png",
                "jpg" | "jpeg" => "image/jpeg",
                "gif" => "image/gif",
                "webp" => "image/webp",
                "bmp" => "image/bmp",
                "svg" => "image/svg+xml",
                _ => "image/png",
            };
            let b64 = general_purpose::STANDARD.encode(&bytes);
            let data_url = format!("data:{};base64,{}", mime, b64);
            return Ok(Attachment {
                name: file_name,
                mime_type: AttachmentType::Image,
                content: data_url,
                path: Some(PathBuf::from(path_str)),
            });
        }
        return Err(AttachmentRejection {
            name: file_name,
            reason: "画像ファイルを読み込めません".to_string(),
        });
    } else {
        let file_size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);

        if file_size <= MAX_TEXT_ATTACHMENT_SIZE {
            if let Ok(text) = fs::read_to_string(&path) {
                return Ok(Attachment {
                    name: file_name,
                    mime_type: AttachmentType::Text,
                    content: text,
                    path: Some(PathBuf::from(path_str)),
                });
            }
        }

        let size_desc = if file_size < 1024 {
            format!("{} B", file_size)
        } else if file_size < 1024 * 1024 {
            format!("{:.1} KB", file_size as f64 / 1024.0)
        } else {
            format!("{:.1} MB", file_size as f64 / (1024.0 * 1024.0))
        };

        return Ok(Attachment {
            name: file_name.clone(),
            mime_type: AttachmentType::File,
            content: format!("[ファイル: {} (サイズ: {})]", file_name, size_desc),
            path: Some(PathBuf::from(path_str)),
        });
    }
}

fn attachment_from_inline(
    name: String,
    content: String,
    media_type: Option<String>,
) -> Result<Attachment, AttachmentRejection> {
    if name.trim().is_empty() {
        return Err(AttachmentRejection {
            name,
            reason: "ファイル名が必要です".to_string(),
        });
    }
    if content.len() > MAX_INLINE_ATTACHMENT_SIZE {
        return Err(AttachmentRejection {
            name,
            reason: "添付内容が大きすぎます (最大 512 KB)".to_string(),
        });
    }
    let is_image = media_type
        .as_deref()
        .is_some_and(|value| value.starts_with("image/"))
        || content.starts_with("data:image/");
    Ok(Attachment {
        name,
        mime_type: if is_image {
            AttachmentType::Image
        } else {
            AttachmentType::Text
        },
        content,
        path: None,
    })
}

fn prepare_attachments_for_sources(sources: Vec<AttachmentSource>, vision_ready: bool) -> AttachmentPreparation {
    let mut attachments = Vec::new();
    let mut rejected = Vec::new();
    let mut names = std::collections::HashSet::new();
    for source in sources {
        let result = match source {
            AttachmentSource::Path { path } => attachment_from_path(path),
            AttachmentSource::Inline {
                name,
                content,
                media_type,
            } => attachment_from_inline(name, content, media_type),
        };
        match result {
            Ok(attachment) if attachment.mime_type == AttachmentType::Image && !vision_ready => {
                rejected.push(AttachmentRejection { name: attachment.name, reason: "画像添付には Vision モデルの設定が必要です".to_string() });
            }
            Ok(attachment) if names.insert(attachment.name.clone()) => attachments.push(attachment),
            Ok(attachment) => rejected.push(AttachmentRejection {
                name: attachment.name,
                reason: "同名の添付が既にあります".to_string(),
            }),
            Err(rejection) => rejected.push(rejection),
        }
    }
    AttachmentPreparation {
        attachments,
        rejected,
    }
}

#[tauri::command]
pub fn prepare_attachments(app: tauri::AppHandle, sources: Vec<AttachmentSource>) -> AttachmentPreparation {
    let settings = crate::settings::load_settings(app).unwrap_or_default();
    let vision_ready = settings.vision_enabled
        && settings.mmproj_path.as_deref().is_some_and(|path| !path.trim().is_empty());
    prepare_attachments_for_sources(sources, vision_ready)
}

#[tauri::command]
pub fn read_files_as_attachments(app: tauri::AppHandle, paths: Vec<String>) -> Result<Vec<Attachment>, TauriError> {
    let settings = crate::settings::load_settings(app).unwrap_or_default();
    Ok(prepare_attachments_for_sources(
        paths
            .into_iter()
            .map(|path| AttachmentSource::Path { path })
            .collect(),
        settings.vision_enabled
            && settings
                .mmproj_path
                .as_deref()
                .is_some_and(|path| !path.trim().is_empty()),
    )
    .attachments)
}
