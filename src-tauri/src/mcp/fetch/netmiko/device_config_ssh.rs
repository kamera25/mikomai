use crate::network::NetmikoDeviceConfig;

pub trait SshDeviceConfigBuilder {
    fn build(
        &self,
        app: &tauri::AppHandle,
        resolved_name: &str,
    ) -> Result<NetmikoDeviceConfig, String>;
}

pub struct SshBuilder;

impl SshDeviceConfigBuilder for SshBuilder {
    fn build(
        &self,
        app: &tauri::AppHandle,
        resolved_name: &str,
    ) -> Result<NetmikoDeviceConfig, String> {
        let device = crate::mcp::fetch::fetch_base::find_device(app, resolved_name);
        if resolved_name.parse::<std::net::IpAddr>().is_ok() && device.is_err() {
            return Err("IP address input is not allowed. Please specify the registered device name.".to_string());
        }
        let device = device?;
        
        Ok(NetmikoDeviceConfig {
            host: device.ip,
            username: device.username,
            password: device.password,
            enable_password: device.enable_password,
            device_type: device.device_type,
            console_port: None,
            console_baud_rate: None,
        })
    }
}
