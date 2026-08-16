use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::Manager;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AttachmentType
{
    Text,
    Image,
    File,
}

impl AttachmentType
{
    pub fn as_str(&self) -> &'static str
    {
        match self
        {
            Self::Text => "text",
            Self::Image => "image",
            Self::File => "file",
        }
    }
}

impl std::fmt::Display for AttachmentType
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result
    {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Attachment
{
    pub name: String,
    #[serde(rename = "type")]
    pub mime_type: AttachmentType,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole
{
    User,
    Ai,
}

impl MessageRole
{
    pub fn as_str(&self) -> &'static str
    {
        match self
        {
            Self::User => "user",
            Self::Ai => "ai",
        }
    }
}

impl std::fmt::Display for MessageRole
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result
    {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecutionStatus
{
    Running,
    Success,
    Failed,
}


#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum UserInputStatus
{
    Pending,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct BaseMessage
{
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
pub enum Message
{
    UserInput
    {
        #[serde(flatten)]
        base: BaseMessage,
        #[serde(skip_serializing_if = "Option::is_none")]
        status: Option<UserInputStatus>,
        #[serde(skip_serializing_if = "Option::is_none")]
        attachments: Option<Vec<Attachment>>,
    },
    ToolExecution
    {
        #[serde(flatten)]
        base: BaseMessage,
        status: ExecutionStatus,
        action_name: String,
        tool_id: String,
        summary_text: String,
        raw_data: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        args: Option<serde_json::Value>,
        #[serde(rename = "saved_path", skip_serializing_if = "Option::is_none")]
        saved_path: Option<String>,
        #[serde(rename = "is_cached", skip_serializing_if = "Option::is_none")]
        is_cached: Option<bool>,
        #[serde(rename = "cache_time", skip_serializing_if = "Option::is_none")]
        cache_time: Option<String>,
    },
    AgentResponse
    {
        #[serde(flatten)]
        base: BaseMessage,
    },
    SystemMessage
    {
        #[serde(flatten)]
        base: BaseMessage,
    },
}


#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChatSession
{
    pub id: uuid::Uuid,
    pub title: String,
    pub messages: Vec<Message>,
    #[serde(rename = "recentIps", skip_serializing_if = "Option::is_none")]
    pub recent_ips: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SummaryItem
{
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub content: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Folder
{
    pub id: uuid::Uuid,
    pub name: String,
    pub items: Vec<HistoryItem>,
    pub is_open: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum HistoryItem
{
    Session(ChatSession),
    Folder(Folder),
}

fn get_history_path(app: &tauri::AppHandle) -> PathBuf
{
    let path = app
        .path()
        .app_data_dir()
        .expect("Failed to get app data dir");
    if !path.exists()
    {
        let _ = fs::create_dir_all(&path);
    }
    path.join("history.json")
}

use crate::error::TauriError;

#[derive(Debug, thiserror::Error)]
pub enum HistoryError
{
    #[error("File I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization/Deserialization error: {0}")]
    Json(#[from] serde_json::Error),
}

pub fn sanitize_history_items(items: &mut Vec<HistoryItem>) -> bool
{
    let mut modified = false;
    for item in items.iter_mut()
    {
        match item
        {
            HistoryItem::Session(ref mut session) =>
            {
                for msg in session.messages.iter_mut()
                {
                    match msg
                    {
                        Message::ToolExecution {
                            ref mut status,
                            ref mut base,
                            ref mut summary_text,
                            ref mut raw_data,
                            ref action_name,
                            ..
                        } =>
                        {
                            if *status == ExecutionStatus::Running || base.is_tool_loading == Some(true)
                            {
                                *status = ExecutionStatus::Failed;
                                base.is_tool_loading = Some(false);
                                *summary_text = format!("{} 失敗", action_name);
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
                        Message::AgentResponse { ref mut base }
                        | Message::UserInput { ref mut base, .. }
                        | Message::SystemMessage { ref mut base } =>
                        {
                            if base.is_tool_loading == Some(true)
                            {
                                base.is_tool_loading = Some(false);
                                modified = true;
                            }
                        }
                    }
                }
            }
            HistoryItem::Folder(ref mut folder) =>
            {
                if sanitize_history_items(&mut folder.items)
                {
                    modified = true;
                }
            }
        }
    }
    modified
}

pub fn cleanup_running_history_on_exit(app: &tauri::AppHandle) -> Result<(), TauriError>
{
    let path = get_history_path(app);
    if !path.exists()
    {
        return Ok(());
    }
    let data = fs::read_to_string(&path)?;
    let mut history: Vec<HistoryItem> = serde_json::from_str(&data)?;
    if sanitize_history_items(&mut history)
    {
        let data = serde_json::to_string_pretty(&history)?;
        fs::write(path, data)?;
    }
    Ok(())
}

#[tauri::command]
pub fn load_history(app: tauri::AppHandle) -> Result<Vec<HistoryItem>, TauriError>
{
    let path = get_history_path(&app);
    if !path.exists()
    {
        return Ok(vec![]);
    }
    let data = fs::read_to_string(path)?;
    let mut history: Vec<HistoryItem> = serde_json::from_str(&data)?;
    if sanitize_history_items(&mut history)
    {
        let _ = save_history(app, history.clone());
    }
    Ok(history)
}

#[tauri::command]
pub fn save_history(app: tauri::AppHandle, history: Vec<HistoryItem>) -> Result<(), TauriError>
{
    let path = get_history_path(&app);
    let data = serde_json::to_string_pretty(&history)?;
    fs::write(path, data)?;
    Ok(())
}

#[cfg(test)]
mod tests
{
    use super::*;

    #[test]
    fn test_message_serialization()
    {
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
    fn test_history_item_session_serialization()
    {
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
    fn test_history_item_folder_serialization()
    {
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
    fn test_summary_item_serialization()
    {
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
    fn test_sanitize_history_items_running_mcp()
    {
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
                action_name: "ask_interface_choice".to_string(),
                tool_id: "ask_interface_choice".to_string(),
                summary_text: "ask_interface_choice を実行中...".to_string(),
                raw_data: None,
                args: None,
                saved_path: None,
                is_cached: None,
                cache_time: None,
            }],
            recent_ips: None,
        })];

        let modified = sanitize_history_items(&mut history);
        assert!(modified);

        if let HistoryItem::Session(session) = &history[0]
        {
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
                assert_eq!(summary_text, "ask_interface_choice 失敗");
                assert_eq!(
                    raw_data.as_deref(),
                    Some("アプリケーションが終了したため、MCPの実行が失敗しました。")
                );
            }
            else
            {
                panic!("Expected ToolExecution message");
            }
        }
        else
        {
            panic!("Expected Session item");
        }
    }


    #[test]
    fn test_attachment_serialization_with_path()
    {
        let att = Attachment {
            name: "firmware.bin".to_string(),
            mime_type: AttachmentType::File,
            content: "[ファイル: firmware.bin (サイズ: 1.2 MB)]".to_string(),
            path: Some("/tmp/firmware.bin".to_string()),
        };
        let json = serde_json::to_string(&att).unwrap();
        assert!(json.contains(r#""name":"firmware.bin""#));
        assert!(json.contains(r#""type":"file""#));
        assert!(json.contains(r#""path":"/tmp/firmware.bin""#));

        let deserialized: Attachment = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "firmware.bin");
        assert_eq!(deserialized.mime_type, AttachmentType::File);
        assert_eq!(deserialized.path, Some("/tmp/firmware.bin".to_string()));
    }

    #[test]
    fn test_read_files_as_attachments_text_and_binary()
    {
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

        let atts = read_files_as_attachments(paths).unwrap();
        assert_eq!(atts.len(), 2);

        let text_att = &atts[0];
        assert_eq!(text_att.name, "test_text_file.txt");
        assert_eq!(text_att.mime_type, AttachmentType::Text);
        assert!(text_att.content.contains("hostname Switch1"));
        assert_eq!(text_att.path, Some(text_path.to_string_lossy().to_string()));

        let bin_att = &atts[1];
        assert_eq!(bin_att.name, "test_bin_file.bin");
        assert_eq!(bin_att.mime_type, AttachmentType::File);
        assert!(bin_att.content.contains("test_bin_file.bin"));
        assert_eq!(bin_att.path, Some(bin_path.to_string_lossy().to_string()));

        let _ = fs::remove_file(text_path);
        let _ = fs::remove_file(bin_path);
    }
}

fn get_summaries_path(app: &tauri::AppHandle) -> PathBuf
{
    let path = app
        .path()
        .app_data_dir()
        .expect("Failed to get app data dir");
    if !path.exists()
    {
        let _ = fs::create_dir_all(&path);
    }
    path.join("summaries.json")
}

#[tauri::command]
pub fn load_summaries(app: tauri::AppHandle) -> Result<Vec<SummaryItem>, TauriError>
{
    let path = get_summaries_path(&app);
    if !path.exists()
    {
        return Ok(vec![]);
    }
    let data = fs::read_to_string(path)?;
    let summaries: Vec<SummaryItem> = serde_json::from_str(&data)?;
    Ok(summaries)
}

#[tauri::command]
pub fn save_summary(app: tauri::AppHandle, summary: SummaryItem) -> Result<(), TauriError>
{
    let mut summaries = load_summaries(app.clone()).unwrap_or_default();
    summaries.push(summary);

    // Keep only the last 100 summaries to prevent the file from growing indefinitely
    if summaries.len() > 100
    {
        let skip = summaries.len() - 100;
        summaries = summaries.into_iter().skip(skip).collect();
    }

    let path = get_summaries_path(&app);
    let data = serde_json::to_string_pretty(&summaries)?;
    fs::write(path, data)?;
    Ok(())
}

use base64::{engine::general_purpose, Engine as _};

#[tauri::command]
pub fn read_files_as_attachments(paths: Vec<String>) -> Result<Vec<Attachment>, TauriError>
{
    let mut result = Vec::new();
    for path_str in paths
    {
        let path = std::path::Path::new(&path_str);
        if !path.exists() || !path.is_file()
        {
            continue;
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

        if is_image
        {
            if let Ok(bytes) = fs::read(&path)
            {
                let mime = match extension.as_str()
                {
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
                result.push(Attachment {
                    name: file_name,
                    mime_type: AttachmentType::Image,
                    content: data_url,
                    path: Some(path_str),
                });
            }
        }
        else
        {
            let file_size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            const MAX_TEXT_FILE_SIZE: u64 = 512 * 1024; // 512 KB

            if file_size <= MAX_TEXT_FILE_SIZE
            {
                if let Ok(text) = fs::read_to_string(&path)
                {
                    result.push(Attachment {
                        name: file_name,
                        mime_type: AttachmentType::Text,
                        content: text,
                        path: Some(path_str),
                    });
                    continue;
                }
            }

            let size_desc = if file_size < 1024
            {
                format!("{} B", file_size)
            }
            else if file_size < 1024 * 1024
            {
                format!("{:.1} KB", file_size as f64 / 1024.0)
            }
            else
            {
                format!("{:.1} MB", file_size as f64 / (1024.0 * 1024.0))
            };

            result.push(Attachment {
                name: file_name.clone(),
                mime_type: AttachmentType::File,
                content: format!("[ファイル: {} (サイズ: {})]", file_name, size_desc),
                path: Some(path_str),
            });
        }
    }
    Ok(result)
}
