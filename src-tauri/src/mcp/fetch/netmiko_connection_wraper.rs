use crate::network::{SidecarNetmikoWrapper, DeviceConfig, NetworkInterface};
use tauri::AppHandle;

/// A wrapper around `SidecarNetmikoWrapper` to encapsulate Netmiko-specific connection processes.
pub struct NetmikoConnectionWrapper {
    wrapper: SidecarNetmikoWrapper,
}

impl NetmikoConnectionWrapper {
    /// Creates a new `NetmikoConnectionWrapper` using the provided Tauri application handle.
    pub fn new(app: &AppHandle) -> Self {
        Self {
            wrapper: SidecarNetmikoWrapper::new(app),
        }
    }

    /// Executes a show command on the target device via Netmiko.
    pub async fn execute_show(
        &self,
        device: &DeviceConfig,
        command: &str,
    ) -> Result<String, String> {
        self.wrapper.execute_show(device, command).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_config_validation() {
        let config = DeviceConfig {
            host: "192.168.1.1".to_string(),
            username: "admin".to_string(),
            password: Some("admin123".to_string()),
            enable_password: Some("secret123".to_string()),
            device_type: "cisco_ios".to_string(),
            console_port: None,
            console_baud_rate: None,
        };

        assert_eq!(config.host, "192.168.1.1");
        assert_eq!(config.username, "admin");
        assert_eq!(config.password, Some("admin123".to_string()));
        assert_eq!(config.enable_password, Some("secret123".to_string()));
        assert_eq!(config.device_type, "cisco_ios");
        assert!(config.console_port.is_none());
        assert!(config.console_baud_rate.is_none());
    }

    #[test]
    fn test_device_config_with_console() {
        let config = DeviceConfig {
            host: "".to_string(),
            username: "admin".to_string(),
            password: None,
            enable_password: None,
            device_type: "cisco_ios".to_string(),
            console_port: Some("/dev/ttyUSB0".to_string()),
            console_baud_rate: Some(9600),
        };

        assert_eq!(config.username, "admin");
        assert_eq!(config.device_type, "cisco_ios");
        assert_eq!(config.console_port, Some("/dev/ttyUSB0".to_string()));
        assert_eq!(config.console_baud_rate, Some(9600));
    }
}
