use crate::network::NetmikoDeviceConfig;
use crate::settings::load_settings;

pub trait ConsoleDeviceConfigBuilder {
    fn build(
        &self,
        app: &tauri::AppHandle,
        resolved_name: &str,
    ) -> Result<NetmikoDeviceConfig, String>;
}

pub struct ConsoleBuilder;

impl ConsoleDeviceConfigBuilder for ConsoleBuilder {
    fn build(
        &self,
        app: &tauri::AppHandle,
        resolved_name: &str,
    ) -> Result<NetmikoDeviceConfig, String> {
        let device = crate::mcp::fetch::fetch_base::find_device(app, resolved_name)?;

        let settings = load_settings(app.clone()).unwrap_or_default();
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
                {
                    port = Some("COM1".to_string());
                }
                #[cfg(not(target_os = "windows"))]
                {
                    port = Some("/dev/ttyUSB0".to_string());
                }
            }
        }

        Ok(NetmikoDeviceConfig {
            host: device.ip,
            username: device.username,
            password: device.password,
            enable_password: device.enable_password,
            device_type: device.device_type,
            console_port: port,
            console_baud_rate: settings.console_baud_rate,
            auth_method: None,
            private_key_path: None,
            passphrase: None,
            agent_forwarding: None,
        })
    }
}
