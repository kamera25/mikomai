use crate::connections::{load_connections, get_mcp_hosts};
use crate::network::{NetmikoDeviceConfig, CommandResult};
use crate::mcp::fetch::netmiko::connection_wraper::NetmikoConnectionWrapper;
use super::ConnectionType;

pub use super::command_template::{CommandTemplate, CommandTemplates, load_templates, get_default_templates, get_templates_path, get_template_for_dtype, map_vendor_type};



pub struct ResolvedDevice {
    pub ip: String,
    pub username: String,
    pub password: Option<String>,
    pub enable_password: Option<String>,
    pub device_type: String,
}

pub fn find_device(app: &tauri::AppHandle, resolved_name: &str) -> Result<ResolvedDevice, String> {
    let mut resolved_device = None;
    
    if let Ok(connections) = load_connections(app.clone()) {
        if let Some(conn) = connections.iter().find(|c| c.hostname.eq_ignore_ascii_case(resolved_name) || c.ip.as_str() == resolved_name) {
            let dtype = if let Some(dt) = &conn.device_type {
                dt.to_string()
            } else if let Some(vt) = &conn.vendor_type {
                map_vendor_type(vt.as_str())
            } else {
                "cisco_ios".to_string()
            };

            let user = conn.username.as_ref().map(|u| u.to_string()).unwrap_or_else(|| "admin".to_string());
            resolved_device = Some(ResolvedDevice {
                ip: conn.ip.to_string(),
                username: user,
                password: conn.password.as_ref().map(|p| p.to_string()),
                enable_password: conn.enable_password.as_ref().map(|ep| ep.to_string()),
                device_type: dtype,
            });
        }
    }
    
    if resolved_device.is_none() {
        if let Ok(mcp_hosts) = get_mcp_hosts() {
            if let Some(mcp) = mcp_hosts.iter().find(|h| h.hostname.eq_ignore_ascii_case(resolved_name) || h.ip.as_str() == resolved_name) {
                let dtype = map_vendor_type(mcp.device_type.as_str());

                resolved_device = Some(ResolvedDevice {
                    ip: mcp.ip.to_string(),
                    username: mcp.username.to_string(),
                    password: None,
                    enable_password: None,
                    device_type: dtype,
                });
            }
        }
    }

    match resolved_device {
        Some(d) => Ok(d),
        None => Err(format!("Error: Device '{}' is not registered. Only registered device names are allowed.", resolved_name)),
    }
}

pub async fn resolve_device_config(app: &tauri::AppHandle, device_name: &str) -> Result<NetmikoDeviceConfig, String> {
    let (resolved_name, conn_type) = super::device_resolver::resolve_device_name_and_type(app, device_name)?;

    match conn_type {
        ConnectionType::Console => {
            use super::netmiko::device_config_console::{ConsoleDeviceConfigBuilder, ConsoleBuilder};
            ConsoleBuilder.build(app, &resolved_name)
        }
        ConnectionType::Telnet => {
            use super::netmiko::device_config_telnet::{TelnetDeviceConfigBuilder, TelnetBuilder};
            TelnetBuilder.build(app, &resolved_name)
        }
        ConnectionType::SSH => {
            use super::netmiko::device_config_ssh::{SshDeviceConfigBuilder, SshBuilder};
            SshBuilder.build(app, &resolved_name)
        }
    }
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
            log::info!("Fetching {} for registered device '{}' via console port '{}' using command '{}'", self.get_log_prefix(), device_name, port, command);
        } else {
            log::info!("Fetching {} for registered device '{}' using command '{}'", self.get_log_prefix(), device_name, command);
        }


        let wrapper = NetmikoConnectionWrapper::new(app);
        match wrapper.execute_show(&target_device, &command).await {
            Ok(output) => {
                let saved_path: Option<String> = if !output.trim().is_empty() {
                    if let Ok(mut manager) = crate::snapshot::SnapshotManager::new(app) {
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
                Ok(CommandResult { success: true, output, saved_path, is_cached: None, cache_time: None })
            }
            Err(err) => Ok(CommandResult { success: false, output: err, saved_path: None, is_cached: None, cache_time: None }),
        }
    }
}

pub fn check_yaml_cache(
    app: &tauri::AppHandle,
    registered_name: &str,
    suffix: &str,
) -> Option<CommandResult> {
    let settings = crate::settings::load_settings(app.clone()).ok()?;
    let expiry_mins = settings.cache_expiry_minutes.unwrap_or(10);
    if expiry_mins == 0 {
        return None;
    }

    let manager = crate::snapshot::SnapshotManager::new(app).ok()?;
    let yaml_path = manager.base_dir().join("current").join(format!("{}_{}.yaml", registered_name, suffix));
    if !yaml_path.exists() {
        return None;
    }

    let metadata = std::fs::metadata(&yaml_path).ok()?;
    let modified = metadata.modified().ok()?;
    let elapsed = modified.elapsed().ok()?;
    if elapsed.as_secs() >= expiry_mins * 60 {
        return None;
    }

    let yaml_content = std::fs::read_to_string(&yaml_path).ok()?;
    log::info!("Returning cached {} YAML for {} (last updated {}s ago)", suffix, registered_name, elapsed.as_secs());

    let datetime: chrono::DateTime<chrono::Local> = modified.into();
    let cache_time_str = datetime.format("%Y-%m-%d %H:%M:%S").to_string();

    Some(CommandResult {
        success: true,
        output: yaml_content,
        saved_path: Some(yaml_path.to_string_lossy().to_string()),
        is_cached: Some(true),
        cache_time: Some(cache_time_str),
    })
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
