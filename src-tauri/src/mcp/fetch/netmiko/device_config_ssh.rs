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

        let device = device?;
        
        Ok(NetmikoDeviceConfig {
            host: device.ip,
            username: device.username,
            password: device.password,
            enable_password: device.enable_password,
            device_type: device.device_type,
            console_port: None,
            console_baud_rate: None,
            auth_method: device.auth_method,
            private_key_path: device.private_key_path,
            passphrase: device.passphrase,
            agent_forwarding: device.agent_forwarding,
        })
    }
}
