use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::Manager;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    pub id: String,
    pub status: String,
    pub hostname: String,
    pub ip: String,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(rename = "type")]
    pub conn_type: String,
    pub last_connected: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct McpHost {
    pub hostname: String,
    pub ip: String,
    pub device_type: String,
    pub username: String,
}

fn get_connections_path(app: &tauri::AppHandle) -> PathBuf {
    let path = app.path().app_data_dir().expect("Failed to get app data dir");
    if !path.exists() {
        let _ = fs::create_dir_all(&path);
    }
    path.join("connections.json")
}

#[tauri::command]
pub fn load_connections(app: tauri::AppHandle) -> Result<Vec<Connection>, String> {
    let path = get_connections_path(&app);
    if !path.exists() {
        return Ok(vec![]);
    }
    let data = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let connections: Vec<Connection> = serde_json::from_str(&data).map_err(|e| e.to_string())?;
    Ok(connections)
}

#[tauri::command]
pub fn save_connections(app: tauri::AppHandle, connections: Vec<Connection>) -> Result<(), String> {
    let path = get_connections_path(&app);
    let data = serde_json::to_string_pretty(&connections).map_err(|e| e.to_string())?;
    fs::write(path, data).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn get_mcp_hosts() -> Result<Vec<McpHost>, String> {
    // Mock MCP Registry
    let hosts = vec![
        McpHost {
            hostname: "Core-Switch-01".to_string(),
            ip: "192.168.1.1".to_string(),
            device_type: "SSH (Cisco IOS)".to_string(),
            username: "admin".to_string(),
        },
        McpHost {
            hostname: "Edge-Router-02".to_string(),
            ip: "192.168.2.1".to_string(),
            device_type: "SSH (Juniper JunOS)".to_string(),
            username: "root".to_string(),
        },
        McpHost {
            hostname: "Dist-Switch-03".to_string(),
            ip: "192.168.1.10".to_string(),
            device_type: "Telnet (Arista)".to_string(),
            username: "admin".to_string(),
        },
        McpHost {
            hostname: "Server-Farm-01".to_string(),
            ip: "10.0.5.50".to_string(),
            device_type: "SSH (Ubuntu)".to_string(),
            username: "root".to_string(),
        },
    ];
    Ok(hosts)
}

pub fn resolve_host_with_mcp(app: &tauri::AppHandle, host: &str) -> String {
    // 1. Check local connections first
    if let Ok(connections) = load_connections(app.clone()) {
        if let Some(conn) = connections.iter().find(|c| c.hostname.to_lowercase() == host.to_lowercase()) {
            return conn.ip.clone();
        }
    }

    // 2. Check MCP registry
    if let Ok(mcp_hosts) = get_mcp_hosts() {
        if let Some(mcp) = mcp_hosts.iter().find(|h| h.hostname.to_lowercase() == host.to_lowercase()) {
            return mcp.ip.clone();
        }
    }

    // 3. Fallback to original host (let DNS handle it)
    host.to_string()
}

pub fn get_device_config(app: &tauri::AppHandle, host: &str) -> Option<(String, String, String)> {
    // Returns (IP, Username, DeviceType)
    
    // 1. Check local connections
    if let Ok(connections) = load_connections(app.clone()) {
        if let Some(conn) = connections.iter().find(|c| c.hostname.to_lowercase() == host.to_lowercase()) {
            let dtype = if conn.conn_type.contains("Cisco IOS") { "cisco_ios" }
                        else if conn.conn_type.contains("Juniper") { "juniper_junos" }
                        else if conn.conn_type.contains("Arista") { "arista_eos" }
                        else { "cisco_ios" }; // Default
            return Some((conn.ip.clone(), "admin".to_string(), dtype.to_string()));
        }
    }

    // 2. Check MCP registry
    if let Ok(mcp_hosts) = get_mcp_hosts() {
        if let Some(mcp) = mcp_hosts.iter().find(|h| h.hostname.to_lowercase() == host.to_lowercase()) {
            let dtype = if mcp.device_type.contains("Cisco IOS") { "cisco_ios" }
                        else if mcp.device_type.contains("Juniper") { "juniper_junos" }
                        else if mcp.device_type.contains("Arista") { "arista_eos" }
                        else { "cisco_ios" };
            return Some((mcp.ip.clone(), mcp.username.clone(), dtype.to_string()));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_serialization() {
        let conn = Connection {
            id: "test-1".to_string(),
            status: "active".to_string(),
            hostname: "router-1".to_string(),
            ip: "10.0.0.1".to_string(),
            port: Some(22),
            conn_type: "SSH".to_string(),
            last_connected: "2023-10-27".to_string(),
        };

        let serialized = serde_json::to_string(&conn).unwrap();
        assert!(serialized.contains(r#""id":"test-1""#));
        assert!(serialized.contains(r#""port":22"#));
        assert!(serialized.contains(r#""type":"SSH""#));
    }

    #[test]
    fn test_mcp_host_serialization() {
        let host = McpHost {
            hostname: "switch-1".to_string(),
            ip: "10.0.0.2".to_string(),
            device_type: "Telnet".to_string(),
            username: "admin".to_string(),
        };

        let serialized = serde_json::to_string(&host).unwrap();
        assert!(serialized.contains(r#""hostname":"switch-1""#));
        assert!(serialized.contains(r#""deviceType":"Telnet""#));
    }

    #[test]
    fn test_get_mcp_hosts_returns_data() {
        let hosts = get_mcp_hosts().unwrap();
        assert!(!hosts.is_empty());
        assert_eq!(hosts[0].hostname, "Core-Switch-01");
    }
}
