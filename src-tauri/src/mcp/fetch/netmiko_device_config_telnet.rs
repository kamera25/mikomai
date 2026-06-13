use crate::network::NetmikoDeviceConfig;

pub fn resolve_telnet_device_config(
    host: String,
    username: String,
    password: Option<String>,
    enable_password: Option<String>,
    mut device_type: String,
) -> Result<NetmikoDeviceConfig, String> {
    if !device_type.ends_with("_telnet") {
        device_type = format!("{}_telnet", device_type);
    }
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
