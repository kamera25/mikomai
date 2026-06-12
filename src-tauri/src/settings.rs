use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::Manager;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub history_limit: usize,
    pub temperature: f32,
    pub repetition_penalty: f32,
    pub model_path: Option<String>,
    pub recent_ips: Vec<String>,
    #[serde(default)]
    pub mcp_timeout: Option<u64>,
    pub db_path: Option<String>,
    #[serde(default)]
    pub ip_version: Option<String>,
    #[serde(default)]
    pub console_port: Option<String>,
    #[serde(default)]
    pub console_baud_rate: Option<u32>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            history_limit: 5,
            temperature: 0.0,
            repetition_penalty: 1.1,
            model_path: None,
            recent_ips: Vec::new(),
            mcp_timeout: Some(30),
            db_path: None,
            ip_version: Some("auto".to_string()),
            console_port: None,
            console_baud_rate: Some(9600),
        }
    }
}

fn get_settings_path(app: &tauri::AppHandle) -> PathBuf {
    let path = app.path().app_data_dir().expect("Failed to get app data dir");
    if !path.exists() {
        let _ = fs::create_dir_all(&path);
    }
    path.join("settings.json")
}

#[tauri::command]
pub fn load_settings(app: tauri::AppHandle) -> Result<AppSettings, String> {
    let path = get_settings_path(&app);
    let mut settings = if !path.exists() {
        AppSettings::default()
    } else {
        let data = fs::read_to_string(path).map_err(|e| e.to_string())?;
        serde_json::from_str(&data).map_err(|e| e.to_string())?
    };

    if settings.db_path.is_none() {
        let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
        settings.db_path = Some(app_data_dir.join("lancedb").to_string_lossy().to_string());
    }

    Ok(settings)
}

#[tauri::command]
pub fn save_settings(app: tauri::AppHandle, settings: AppSettings) -> Result<(), String> {
    let path = get_settings_path(&app);
    let data = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    fs::write(path, data).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_settings_default() {
        let settings = AppSettings::default();
        assert_eq!(settings.history_limit, 5);
        assert_eq!(settings.temperature, 0.0);
        assert_eq!(settings.repetition_penalty, 1.1);
        assert!(settings.model_path.is_none());
        assert!(settings.recent_ips.is_empty());
        assert_eq!(settings.mcp_timeout, Some(30));
        assert!(settings.db_path.is_none());
        assert_eq!(settings.ip_version, Some("auto".to_string()));
        assert!(settings.console_port.is_none());
        assert_eq!(settings.console_baud_rate, Some(9600));
    }

    #[test]
    fn test_app_settings_serialization() {
        let settings = AppSettings {
            history_limit: 10,
            temperature: 0.7,
            repetition_penalty: 1.2,
            model_path: Some("/path/to/model".to_string()),
            recent_ips: vec!["192.168.1.1".to_string()],
            mcp_timeout: Some(60),
            db_path: Some("/path/to/db".to_string()),
            ip_version: Some("ipv6".to_string()),
            console_port: Some("/dev/ttyUSB0".to_string()),
            console_baud_rate: Some(115200),
        };

        let serialized = serde_json::to_string(&settings).unwrap();
        assert!(serialized.contains(r#""historyLimit":10"#));
        assert!(serialized.contains(r#""temperature":0.7"#));
        assert!(serialized.contains(r#""repetitionPenalty":1.2"#));
        assert!(serialized.contains(r#""modelPath":"/path/to/model""#));
        assert!(serialized.contains(r#""recentIps":["192.168.1.1"]"#));
        assert!(serialized.contains(r#""mcpTimeout":60"#));
        assert!(serialized.contains(r#""dbPath":"/path/to/db""#));
        assert!(serialized.contains(r#""ipVersion":"ipv6""#));
        assert!(serialized.contains(r#""consolePort":"/dev/ttyUSB0""#));
        assert!(serialized.contains(r#""consoleBaudRate":115200"#));
    }
}
