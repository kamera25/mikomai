use crate::network::NetmikoDeviceConfig;

pub trait TelnetDeviceConfigBuilder {
    fn build(
        &self,
        app: &tauri::AppHandle,
        resolved_name: &str,
    ) -> Result<NetmikoDeviceConfig, String>;
}

pub struct TelnetBuilder;

impl TelnetDeviceConfigBuilder for TelnetBuilder {
    fn build(
        &self,
        app: &tauri::AppHandle,
        resolved_name: &str,
    ) -> Result<NetmikoDeviceConfig, String> {
        let device = crate::mcp::fetch::fetch_base::find_device(app, resolved_name);

        let device = device?;
        
        let mut device_type = device.device_type;
        if !device_type.ends_with("_telnet") {
            device_type = format!("{}_telnet", device_type);
        }
        
        Ok(NetmikoDeviceConfig {
            host: device.ip,
            username: device.username,
            password: device.password,
            enable_password: device.enable_password,
            device_type,
            console_port: None,
            console_baud_rate: None,
        })
    }
}
