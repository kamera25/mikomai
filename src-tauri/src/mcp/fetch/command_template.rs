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
    const DEFAULT_YAML: &str = include_str!("../config/default_templates.yaml");
    serde_yaml::from_str(DEFAULT_YAML).expect("Failed to parse default_templates.yaml")
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

use std::sync::LazyLock;

static SHOW_RUNNING_CONFIG_RULES: LazyLock<Option<ShowRunningConfigRules>> = LazyLock::new(|| {
    let yaml_content = std::fs::read_to_string("src-tauri/src/mcp/config/show_running_config_commands.yaml")
        .or_else(|_| std::fs::read_to_string("src/mcp/config/show_running_config_commands.yaml"))
        .unwrap_or_else(|_| include_str!("../config/show_running_config_commands.yaml").to_string());
    serde_yaml::from_str(&yaml_content).ok()
});

static APPLY_CONFIG_RULES: LazyLock<Option<ApplyConfigRules>> = LazyLock::new(|| {
    let yaml_content = std::fs::read_to_string("src-tauri/src/mcp/config/apply_config_commands.yaml")
        .or_else(|_| std::fs::read_to_string("src/mcp/config/apply_config_commands.yaml"))
        .unwrap_or_else(|_| include_str!("../config/apply_config_commands.yaml").to_string());
    serde_yaml::from_str(&yaml_content).ok()
});

pub fn get_show_running_config_command(device_type: &str) -> String {
    if let Some(rules) = SHOW_RUNNING_CONFIG_RULES.as_ref() {
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
        return rules.fallback.command.clone();
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
    if let Some(rules) = APPLY_CONFIG_RULES.as_ref() {
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
    let conn_type_trimmed = conn_type.trim();
    if let Some(brand) = crate::mcp::brands::get_brand(conn_type_trimmed) {
        return brand.to_string();
    }

    if let Some((brand, _)) = crate::mcp::brands::detect_brand_in_text(conn_type_trimmed) {
        return brand.to_string();
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
            ApplyAndSaveCommands { apply_command: "commit".to_string(), save_command: "save moff".to_string() }
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

    #[test]
    fn test_map_vendor_type() {
        assert_eq!(map_vendor_type("Cisco IOS"), "cisco_ios");
        assert_eq!(map_vendor_type("cisco"), "cisco_ios");
        assert_eq!(map_vendor_type("Juniper"), "juniper_junos");
        assert_eq!(map_vendor_type("Fortigate"), "fortinet");
        assert_eq!(map_vendor_type("Yamaha"), "yamaha");
        assert_eq!(map_vendor_type("Furukawa"), "furukawa_fitelnet");
        assert_eq!(map_vendor_type("A10"), "a10");
        assert_eq!(map_vendor_type("PaloAlto"), "paloalto_panos");
        assert_eq!(map_vendor_type("unknown_device"), "cisco_ios");
    }
}




