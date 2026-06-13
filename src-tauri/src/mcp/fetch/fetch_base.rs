use std::fs;
use std::path::PathBuf;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use crate::connections::{load_connections, get_mcp_hosts};
use crate::network::{DeviceConfig, CommandResult};
use crate::mcp::fetch::netmiko_connection_wraper::NetmikoConnectionWrapper;
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

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionType {
    SSH,
    Console,
    Telnet,
}

impl ConnectionType {
    pub fn from_str(s: &str) -> Option<Self> {
        let s_lower = s.to_lowercase();
        if s_lower.contains("console") || s_lower.contains("serial") {
            Some(ConnectionType::Console)
        } else if s_lower.contains("telnet") {
            Some(ConnectionType::Telnet)
        } else if s_lower.contains("ssh") {
            Some(ConnectionType::SSH)
        } else {
            None
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
struct VendorPattern {
    patterns: Vec<String>,
    device_type: String,
}

fn map_vendor_type(conn_type: &str) -> String {
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
    "cisco_ios".to_string()
}

pub async fn resolve_device_config(app: &tauri::AppHandle, device_name: &str) -> Result<DeviceConfig, String> {
    let mut resolved_name = device_name.to_string();
    if resolved_name.trim().is_empty() {
        if let Ok(connections) = load_connections(app.clone()) {
            if let Some(conn) = connections.iter().find(|c| ConnectionType::from_str(&c.conn_type) == Some(ConnectionType::Console)) {
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
    let _console_port = match settings.console_port {
        Some(ref p) if !p.trim().is_empty() && p != "None" => Some(p.clone()),
        _ => None,
    };

    // Find the device in connections or mcp_hosts first to check if it's a console connection
    let mut is_console = false;
    let mut resolved_device = None;
    
    if let Ok(connections) = load_connections(app.clone()) {
        if let Some(conn) = connections.iter().find(|c| c.hostname.to_lowercase() == resolved_name.to_lowercase() || c.ip == resolved_name) {
            let mut dtype = if let Some(dt) = &conn.device_type {
                dt.clone()
            } else {
                map_vendor_type(&conn.conn_type)
            };

            match ConnectionType::from_str(&conn.conn_type) {
                Some(ConnectionType::Console) => {
                    is_console = true;
                }
                Some(ConnectionType::Telnet) => {
                    if !dtype.ends_with("_telnet") {
                        dtype = format!("{}_telnet", dtype);
                    }
                }
                _ => {}
            }

            let user = conn.username.clone().unwrap_or_else(|| "admin".to_string());
            resolved_device = Some((conn.ip.clone(), user, conn.password.clone(), conn.enable_password.clone(), dtype));
        }
    }
    
    if resolved_device.is_none() {
        if let Ok(mcp_hosts) = get_mcp_hosts() {
            if let Some(mcp) = mcp_hosts.iter().find(|h| h.hostname.to_lowercase() == resolved_name.to_lowercase() || h.ip == resolved_name) {
                let mut dtype = map_vendor_type(&mcp.device_type);

                match ConnectionType::from_str(&mcp.device_type) {
                    Some(ConnectionType::Console) => {
                        is_console = true;
                    }
                    Some(ConnectionType::Telnet) => {
                        if !dtype.ends_with("_telnet") {
                            dtype = format!("{}_telnet", dtype);
                        }
                    }
                    _ => {}
                }

                resolved_device = Some((mcp.ip.clone(), mcp.username.clone(), None, None, dtype));
            }
        }
    }

    // Now check IP validation
    if !is_console && resolved_name.parse::<std::net::IpAddr>().is_ok() && resolved_device.is_none() {
        return Err("IP address input is not allowed. Please specify the registered device name.".to_string());
    }
    
    let (ip, username, password, enable_password, dtype) = match resolved_device {
        Some(d) => d,
        None => return Err(format!("Error: Device '{}' is not registered. Only registered device names are allowed.", resolved_name)),
    };

    let (final_console_port, final_console_baud_rate) = if is_console {
        let mut port = match settings.console_port {
            Some(ref p) if !p.trim().is_empty() && p != "None" => Some(p.clone()),
            _ => None,
        };
        if port.is_none() {
            if let Ok(ports) = serialport::available_ports() {
                if let Some(p) = ports.first() {
                    port = Some(p.port_name.clone());
                }
            }
            if port.is_none() {
                #[cfg(target_os = "windows")]
                { port = Some("COM1".to_string()); }
                #[cfg(not(target_os = "windows"))]
                { port = Some("/dev/ttyUSB0".to_string()); }
            }
        }
        (port, settings.console_baud_rate)
    } else {
        (None, None)
    };

    Ok(DeviceConfig {
        host: ip,
        username,
        password,
        enable_password,
        device_type: dtype,
        console_port: final_console_port,
        console_baud_rate: final_console_baud_rate,
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
        let wrapper = NetmikoConnectionWrapper::new(app);
        match wrapper.execute_show(&target_device, &command).await {
            Ok(output) => {
                let saved_path = if !output.trim().is_empty() {
                    if let Ok(mut manager) = crate::snapshot::SnapshotManager::new() {
                        let data_type = self.get_log_prefix().to_lowercase();
                        if let Ok(path) = manager.save_artifact(device_name, &data_type, &output) {
                            let _ = manager.update_current_link(path.parent().unwrap());
                            Some(path.to_string_lossy().to_string())
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                };
                Ok(CommandResult { success: true, output, saved_path })
            }
            Err(err) => Ok(CommandResult { success: false, output: err, saved_path: None }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_template_serialization() {
        let template = CommandTemplate {
            fetch_config: "show run".to_string(),
            fetch_route: "show ip route".to_string(),
            fetch_bgp: "show ip bgp".to_string(),
            fetch_arp: "show ip arp".to_string(),
        };
        let serialized = serde_json::to_string(&template).unwrap();
        assert!(serialized.contains(r#""fetch_config":"show run""#));
        assert!(serialized.contains(r#""fetch_route":"show ip route""#));
        assert!(serialized.contains(r#""fetch_bgp":"show ip bgp""#));
        assert!(serialized.contains(r#""fetch_arp":"show ip arp""#));
    }

    #[test]
    fn test_default_templates_loading() {
        let templates = get_default_templates();
        assert!(templates.contains_key("cisco_ios"));
        assert!(templates.contains_key("juniper_junos"));
        assert!(templates.contains_key("arista_eos"));
        assert!(templates.contains_key("yamaha"));
        assert!(templates.contains_key("furukawa_fitelnet"));

        let cisco = templates.get("cisco_ios").unwrap();
        assert_eq!(cisco.fetch_config, "show running-config");
        assert_eq!(cisco.fetch_arp, "show ip arp");
    }

    #[test]
    fn test_get_template_for_dtype() {
        let templates = get_default_templates();

        // Exact match
        let t1 = get_template_for_dtype(&templates, "cisco_ios").unwrap();
        assert_eq!(t1.fetch_config, "show running-config");

        // Substring and fallback checks
        let t2 = get_template_for_dtype(&templates, "Cisco IOS Switch").unwrap();
        assert_eq!(t2.fetch_config, "show running-config");

        let t3 = get_template_for_dtype(&templates, "Juniper SRX").unwrap();
        assert_eq!(t3.fetch_config, "show configuration");

        let t4 = get_template_for_dtype(&templates, "Arista EOS").unwrap();
        assert_eq!(t4.fetch_config, "show running-config");

        let t_yamaha = get_template_for_dtype(&templates, "Yamaha RTX").unwrap();
        assert_eq!(t_yamaha.fetch_config, "show config");

        // Unknown fallback
        let t_fallback = get_template_for_dtype(&templates, "unknown_vendor").unwrap();
        assert_eq!(t_fallback.fetch_config, "show running-config");
    }
}
