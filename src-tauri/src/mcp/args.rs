use crate::connections::IpAddress;
use tauri::{AppHandle, Runtime};

pub fn normalize_device_args<R: Runtime>(
    app: &AppHandle<R>,
    device_name: Option<String>,
    device_name_camel: Option<String>,
    device: Option<String>,
    host: Option<String>,
    user_message: Option<String>,
    user_message_camel: Option<String>,
) -> Result<String, String>
{
    // 1. Identify if a device name has been passed
    let mut name = device_name.or(device_name_camel).or(device).or(host);

    // 2. If empty, check if we can scan user_message
    if name.as_ref().map_or(true, |n| n.trim().is_empty())
    {
        let msg = user_message.or(user_message_camel);
        if let Some(user_msg) = msg
        {
            if !user_msg.trim().is_empty()
            {
                if let Some(extracted) = resolve_device_from_connections(app, &user_msg)
                {
                    name = Some(extracted);
                }
            }
        }
    }

    // 3. Fallback to settings recent_ips if still empty
    if name.as_ref().map_or(true, |n| n.trim().is_empty())
    {
        if let Some(first_recent) = crate::settings::load_settings(app.clone())
            .ok()
            .and_then(|settings| settings.recent_ips.first().cloned())
            .filter(|ip| !ip.trim().is_empty())
        {
            name = Some(first_recent);
        }
    }

    name.filter(|n| !n.trim().is_empty()).ok_or_else(|| {
        "Error: device_name is required but was not provided or is empty.".to_string()
    })
}

pub fn normalize_host_args<R: Runtime>(
    app: &AppHandle<R>,
    host: Option<String>,
    device: Option<String>,
    device_name_camel: Option<String>,
    device_name: Option<String>,
    ip: Option<IpAddress>,
) -> Result<String, String>
{
    let mut target = host
        .or(device)
        .or(device_name_camel)
        .or(device_name)
        .or_else(|| ip.map(|i| i.to_string()));

    if target.as_ref().map_or(true, |t| t.trim().is_empty())
    {
        if let Some(first_recent) = crate::settings::load_settings(app.clone())
            .ok()
            .and_then(|settings| settings.recent_ips.first().cloned())
            .filter(|ip| !ip.trim().is_empty())
        {
            target = Some(first_recent);
        }
    }

    target
        .filter(|t| !t.trim().is_empty())
        .ok_or_else(|| "Error: host is required but was not provided or is empty.".to_string())
}

pub async fn resolve_target_ip<R: Runtime>(
    app: &tauri::AppHandle<R>,
    target_host: &str,
) -> Result<std::net::IpAddr, String>
{
    let resolved_host = crate::connections::resolve_host_with_mcp(app, target_host);
    let app_clone = app.clone();
    tokio::task::spawn_blocking(move || {
        crate::connections::resolve_host_with_preference(&app_clone, &resolved_host)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}

pub async fn resolve_target_host_string<R: Runtime>(
    app: &tauri::AppHandle<R>,
    target_host: &str,
) -> Result<String, String>
{
    resolve_target_ip(app, target_host)
        .await
        .map(|ip| ip.to_string())
}

fn resolve_device_from_connections<R: Runtime>(
    app: &AppHandle<R>,
    user_message: &str,
) -> Option<String>
{
    let lower_msg = user_message.to_lowercase();
    if let Ok(connections) = crate::connections::load_connections_raw(app)
    {
        for conn in connections
        {
            if (!conn.hostname.as_str().is_empty()
                && lower_msg.contains(&conn.hostname.to_lowercase()))
                || lower_msg.contains(&conn.ip.to_string())
            {
                return Some(conn.hostname.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests
{
    use super::*;
    use tauri::test::mock_app;

    #[test]
    fn test_normalize_device_args_direct()
    {
        let app = mock_app();
        let handle = app.handle();

        // 1. Direct device name passing
        let res = normalize_device_args(
            handle,
            Some("router-1".to_string()),
            None,
            None,
            None,
            None,
            None,
        );
        assert_eq!(res.unwrap(), "router-1");

        // 2. Direct device_name_camel
        let res = normalize_device_args(
            handle,
            None,
            Some("router-camel".to_string()),
            None,
            None,
            None,
            None,
        );
        assert_eq!(res.unwrap(), "router-camel");

        // 3. Fallback when everything is empty (should fail if settings/connections are empty)
        let res = normalize_device_args(handle, None, None, None, None, None, None);
        assert!(res.is_err());
    }

    #[test]
    fn test_normalize_host_args_direct()
    {
        let app = mock_app();
        let handle = app.handle();

        let res = normalize_host_args(
            handle,
            Some("192.168.1.1".to_string()),
            None,
            None,
            None,
            None,
        );
        assert_eq!(res.unwrap(), "192.168.1.1");

        let res = normalize_host_args(handle, None, None, None, None, None);
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn test_resolve_target_ip_loopback()
    {
        let app = mock_app();
        let handle = app.handle();

        let res = resolve_target_ip(handle, "127.0.0.1").await;
        assert!(res.is_ok());
        assert_eq!(res.unwrap().to_string(), "127.0.0.1");

        let str_res = resolve_target_host_string(handle, "127.0.0.1").await;
        assert!(str_res.is_ok());
        assert_eq!(str_res.unwrap(), "127.0.0.1");
    }
}
