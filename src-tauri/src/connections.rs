use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::Manager;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    pub id: String,
    pub status: String,
    pub hostname: String,
    pub ip: String,
    #[serde(rename = "type")]
    pub conn_type: String,
    pub last_connected: String,
}

fn get_connections_path(app: &tauri::AppHandle) -> PathBuf {
    let path = app.path().app_data_dir().expect("Failed to get app data dir");
    if !path.exists() {
        let _ = fs::create_dir_all(&path);
    }
    path.join("connections.json")
}

#[tauri::command]
pub fn load_connections(app: tauri::AppHandle) -> Result<Vec<Connection>, String> {
    let path = get_connections_path(&app);
    if !path.exists() {
        return Ok(vec![]);
    }
    let data = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let connections: Vec<Connection> = serde_json::from_str(&data).map_err(|e| e.to_string())?;
    Ok(connections)
}

#[tauri::command]
pub fn save_connections(app: tauri::AppHandle, connections: Vec<Connection>) -> Result<(), String> {
    let path = get_connections_path(&app);
    let data = serde_json::to_string_pretty(&connections).map_err(|e| e.to_string())?;
    fs::write(path, data).map_err(|e| e.to_string())?;
    Ok(())
}
