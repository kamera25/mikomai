use tauri::AppHandle;

pub fn normalize_device_args(
    app: &AppHandle,
    device_name: Option<String>,
    device_name_camel: Option<String>,
    device: Option<String>,
    host: Option<String>,
    user_message: Option<String>,
    user_message_camel: Option<String>,
) -> Result<String, String> {
    // 1. Identify if a device name has been passed
    let mut name = device_name
        .or(device_name_camel)
        .or(device)
        .or(host);

    // 2. If empty, check if we can scan user_message
    if name.as_ref().map_or(true, |n| n.trim().is_empty()) {
        let msg = user_message.or(user_message_camel);
        if let Some(user_msg) = msg {
            if !user_msg.trim().is_empty() {
                if let Some(extracted) = resolve_device_from_connections(app, &user_msg) {
                    name = Some(extracted);
                }
            }
        }
    }

    // 3. Fallback to settings recent_ips if still empty
    if name.as_ref().map_or(true, |n| n.trim().is_empty()) {
        if let Some(first_recent) = crate::settings::load_settings(app.clone())
            .ok()
            .and_then(|settings| settings.recent_ips.first().cloned())
            .filter(|ip| !ip.trim().is_empty())
        {
            name = Some(first_recent);
        }
    }

    name.filter(|n| !n.trim().is_empty())
        .ok_or_else(|| "Error: device_name is required but was not provided or is empty.".to_string())
}

pub fn normalize_host_args(
    app: &AppHandle,
    host: Option<String>,
    device: Option<String>,
    device_name_camel: Option<String>,
    device_name: Option<String>,
    ip: Option<String>,
) -> Result<String, String> {
    let mut target = host
        .or(device)
        .or(device_name_camel)
        .or(device_name)
        .or(ip);

    if target.as_ref().map_or(true, |t| t.trim().is_empty()) {
        if let Some(first_recent) = crate::settings::load_settings(app.clone())
            .ok()
            .and_then(|settings| settings.recent_ips.first().cloned())
            .filter(|ip| !ip.trim().is_empty())
        {
            target = Some(first_recent);
        }
    }

    target.filter(|t| !t.trim().is_empty())
        .ok_or_else(|| "Error: host is required but was not provided or is empty.".to_string())
}

fn resolve_device_from_connections(app: &AppHandle, user_message: &str) -> Option<String> {
    let lower_msg = user_message.to_lowercase();
    if let Ok(connections) = crate::connections::load_connections_raw(app) {
        for conn in connections {
            if (!conn.hostname.as_str().is_empty() && lower_msg.contains(&conn.hostname.to_lowercase()))
                || (!conn.ip.as_str().is_empty() && lower_msg.contains(conn.ip.as_str()))
            {
                return Some(conn.hostname.to_string());
            }
        }
    }
    if let Ok(mcp_hosts) = crate::connections::get_mcp_hosts() {
        for host in mcp_hosts {
            if (!host.hostname.as_str().is_empty() && lower_msg.contains(&host.hostname.to_lowercase()))
                || (!host.ip.as_str().is_empty() && lower_msg.contains(host.ip.as_str()))
            {
                return Some(host.hostname.to_string());
            }
        }
    }
    None
}
