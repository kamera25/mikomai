use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::Manager;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Message {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    #[serde(rename = "isToolLoading", skip_serializing_if = "Option::is_none")]
    pub is_tool_loading: Option<bool>,
    #[serde(rename = "isHidden", skip_serializing_if = "Option::is_none")]
    pub is_hidden: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_data: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<serde_json::Value>,
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

#[tauri::command]
pub fn load_history(app: tauri::AppHandle) -> Result<Vec<HistoryItem>, String> {
    let path = get_history_path(&app);
    if !path.exists() {
        return Ok(vec![]);
    }
    let data = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let history: Vec<HistoryItem> = serde_json::from_str(&data).map_err(|e| e.to_string())?;
    Ok(history)
}

#[tauri::command]
pub fn save_history(app: tauri::AppHandle, history: Vec<HistoryItem>) -> Result<(), String> {
    let path = get_history_path(&app);
    let data = serde_json::to_string_pretty(&history).map_err(|e| e.to_string())?;
    fs::write(path, data).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_serialization() {
        let msg = Message {
            role: "user".to_string(),
            content: "Hello".to_string(),
            timestamp: None,
            is_tool_loading: None,
            is_hidden: None,
            task_id: None,
            event_type: None,
            status: None,
            action_name: None,
            summary_text: None,
            raw_data: None,
            args: None,
        };
        let serialized = serde_json::to_string(&msg).unwrap();
        assert!(serialized.contains(r#""role":"user""#));
        assert!(serialized.contains(r#""content":"Hello""#));
    }

    #[test]
    fn test_history_item_session_serialization() {
        let session = ChatSession {
            id: "session-1".to_string(),
            title: "Test Session".to_string(),
            messages: vec![Message {
                role: "user".to_string(),
                content: "Hi".to_string(),
                timestamp: None,
                is_tool_loading: None,
                is_hidden: None,
                task_id: None,
                event_type: None,
                status: None,
                action_name: None,
                summary_text: None,
                raw_data: None,
                args: None,
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
pub fn load_summaries(app: tauri::AppHandle) -> Result<Vec<SummaryItem>, String> {
    let path = get_summaries_path(&app);
    if !path.exists() {
        return Ok(vec![]);
    }
    let data = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let summaries: Vec<SummaryItem> = serde_json::from_str(&data).map_err(|e| e.to_string())?;
    Ok(summaries)
}

#[tauri::command]
pub fn save_summary(app: tauri::AppHandle, summary: SummaryItem) -> Result<(), String> {
    let mut summaries = load_summaries(app.clone()).unwrap_or_default();
    summaries.push(summary);
    
    // Keep only the last 100 summaries to prevent the file from growing indefinitely
    if summaries.len() > 100 {
        let skip = summaries.len() - 100;
        summaries = summaries.into_iter().skip(skip).collect();
    }

    let path = get_summaries_path(&app);
    let data = serde_json::to_string_pretty(&summaries).map_err(|e| e.to_string())?;
    fs::write(path, data).map_err(|e| e.to_string())?;
    Ok(())
}
