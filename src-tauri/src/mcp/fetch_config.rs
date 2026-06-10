use std::fs;
use std::path::PathBuf;
use tauri::Manager;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use crate::connections::{load_connections, get_mcp_hosts};
use crate::network::{SidecarNetmikoWrapper, DeviceConfig, CommandResult, NetworkInterface};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CommandTemplate {
    pub fetch_config: String,
    pub fetch_route: String,
    pub fetch_bgp: String,
}

pub type CommandTemplates = HashMap<String, CommandTemplate>;

fn get_templates_path(app: &tauri::AppHandle) -> PathBuf {
    let path = app.path().app_data_dir().expect("Failed to get app data dir");
    if !path.exists() {
        let _ = fs::create_dir_all(&path);
    }
    path.join("command_templates.json")
}

fn get_default_templates() -> CommandTemplates {
    const DEFAULT_JSON: &str = include_str!("default_templates.json");
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

fn get_template_for_dtype<'a>(templates: &'a CommandTemplates, dtype: &str) -> Option<&'a CommandTemplate> {
    let dtype_lower = dtype.to_lowercase();
    if templates.contains_key(&dtype_lower) {
        return templates.get(&dtype_lower);
    }
    
    if dtype_lower.contains("cisco_ios") || dtype_lower.contains("cisco") {
        return templates.get("cisco_ios");
    }
    if dtype_lower.contains("juniper") || dtype_lower.contains("junos") {
        return templates.get("juniper_junos");
    }
    if dtype_lower.contains("arista") || dtype_lower.contains("eos") {
        return templates.get("arista_eos");
    }
    if dtype_lower.contains("yamaha") {
        return templates.get("yamaha");
    }
    if dtype_lower.contains("furukawa") || dtype_lower.contains("fitelnet") {
        return templates.get("furukawa_fitelnet");
    }
    
    templates.get("cisco_ios")
}

#[tauri::command]
pub async fn fetch_config(app: tauri::AppHandle, device_name: String) -> Result<CommandResult, String> {
    if device_name.parse::<std::net::IpAddr>().is_ok() {
        return Err("IP address input is not allowed. Please specify the registered device name.".to_string());
    }

    let mut resolved_device = None;
    
    if let Ok(connections) = load_connections(app.clone()) {
        if let Some(conn) = connections.iter().find(|c| c.hostname.to_lowercase() == device_name.to_lowercase()) {
            let dtype = if let Some(dt) = &conn.device_type {
                dt.clone()
            } else if conn.conn_type.contains("Cisco IOS") { "cisco_ios".to_string() }
                        else if conn.conn_type.contains("Juniper") { "juniper_junos".to_string() }
                        else if conn.conn_type.contains("Arista") { "arista_eos".to_string() }
                        else if conn.conn_type.contains("Yamaha") { "yamaha".to_string() }
                        else if conn.conn_type.contains("Furukawa") || conn.conn_type.contains("Fitelnet") { "furukawa_fitelnet".to_string() }
                        else { "cisco_ios".to_string() };

            let user = conn.username.clone().unwrap_or_else(|| "admin".to_string());
            resolved_device = Some((conn.ip.clone(), user, conn.password.clone(), conn.enable_password.clone(), dtype));
        }
    }
    
    if resolved_device.is_none() {
        if let Ok(mcp_hosts) = get_mcp_hosts() {
            if let Some(mcp) = mcp_hosts.iter().find(|h| h.hostname.to_lowercase() == device_name.to_lowercase()) {
                let dtype = if mcp.device_type.contains("Cisco IOS") { "cisco_ios" }
                            else if mcp.device_type.contains("Juniper") { "juniper_junos" }
                            else if mcp.device_type.contains("Arista") { "arista_eos" }
                            else if mcp.device_type.contains("Yamaha") { "yamaha" }
                            else if mcp.device_type.contains("Furukawa") || mcp.device_type.contains("Fitelnet") { "furukawa_fitelnet" }
                            else { "cisco_ios" };
                resolved_device = Some((mcp.ip.clone(), mcp.username.clone(), None, None, dtype.to_string()));
            }
        }
    }
    
    let (ip, username, password, enable_password, dtype) = match resolved_device {
        Some(d) => d,
        None => return Err(format!("Error: Device '{}' is not registered. Only registered device names are allowed.", device_name)),
    };
    
    let templates = load_templates(&app);
    let template = match get_template_for_dtype(&templates, &dtype) {
        Some(t) => t,
        None => return Err(format!("Error: No command template found for device type '{}'.", dtype)),
    };
    
    let command = template.fetch_config.clone();
    
    let target_device = DeviceConfig {
        host: ip,
        username,
        password,
        enable_password,
        device_type: dtype,
    };
    
    println!("Fetching config for registered device '{}' using command '{}'", device_name, command);
    
    let wrapper = SidecarNetmikoWrapper::new(&app);
    match wrapper.execute_show(&target_device, &command).await {
        Ok(output) => Ok(CommandResult { success: true, output }),
        Err(err) => Ok(CommandResult { success: false, output: err }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_templates() {
        let templates = get_default_templates();
        assert!(templates.contains_key("cisco_ios"));
        assert!(templates.contains_key("juniper_junos"));
        assert!(templates.contains_key("arista_eos"));
        assert!(templates.contains_key("yamaha"));
        assert!(templates.contains_key("furukawa_fitelnet"));

        let cisco = templates.get("cisco_ios").unwrap();
        assert_eq!(cisco.fetch_config, "show running-config");

        let yamaha = templates.get("yamaha").unwrap();
        assert_eq!(yamaha.fetch_config, "show config");

        let furukawa = templates.get("furukawa_fitelnet").unwrap();
        assert_eq!(furukawa.fetch_config, "show running-config");
    }

    #[test]
    fn test_get_template_for_dtype() {
        let templates = get_default_templates();

        // Exact match
        let t1 = get_template_for_dtype(&templates, "cisco_ios").unwrap();
        assert_eq!(t1.fetch_config, "show running-config");

        // Substring matches
        let t2 = get_template_for_dtype(&templates, "Cisco IOS Switch").unwrap();
        assert_eq!(t2.fetch_config, "show running-config");

        let t3 = get_template_for_dtype(&templates, "Juniper SRX").unwrap();
        assert_eq!(t3.fetch_config, "show configuration");

        let t4 = get_template_for_dtype(&templates, "Arista EOS").unwrap();
        assert_eq!(t4.fetch_config, "show running-config");

        let t_yamaha = get_template_for_dtype(&templates, "Yamaha RTX").unwrap();
        assert_eq!(t_yamaha.fetch_config, "show config");

        let t_furukawa = get_template_for_dtype(&templates, "Furukawa Fitelnet").unwrap();
        assert_eq!(t_furukawa.fetch_config, "show running-config");

        // Default fallback
        let t5 = get_template_for_dtype(&templates, "unknown_vendor").unwrap();
        assert_eq!(t5.fetch_config, "show running-config");
    }
}
