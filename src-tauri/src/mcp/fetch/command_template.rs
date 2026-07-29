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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FallbackConfig {
    pub device_type: Option<String>,
    pub command: String,
    pub description: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VendorConfig {
    pub command: String,
    pub description: Option<String>,
    pub aliases: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ShowRunningConfigRules {
    pub fallback: FallbackConfig,
    pub vendors: HashMap<String, VendorConfig>,
}

pub fn get_show_running_config_command(device_type: &str) -> String {
    let yaml_content = std::fs::read_to_string("src-tauri/src/mcp/config/show_running_config_commands.yaml")
        .or_else(|_| std::fs::read_to_string("src/mcp/config/show_running_config_commands.yaml"))
        .unwrap_or_else(|_| include_str!("../config/show_running_config_commands.yaml").to_string());

    if let Ok(rules) = serde_yaml::from_str::<ShowRunningConfigRules>(&yaml_content) {
        let dt_lower = device_type.to_lowercase();
        if let Some(v) = rules.vendors.get(&dt_lower) {
            return v.command.clone();
        }
        for (vendor_key, v) in &rules.vendors {
            if dt_lower.contains(vendor_key) {
                return v.command.clone();
            }
            if let Some(aliases) = &v.aliases {
                for alias in aliases {
                    if dt_lower == alias.to_lowercase() || dt_lower.contains(&alias.to_lowercase()) {
                        return v.command.clone();
                    }
                }
            }
        }
        return rules.fallback.command;
    }

    "show running-config".to_string()
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FallbackApplySaveConfig {
    pub device_type: Option<String>,
    pub apply_command: Option<String>,
    pub save_command: Option<String>,
    pub command: Option<String>,
    pub description: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VendorApplySaveConfig {
    pub apply_command: Option<String>,
    pub save_command: Option<String>,
    pub command: Option<String>,
    pub description: Option<String>,
    pub aliases: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ApplyConfigRules {
    pub fallback: FallbackApplySaveConfig,
    pub vendors: HashMap<String, VendorApplySaveConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyAndSaveCommands {
    pub apply_command: String,
    pub save_command: String,
}

pub fn get_apply_and_save_config_commands(device_type: &str) -> ApplyAndSaveCommands {
    let yaml_content = std::fs::read_to_string("src-tauri/src/mcp/config/apply_config_commands.yaml")
        .or_else(|_| std::fs::read_to_string("src/mcp/config/apply_config_commands.yaml"))
        .unwrap_or_else(|_| include_str!("../config/apply_config_commands.yaml").to_string());

    if let Ok(rules) = serde_yaml::from_str::<ApplyConfigRules>(&yaml_content) {
        let dt_lower = device_type.to_lowercase();
        
        let find_vendor = rules.vendors.get(&dt_lower).or_else(|| {
            rules.vendors.values().find(|v| {
                if let Some(aliases) = &v.aliases {
                    aliases.iter().any(|a| dt_lower == a.to_lowercase() || dt_lower.contains(&a.to_lowercase()))
                } else {
                    false
                }
            })
        });

        if let Some(v) = find_vendor {
            let apply = v.apply_command.clone()
                .or_else(|| v.command.clone())
                .unwrap_or_default();
            let save = v.save_command.clone().unwrap_or_default();
            return ApplyAndSaveCommands { apply_command: apply, save_command: save };
        }

        let fallback_apply = rules.fallback.apply_command.clone()
            .or_else(|| rules.fallback.command.clone())
            .unwrap_or_default();
        let fallback_save = rules.fallback.save_command.clone().unwrap_or_default();
        return ApplyAndSaveCommands {
            apply_command: fallback_apply,
            save_command: fallback_save,
        };
    }

    ApplyAndSaveCommands {
        apply_command: String::new(),
        save_command: "write memory".to_string(),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_show_running_config_command() {
        assert_eq!(get_show_running_config_command("furukawa_fitelnet"), "show running.cfg");
        assert_eq!(get_show_running_config_command("cisco_ios"), "show running-config");
        assert_eq!(get_show_running_config_command("juniper_junos"), "show configuration");
        assert_eq!(get_show_running_config_command("yamaha"), "show config");
        assert_eq!(get_show_running_config_command("unknown_device_type"), "show running-config");
    }

    #[test]
    fn test_get_apply_and_save_config_commands() {
        assert_eq!(
            get_apply_and_save_config_commands("furukawa_fitelnet"),
            ApplyAndSaveCommands { apply_command: "commit".to_string(), save_command: "save side".to_string() }
        );
        assert_eq!(
            get_apply_and_save_config_commands("cisco_ios"),
            ApplyAndSaveCommands { apply_command: "".to_string(), save_command: "write memory".to_string() }
        );
        assert_eq!(
            get_apply_and_save_config_commands("juniper_junos"),
            ApplyAndSaveCommands { apply_command: "commit".to_string(), save_command: "".to_string() }
        );
        assert_eq!(
            get_apply_and_save_config_commands("yamaha"),
            ApplyAndSaveCommands { apply_command: "".to_string(), save_command: "save".to_string() }
        );
        assert_eq!(
            get_apply_and_save_config_commands("unknown_device_type"),
            ApplyAndSaveCommands { apply_command: "".to_string(), save_command: "write memory".to_string() }
        );
    }
}




