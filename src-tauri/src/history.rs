use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::Manager;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "event_type")]
pub enum Message {
    UserInput {
        role: String,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        timestamp: Option<String>,
        #[serde(rename = "isToolLoading", skip_serializing_if = "Option::is_none")]
        is_tool_loading: Option<bool>,
        #[serde(rename = "isHidden", skip_serializing_if = "Option::is_none")]
        is_hidden: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        task_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        status: Option<String>,
    },
    ToolExecution {
        role: String,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        timestamp: Option<String>,
        #[serde(rename = "isToolLoading", skip_serializing_if = "Option::is_none")]
        is_tool_loading: Option<bool>,
        #[serde(rename = "isHidden", skip_serializing_if = "Option::is_none")]
        is_hidden: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        task_id: Option<String>,
        status: String,
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
    AgentResponse {
        role: String,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        timestamp: Option<String>,
        #[serde(rename = "isToolLoading", skip_serializing_if = "Option::is_none")]
        is_tool_loading: Option<bool>,
        #[serde(rename = "isHidden", skip_serializing_if = "Option::is_none")]
        is_hidden: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        task_id: Option<String>,
    },
    SystemMessage {
        role: String,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        timestamp: Option<String>,
        #[serde(rename = "isToolLoading", skip_serializing_if = "Option::is_none")]
        is_tool_loading: Option<bool>,
        #[serde(rename = "isHidden", skip_serializing_if = "Option::is_none")]
        is_hidden: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        task_id: Option<String>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChatSession {
    pub id: String,
    pub title: String,
    pub messages: Vec<Message>,
    #[serde(rename = "recentIps", skip_serializing_if = "Option::is_none")]
    pub recent_ips: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SummaryItem {
    pub timestamp: String,
    pub content: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    pub id: String,
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

fn get_history_path(app: &tauri::AppHandle) -> PathBuf {
    let path = app.path().app_data_dir().expect("Failed to get app data dir");
    if !path.exists() {
        let _ = fs::create_dir_all(&path);
    }
    path.join("history.json")
}

use crate::error::TauriError;

#[derive(Debug, thiserror::Error)]
pub enum HistoryError {
    #[error("File I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization/Deserialization error: {0}")]
    Json(#[from] serde_json::Error),
}

#[tauri::command]
pub fn load_history(app: tauri::AppHandle) -> Result<Vec<HistoryItem>, TauriError> {
    let path = get_history_path(&app);
    if !path.exists() {
        return Ok(vec![]);
    }
    let data = fs::read_to_string(path)?;
    let history: Vec<HistoryItem> = serde_json::from_str(&data)?;
    Ok(history)
}

#[tauri::command]
pub fn save_history(app: tauri::AppHandle, history: Vec<HistoryItem>) -> Result<(), TauriError> {
    let path = get_history_path(&app);
    let data = serde_json::to_string_pretty(&history)?;
    fs::write(path, data)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_serialization() {
        let msg = Message::UserInput {
            role: "user".to_string(),
            content: "Hello".to_string(),
            timestamp: None,
            is_tool_loading: None,
            is_hidden: None,
            task_id: None,
            status: None,
        };
        let serialized = serde_json::to_string(&msg).unwrap();
        assert!(serialized.contains(r#""role":"user""#));
        assert!(serialized.contains(r#""content":"Hello""#));
        assert!(serialized.contains(r#""event_type":"UserInput""#));
    }

    #[test]
    fn test_history_item_session_serialization() {
        let session = ChatSession {
            id: "session-1".to_string(),
            title: "Test Session".to_string(),
            messages: vec![Message::UserInput {
                role: "user".to_string(),
                content: "Hi".to_string(),
                timestamp: None,
                is_tool_loading: None,
                is_hidden: None,
                task_id: None,
                status: None,
            }],
            recent_ips: None,
        };
        let item = HistoryItem::Session(session);
        let serialized = serde_json::to_string(&item).unwrap();
        assert!(serialized.contains(r#""type":"session""#));
        assert!(serialized.contains(r#""id":"session-1""#));
    }

    #[test]
    fn test_history_item_folder_serialization() {
        let folder = Folder {
            id: "folder-1".to_string(),
            name: "Test Folder".to_string(),
            items: vec![],
            is_open: true,
        };
        let item = HistoryItem::Folder(folder);
        let serialized = serde_json::to_string(&item).unwrap();
        assert!(serialized.contains(r#""type":"folder""#));
        assert!(serialized.contains(r#""name":"Test Folder""#));
        assert!(serialized.contains(r#""isOpen":true"#));
    }

    #[test]
    fn test_summary_item_serialization() {
        let summary = SummaryItem {
            timestamp: "2023-10-27T10:00:00Z".to_string(),
            content: "Test summary".to_string(),
        };
        let serialized = serde_json::to_string(&summary).unwrap();
        assert!(serialized.contains(r#""timestamp":"2023-10-27T10:00:00Z""#));
        assert!(serialized.contains(r#""content":"Test summary""#));
    }
}

fn get_summaries_path(app: &tauri::AppHandle) -> PathBuf {
    let path = app.path().app_data_dir().expect("Failed to get app data dir");
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
