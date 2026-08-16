use crate::connections::load_connections_raw;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeviceContext
{
    pub hostname: String,
    pub ip: String,
    pub vendor: String,
    pub device_type: String,
    pub conn_type: String,
}

pub fn resolve_device_contexts<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    text: &str,
) -> Vec<DeviceContext>
{
    let text_lower = text.to_lowercase();
    let mut matched = Vec::new();

    if let Ok(connections) = load_connections_raw(app)
    {
        for conn in connections
        {
            let hostname = conn.hostname.as_str().to_lowercase();
            let ip = conn.ip.to_string();
            // Check if either hostname (if not empty) or IP is mentioned in the text
            if (!hostname.is_empty() && text_lower.contains(&hostname)) || text_lower.contains(&ip)
            {
                let vendor = conn
                    .vendor_type
                    .as_ref()
                    .map(|v| v.as_str().to_string())
                    .unwrap_or_else(|| "Unknown".to_string());
                let device_type = conn
                    .device_type
                    .as_ref()
                    .map(|d| d.as_str().to_string())
                    .unwrap_or_else(|| "Unknown".to_string());
                let conn_type = format!("{:?}", conn.conn_type);

                matched.push(DeviceContext {
                    hostname: conn.hostname.as_str().to_string(),
                    ip: conn.ip,
                    vendor,
                    device_type,
                    conn_type,
                });

                if matched.len() >= 3
                {
                    break;
                }
            }
        }
    }
    matched
}

pub fn format_device_contexts(contexts: &[DeviceContext]) -> String
{
    if contexts.is_empty()
    {
        return String::new();
    }

    let mut info = String::from("### Registered Device Information (System Context) ###\n");
    for (i, ctx) in contexts.iter().enumerate()
    {
        info.push_str(&format!(
            "Device {}:\n  - Hostname: {}\n  - IP Address: {}\n  - Vendor: {}\n  - Device Type: {}\n  - Connection Type: {}\n",
            i + 1,
            ctx.hostname,
            ctx.ip,
            ctx.vendor,
            ctx.device_type,
            ctx.conn_type
        ));
    }
    info.push_str("#####################################################\n\n");
    info
}
