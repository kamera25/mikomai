use tauri::AppHandle;
use crate::connections::{Connection, McpHost};

pub fn get_registered_device_info(query: &str, app: &AppHandle) -> Option<String> {
    let connections = crate::connections::load_connections(app.clone()).unwrap_or_default();
    let mcp_hosts = crate::connections::get_mcp_hosts().unwrap_or_default();
    get_registered_device_info_from_lists(query, &connections, &mcp_hosts)
}

pub fn get_registered_device_info_from_lists(
    query: &str,
    connections: &[Connection],
    mcp_hosts: &[McpHost],
) -> Option<String> {
    let target = query.trim().to_lowercase();
    
    // Check local connections
    if let Some(conn) = connections.iter().find(|c| c.hostname.eq_ignore_ascii_case(&target) || c.ip.as_str() == target) {
        let mut info = format!("登録済み機器 '{}' の接続情報:\n\n", conn.hostname);
        info.push_str(&format!("- ホスト名: {}\n", conn.hostname));
        info.push_str(&format!("- IPアドレス: {}\n", conn.ip));
        if let Some(port) = conn.port {
            info.push_str(&format!("- ポート番号: {}\n", port));
        }
        info.push_str(&format!("- 接続タイプ: {}\n", conn.conn_type));
        if let Some(user) = &conn.username {
            info.push_str(&format!("- ユーザー名: {}\n", user));
        }
        if let Some(device_type) = &conn.device_type {
            info.push_str(&format!("- 機器タイプ: {}\n", device_type));
        }
        if let Some(vendor_type) = &conn.vendor_type {
            info.push_str(&format!("- ベンダー: {}\n", vendor_type));
        }
        info.push_str(&format!("- ステータス: {}\n", conn.status));
        info.push_str(&format!("- 最終接続日: {}\n", conn.last_connected));
        return Some(info);
    }

    // Check MCP hosts
    if let Some(host) = mcp_hosts.iter().find(|h| h.hostname.eq_ignore_ascii_case(&target) || h.ip.as_str() == target) {
        let mut info = format!("登録済み機器 '{}' の接続情報 (MCPレジストリ):\n\n", host.hostname);
        info.push_str(&format!("- ホスト名: {}\n", host.hostname));
        info.push_str(&format!("- IPアドレス: {}\n", host.ip));
        info.push_str(&format!("- 機器タイプ: {}\n", host.device_type));
        info.push_str(&format!("- ユーザー名: {}\n", host.username));
        return Some(info);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_registered_device_info_from_lists_match_hostname() {
        let connections = vec![
            Connection {
                id: crate::connections::ConnectionId::try_from("1").unwrap(),
                status: crate::connections::ConnectionStatus::try_from("active").unwrap(),
                hostname: crate::connections::Hostname::try_from("router-cisco").unwrap(),
                ip: crate::connections::IpAddress::try_from("192.168.1.1").unwrap(),
                port: Some(22),
                conn_type: crate::connections::ConnectionType::try_from("SSH").unwrap(),
                last_connected: crate::connections::LastConnected::try_from("2026-06-11").unwrap(),
                username: Some(crate::connections::Username::try_from("admin").unwrap()),
                password: None,
                enable_password: None,
                device_type: Some(crate::connections::DeviceType::try_from("Router").unwrap()),
                vendor_type: Some(crate::connections::VendorType::try_from("Cisco").unwrap()),
            }
        ];
        let mcp_hosts = vec![];

        let result = get_registered_device_info_from_lists("router-cisco", &connections, &mcp_hosts);
        assert!(result.is_some());
        let info = result.unwrap();
        assert!(info.contains("登録済み機器 'router-cisco' の接続情報"));
        assert!(info.contains("- IPアドレス: 192.168.1.1"));
        assert!(info.contains("- ベンダー: Cisco"));

        // Case-insensitive match check
        let result_caps = get_registered_device_info_from_lists("ROUTER-CISCO", &connections, &mcp_hosts);
        assert!(result_caps.is_some());
    }

    #[test]
    fn test_get_registered_device_info_from_lists_match_ip() {
        let connections = vec![];
        let mcp_hosts = vec![
            McpHost {
                hostname: crate::connections::Hostname::try_from("switch-juniper").unwrap(),
                ip: crate::connections::IpAddress::try_from("192.168.1.2").unwrap(),
                device_type: crate::connections::DeviceType::try_from("Switch").unwrap(),
                username: crate::connections::Username::try_from("juniper-user").unwrap(),
            }
        ];

        let result = get_registered_device_info_from_lists("192.168.1.2", &connections, &mcp_hosts);
        assert!(result.is_some());
        let info = result.unwrap();
        assert!(info.contains("登録済み機器 'switch-juniper' の接続情報 (MCPレジストリ)"));
        assert!(info.contains("- ユーザー名: juniper-user"));
    }

    #[test]
    fn test_get_registered_device_info_from_lists_no_match() {
        let connections = vec![];
        let mcp_hosts = vec![];

        let result = get_registered_device_info_from_lists("unknown-host", &connections, &mcp_hosts);
        assert!(result.is_none());
    }
}
