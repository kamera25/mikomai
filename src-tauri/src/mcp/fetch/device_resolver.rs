use crate::connections::{load_connections, ConnectionType};

pub fn resolve_device_name_and_type(
    app: &tauri::AppHandle,
    device_name: &str,
) -> Result<(String, ConnectionType), String>
{
    let resolved_name = if !device_name.trim().is_empty()
    {
        device_name.to_string()
    }
    else if let Some(conn_name) = load_connections(app.clone())
        .ok()
        .and_then(|conns| {
            conns
                .iter()
                .find(|c| c.conn_type == ConnectionType::Console)
                .map(|c| c.hostname.to_string())
                .or_else(|| {
                    conns
                        .iter()
                        .find(|c| !c.hostname.as_str().trim().is_empty())
                        .map(|c| c.hostname.to_string())
                })
        })
        .filter(|name| !name.trim().is_empty())
    {
        conn_name
    }
    else if let Some(first_recent) = crate::settings::load_settings(app.clone())
        .ok()
        .and_then(|settings| settings.recent_ips.first().cloned())
        .filter(|ip| !ip.trim().is_empty())
    {
        first_recent
    }
    else
    {
        return Err(
            "Error: device_name (機器名) is required but was not provided or is empty.".to_string(),
        );
    };

    let conn_type = detect_connection_type(app, &resolved_name);
    Ok((resolved_name, conn_type))
}

pub fn detect_connection_type(app: &tauri::AppHandle, resolved_name: &str) -> ConnectionType
{
    if let Ok(connections) = load_connections(app.clone())
    {
        if let Some(conn) = connections.iter().find(|c| {
            c.hostname.eq_ignore_ascii_case(resolved_name) || c.ip.to_string() == resolved_name
        })
        {
            return conn.conn_type;
        }
    }
    ConnectionType::SSH
}
