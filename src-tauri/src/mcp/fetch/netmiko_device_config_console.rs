use crate::network::NetmikoDeviceConfig;
use crate::settings::load_settings;

pub fn resolve_console_device_config(
    app: &tauri::AppHandle,
    host: String,
    username: String,
    password: Option<String>,
    enable_password: Option<String>,
    device_type: String,
) -> Result<NetmikoDeviceConfig, String> {
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
            { port = Some("COM1".to_string()); }
            #[cfg(not(target_os = "windows"))]
            { port = Some("/dev/ttyUSB0".to_string()); }
        }
    }
    
    Ok(NetmikoDeviceConfig {
        host,
        username,
        password,
        enable_password,
        device_type,
        console_port: port,
        console_baud_rate: settings.console_baud_rate,
    })
}
