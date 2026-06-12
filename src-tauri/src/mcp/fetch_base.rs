use std::fs;
use std::path::PathBuf;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use crate::connections::{load_connections, get_mcp_hosts};
use crate::network::{SidecarNetmikoWrapper, DeviceConfig, CommandResult, NetworkInterface};
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

pub fn get_template_for_dtype<'a>(templates: &'a CommandTemplates, dtype: &str) -> Option<&'a CommandTemplate> {
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

pub async fn resolve_device_config(app: &tauri::AppHandle, device_name: &str) -> Result<DeviceConfig, String> {
    let mut resolved_name = device_name.to_string();
    if resolved_name.trim().is_empty() {
        if let Ok(connections) = load_connections(app.clone()) {
            if let Some(conn) = connections.iter().find(|c| c.conn_type.contains("Console") || c.conn_type.contains("Serial")) {
                resolved_name = conn.hostname.clone();
            }
        }

        if resolved_name.trim().is_empty() {
            let settings = crate::settings::load_settings(app.clone()).unwrap_or_default();
            if let Some(first_recent) = settings.recent_ips.first() {
                resolved_name = first_recent.clone();
            }
        }
    }

    if resolved_name.trim().is_empty() {
        return Err("Error: device_name (機器名) is required but was not provided or is empty.".to_string());
    }

    let settings = crate::settings::load_settings(app.clone()).unwrap_or_default();
    let console_port = match settings.console_port {
        Some(ref p) if !p.trim().is_empty() && p != "None" => Some(p.clone()),
        _ => None,
    };

    // Find the device in connections or mcp_hosts first to check if it's a console connection
    let mut is_console = console_port.is_some();
    let mut resolved_device = None;
    
    if let Ok(connections) = load_connections(app.clone()) {
        if let Some(conn) = connections.iter().find(|c| c.hostname.to_lowercase() == resolved_name.to_lowercase() || c.ip == resolved_name) {
            let dtype = if let Some(dt) = &conn.device_type {
                dt.clone()
            } else if conn.conn_type.contains("Cisco IOS") { "cisco_ios".to_string() }
                        else if conn.conn_type.contains("Juniper") { "juniper_junos".to_string() }
                        else if conn.conn_type.contains("Arista") { "arista_eos".to_string() }
                        else if conn.conn_type.contains("Yamaha") { "yamaha".to_string() }
                        else if conn.conn_type.contains("Furukawa") || conn.conn_type.contains("Fitelnet") { "furukawa_fitelnet".to_string() }
                        else { "cisco_ios".to_string() };

            if conn.conn_type.contains("Console") || conn.conn_type.contains("Serial") {
                is_console = true;
            }

            let user = conn.username.clone().unwrap_or_else(|| "admin".to_string());
            resolved_device = Some((conn.ip.clone(), user, conn.password.clone(), conn.enable_password.clone(), dtype));
        }
    }
    
    if resolved_device.is_none() {
        if let Ok(mcp_hosts) = get_mcp_hosts() {
            if let Some(mcp) = mcp_hosts.iter().find(|h| h.hostname.to_lowercase() == resolved_name.to_lowercase() || h.ip == resolved_name) {
                let dtype = if mcp.device_type.contains("Cisco IOS") { "cisco_ios" }
                            else if mcp.device_type.contains("Juniper") { "juniper_junos" }
                            else if mcp.device_type.contains("Arista") { "arista_eos" }
                            else if mcp.device_type.contains("Yamaha") { "yamaha" }
                            else if mcp.device_type.contains("Furukawa") || mcp.device_type.contains("Fitelnet") { "furukawa_fitelnet" }
                            else { "cisco_ios" };

                if mcp.device_type.contains("Console") || mcp.device_type.contains("Serial") {
                    is_console = true;
                }

                resolved_device = Some((mcp.ip.clone(), mcp.username.clone(), None, None, dtype.to_string()));
            }
        }
    }

    // Now check IP validation
    if !is_console && resolved_name.parse::<std::net::IpAddr>().is_ok() {
        return Err("IP address input is not allowed. Please specify the registered device name.".to_string());
    }
    
    let (ip, username, password, enable_password, dtype) = match resolved_device {
        Some(d) => d,
        None => return Err(format!("Error: Device '{}' is not registered. Only registered device names are allowed.", resolved_name)),
    };

    let settings = crate::settings::load_settings(app.clone()).unwrap_or_default();
    let mut console_port = match settings.console_port {
        Some(ref p) if !p.trim().is_empty() && p != "None" => Some(p.clone()),
        _ => None,
    };
    let console_baud_rate = settings.console_baud_rate;

    if is_console && console_port.is_none() {
        if let Ok(ports) = serialport::available_ports() {
            if let Some(p) = ports.first() {
                console_port = Some(p.port_name.clone());
            }
        }
        if console_port.is_none() {
            #[cfg(target_os = "windows")]
            { console_port = Some("COM1".to_string()); }
            #[cfg(not(target_os = "windows"))]
            { console_port = Some("/dev/ttyUSB0".to_string()); }
        }
    }

    Ok(DeviceConfig {
        host: ip,
        username,
        password,
        enable_password,
        device_type: dtype,
        console_port,
        console_baud_rate,
    })
}

pub trait McpCommandFetcher {
    fn get_command_from_template(&self, template: &CommandTemplate) -> String;
    fn get_log_prefix(&self) -> &'static str;

    async fn fetch_device_info(
        &self,
        app: &tauri::AppHandle,
        device_name: &str,
    ) -> Result<CommandResult, String> {
        let target_device = resolve_device_config(app, device_name).await?;
        let templates = load_templates(app);
        let template = match get_template_for_dtype(&templates, &target_device.device_type) {
            Some(t) => t,
            None => return Err(format!("Error: No command template found for device type '{}'.", target_device.device_type)),
        };
        let command = self.get_command_from_template(template);
        if let Some(ref port) = target_device.console_port {
            println!("Fetching {} for registered device '{}' via console port '{}' using command '{}'", self.get_log_prefix(), device_name, port, command);
        } else {
            println!("Fetching {} for registered device '{}' using command '{}'", self.get_log_prefix(), device_name, command);
        }
        let wrapper = SidecarNetmikoWrapper::new(app);
        match wrapper.execute_show(&target_device, &command).await {
            Ok(output) => Ok(CommandResult { success: true, output }),
            Err(err) => Ok(CommandResult { success: false, output: err }),
        }
    }
}
