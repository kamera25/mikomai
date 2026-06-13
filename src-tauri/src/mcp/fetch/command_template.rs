use std::fs;
use std::path::PathBuf;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use tauri::Manager;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CommandTemplate {
    pub fetch_config: String,
    pub fetch_route: String,
    pub fetch_bgp: String,
    pub fetch_arp: String,
}

pub type CommandTemplates = HashMap<String, CommandTemplate>;

pub fn get_templates_path(app: &tauri::AppHandle) -> PathBuf {
    let path = app.path().app_data_dir().expect("Failed to get app data dir");
    if !path.exists() {
        let _ = fs::create_dir_all(&path);
    }
    path.join("command_templates.json")
}

pub fn get_default_templates() -> CommandTemplates {
    const DEFAULT_JSON: &str = include_str!("../default_templates.json");
    serde_json::from_str(DEFAULT_JSON).expect("Failed to parse default_templates.json")
}

pub fn load_templates(app: &tauri::AppHandle) -> CommandTemplates {
    let path = get_templates_path(app);
    if !path.exists() {
        let defaults = get_default_templates();
        if let Ok(data) = serde_json::to_string_pretty(&defaults) {
            let _ = fs::write(&path, data);
        }
        defaults
    } else {
        match fs::read_to_string(&path) {
            Ok(data) => serde_json::from_str(&data).unwrap_or_else(|_| get_default_templates()),
            Err(_) => get_default_templates(),
        }
    }
}

pub fn get_template_for_dtype<'a>(templates: &'a CommandTemplates, dtype: &str) -> Option<&'a CommandTemplate> {
    let dtype_lower = dtype.to_lowercase();
    if templates.contains_key(&dtype_lower) {
        return templates.get(&dtype_lower);
    }
    
    let mapped = map_vendor_type(dtype);
    templates.get(&mapped).or_else(|| templates.get("cisco_ios"))
}

#[derive(Deserialize, Debug, Clone)]
struct VendorPattern {
    patterns: Vec<String>,
    device_type: String,
}

pub fn map_vendor_type(conn_type: &str) -> String {
    static MAPPINGS: std::sync::OnceLock<Vec<VendorPattern>> = std::sync::OnceLock::new();
    let mappings = MAPPINGS.get_or_init(|| {
        let json_str = include_str!("vender_mapping.json");
        serde_json::from_str(json_str).expect("Failed to parse vender_mapping.json")
    });

    let conn_type_lower = conn_type.to_lowercase();
    for mapping in mappings {
        for pattern in &mapping.patterns {
            if conn_type_lower.contains(&pattern.to_lowercase()) {
                return mapping.device_type.clone();
            }
        }
    }

    // フェイルオーバーとして「Cisco IOS」を選択
    "cisco_ios".to_string()
}

