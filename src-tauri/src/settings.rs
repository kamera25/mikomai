use crate::error::TauriError;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::Manager;
use validator::Validate;

fn default_false() -> bool {
    false
}

fn default_cache_expiry() -> Option<u64> {
    Some(10)
}

fn default_prompt_keep_tokens() -> usize {
    500
}

fn default_n_ctx() -> usize {
    4096
}

fn default_max_gen() -> usize {
    2048
}

#[derive(Serialize, Deserialize, Clone, Debug, Validate)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    #[validate(range(min = 0, max = 100))]
    pub history_limit: usize,
    #[validate(range(min = 0.0, max = 2.0))]
    pub temperature: f32,
    #[validate(range(min = 0.5, max = 2.0))]
    pub repetition_penalty: f32,
    pub model_path: Option<String>,
    pub recent_ips: Vec<String>,
    #[serde(default)]
    #[validate(range(min = 1, max = 300))]
    pub mcp_timeout: Option<u64>,
    pub db_path: Option<String>,
    #[serde(default)]
    pub ip_version: Option<String>,
    #[serde(default)]
    pub console_port: Option<String>,
    #[serde(default)]
    #[validate(range(min = 110, max = 1000000))]
    pub console_baud_rate: Option<u32>,
    #[serde(default = "default_false")]
    pub preload_knowledge: bool,
    #[serde(default = "default_false")]
    pub preload_analysis: bool,
    #[serde(default = "default_false")]
    pub preload_rag: bool,
    #[serde(default = "default_false")]
    pub preload_plotter: bool,
    #[serde(default = "default_false")]
    pub preload_builder: bool,
    #[serde(default = "default_false")]
    pub preload_summarization: bool,
    #[serde(default = "default_cache_expiry")]
    #[validate(range(min = 0, max = 1440))]
    pub cache_expiry_minutes: Option<u64>,
    #[serde(default = "default_n_ctx")]
    #[validate(range(min = 512, max = 32768))]
    pub n_ctx: usize,
    #[serde(default = "default_max_gen")]
    #[validate(range(min = 1, max = 8192))]
    pub max_gen: usize,
    #[serde(default = "default_prompt_keep_tokens")]
    #[validate(range(min = 0, max = 4096))]
    pub prompt_keep_tokens: usize,
    #[serde(default = "default_false")]
    pub vision_enabled: bool,
    #[serde(default = "default_false")]
    pub auto_dry_run: bool,
    #[serde(default)]
    pub mmproj_path: Option<String>,
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
            preload_knowledge: false,
            preload_analysis: false,
            preload_rag: false,
            preload_plotter: false,
            preload_builder: false,
            preload_summarization: false,
            cache_expiry_minutes: Some(10),
            n_ctx: 4096,
            max_gen: 2048,
            prompt_keep_tokens: 500,
            vision_enabled: false,
            auto_dry_run: false,
            mmproj_path: None,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("File I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization/Deserialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Tauri path resolution failed")]
    TauriPath,
}

impl serde::Serialize for SettingsError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

fn get_settings_path<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> PathBuf {
    let path = app
        .path()
        .app_data_dir()
        .expect("Failed to get app data dir");
    if !path.exists() {
        let _ = fs::create_dir_all(&path);
    }
    path.join("settings.json")
}

#[tauri::command]
pub fn load_settings<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<AppSettings, TauriError> {
    let path = get_settings_path(&app);
    let mut settings = if !path.exists() {
        AppSettings::default()
    } else {
        let data = fs::read_to_string(path)?;
        serde_json::from_str(&data)?
    };

    let has_no_db_path = settings
        .db_path
        .as_ref()
        .map_or(true, |s| s.trim().is_empty());
    if has_no_db_path {
        let app_data_dir = app
            .path()
            .app_data_dir()
            .map_err(|_| SettingsError::TauriPath)?;
        settings.db_path = Some(app_data_dir.join("lancedb").to_string_lossy().to_string());
    }

    Ok(settings)
}

#[tauri::command]
pub fn save_settings<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    settings: AppSettings,
) -> Result<(), TauriError> {
    settings
        .validate()
        .map_err(|e| TauriError(crate::error::MikomaiError::Validation(e.to_string())))?;
    let path = get_settings_path(&app);
    let data = serde_json::to_string_pretty(&settings)?;
    fs::write(path, data)?;
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
        assert!(!settings.preload_knowledge);
        assert!(!settings.preload_analysis);
        assert!(!settings.preload_rag);
        assert!(!settings.preload_plotter);
        assert!(!settings.preload_builder);
        assert!(!settings.preload_summarization);
        assert_eq!(settings.cache_expiry_minutes, Some(10));
        assert_eq!(settings.prompt_keep_tokens, 500);
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
            preload_knowledge: true,
            preload_analysis: false,
            preload_rag: true,
            preload_plotter: true,
            preload_builder: false,
            preload_summarization: false,
            cache_expiry_minutes: Some(15),
            n_ctx: 4096,
            max_gen: 2048,
            prompt_keep_tokens: 500,
            vision_enabled: true,
            auto_dry_run: false,
            mmproj_path: Some("/path/to/mmproj.gguf".to_string()),
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
        assert!(serialized.contains(r#""cacheExpiryMinutes":15"#));
        assert!(serialized.contains(r#""promptKeepTokens":500"#));
        assert!(serialized.contains(r#""visionEnabled":true"#));
    }

    #[test]
    fn test_app_settings_zero_validation() {
        let settings = AppSettings {
            history_limit: 0,
            temperature: 0.0,
            repetition_penalty: 1.1,
            model_path: None,
            recent_ips: Vec::new(),
            mcp_timeout: Some(30),
            db_path: None,
            ip_version: Some("auto".to_string()),
            console_port: None,
            console_baud_rate: Some(9600),
            preload_knowledge: false,
            preload_analysis: false,
            preload_rag: false,
            preload_plotter: false,
            preload_builder: false,
            preload_summarization: false,
            cache_expiry_minutes: Some(0),
            n_ctx: 4096,
            max_gen: 2048,
            prompt_keep_tokens: 500,
            vision_enabled: false,
            auto_dry_run: false,
            mmproj_path: None,
        };

        assert!(settings.validate().is_ok());
    }
}
