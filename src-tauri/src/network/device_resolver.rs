use crate::network::{NetmikoDeviceConfig, NetworkError};
use crate::connections::{get_device_config, load_connections, ConnectionType, resolve_host_with_preference};
use crate::settings::load_settings;
use tauri::AppHandle;

#[derive(Debug, Clone)]
pub struct TargetDevice {
    host: String,
    username: String,
    password: Option<String>,
    enable_password: Option<String>,
    device_type: String,
    console_port: Option<String>,
    console_baud_rate: Option<u32>,
}

impl TargetDevice {
    pub fn host(&self) -> &str {
        &self.host
    }

    #[allow(dead_code)]
    pub fn username(&self) -> &str {
        &self.username
    }

    #[allow(dead_code)]
    pub fn password(&self) -> Option<&str> {
        self.password.as_deref()
    }

    #[allow(dead_code)]
    pub fn enable_password(&self) -> Option<&str> {
        self.enable_password.as_deref()
    }

    #[allow(dead_code)]
    pub fn device_type(&self) -> &str {
        &self.device_type
    }

    pub fn console_port(&self) -> Option<&str> {
        self.console_port.as_deref()
    }

    #[allow(dead_code)]
    pub fn console_baud_rate(&self) -> Option<u32> {
        self.console_baud_rate
    }

    pub fn to_netmiko_config(&self) -> NetmikoDeviceConfig {
        NetmikoDeviceConfig {
            host: self.host.clone(),
            username: self.username.clone(),
            password: self.password.clone(),
            enable_password: self.enable_password.clone(),
            device_type: self.device_type.clone(),
            console_port: self.console_port.clone(),
            console_baud_rate: self.console_baud_rate,
        }
    }
}

pub struct TargetDeviceBuilder {
    app: AppHandle,
    device: NetmikoDeviceConfig,
}

impl TargetDeviceBuilder {
    pub fn new(app: AppHandle, device: NetmikoDeviceConfig) -> Self {
        Self { app, device }
    }

    pub async fn resolve(self) -> Result<TargetDevice, NetworkError> {
        let mut target_device = self.device;

        // 1. Try to resolve it from MCP/Connections, falling back to passed-in device if not found
        if let Some((ip, user, password, enable_password, dtype)) = get_device_config(&self.app, &target_device.host) {
            target_device.host = ip;
            target_device.username = user;
            if password.is_some() {
                target_device.password = password;
            }
            if enable_password.is_some() {
                target_device.enable_password = enable_password;
            }
            target_device.device_type = dtype;
        }

        // 2. Load settings for console override if connection type is console/serial
        let mut is_console = target_device.console_port.is_some();
        if !is_console {
            if let Ok(connections) = load_connections(self.app.clone()) {
                if let Some(conn) = connections.iter().find(|c| c.hostname.eq_ignore_ascii_case(&target_device.host) || c.ip.as_str() == target_device.host) {
                    if conn.conn_type == ConnectionType::Console {
                        is_console = true;
                    }
                }
            }
        }

        if is_console {
            let settings = load_settings(self.app.clone()).unwrap_or_default();
            if let Some(ref port) = settings.console_port {
                if !port.trim().is_empty() && port != "None" {
                    target_device.console_port = Some(port.clone());
                    target_device.console_baud_rate = settings.console_baud_rate;
                }
            }
        } else {
            target_device.console_port = None;
            target_device.console_baud_rate = None;
        }

        // 3. Resolve using preference if not console
        if target_device.console_port.is_none() {
            let host_to_resolve = target_device.host.clone();
            let app_clone = self.app.clone();
            let ip = tokio::task::spawn_blocking(move || {
                resolve_host_with_preference(&app_clone, &host_to_resolve)
            })
            .await
            .map_err(|e| NetworkError::SpawnBlocking(e.to_string()))??;
            target_device.host = ip.to_string();
        }

        Ok(TargetDevice {
            host: target_device.host,
            username: target_device.username,
            password: target_device.password,
            enable_password: target_device.enable_password,
            device_type: target_device.device_type,
            console_port: target_device.console_port,
            console_baud_rate: target_device.console_baud_rate,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_target_device_getters_and_conversion() {
        let device = TargetDevice {
            host: "10.0.0.1".to_string(),
            username: "admin".to_string(),
            password: Some("pass".to_string()),
            enable_password: Some("secret".to_string()),
            device_type: "cisco_ios".to_string(),
            console_port: Some("COM3".to_string()),
            console_baud_rate: Some(9600),
        };

        assert_eq!(device.host(), "10.0.0.1");
        assert_eq!(device.username(), "admin");
        assert_eq!(device.password(), Some("pass"));
        assert_eq!(device.enable_password(), Some("secret"));
        assert_eq!(device.device_type(), "cisco_ios");
        assert_eq!(device.console_port(), Some("COM3"));
        assert_eq!(device.console_baud_rate(), Some(9600));

        let config = device.to_netmiko_config();
        assert_eq!(config.host, "10.0.0.1");
        assert_eq!(config.username, "admin");
        assert_eq!(config.password, Some("pass".to_string()));
        assert_eq!(config.enable_password, Some("secret".to_string()));
        assert_eq!(config.device_type, "cisco_ios");
        assert_eq!(config.console_port, Some("COM3".to_string()));
        assert_eq!(config.console_baud_rate, Some(9600));
    }
}

