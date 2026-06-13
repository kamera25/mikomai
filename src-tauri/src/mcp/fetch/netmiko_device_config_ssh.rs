use crate::network::NetmikoDeviceConfig;

pub fn resolve_ssh_device_config(
    host: String,
    username: String,
    password: Option<String>,
    enable_password: Option<String>,
    device_type: String,
) -> Result<NetmikoDeviceConfig, String> {
    Ok(NetmikoDeviceConfig {
        host,
        username,
        password,
        enable_password,
        device_type,
        console_port: None,
        console_baud_rate: None,
    })
}
