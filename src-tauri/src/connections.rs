use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::Manager;
use crate::crypto::{encrypt, decrypt};

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
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub enable_password: Option<String>,
    #[serde(default)]
    pub device_type: Option<String>,
    #[serde(default)]
    pub vendor_type: Option<String>,
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
    let mut connections: Vec<Connection> = serde_json::from_str(&data).map_err(|e| e.to_string())?;

    // Decrypt passwords when loading
    for conn in &mut connections {
        if let Some(encrypted_password) = &conn.password {
            if !encrypted_password.is_empty() {
                match decrypt(&app, encrypted_password) {
                    Ok(decrypted) => conn.password = Some(decrypted),
                    Err(e) => eprintln!("Failed to decrypt password for connection {}: {}", conn.id, e),
                }
            }
        }
        if let Some(encrypted_enable_password) = &conn.enable_password {
            if !encrypted_enable_password.is_empty() {
                match decrypt(&app, encrypted_enable_password) {
                    Ok(decrypted) => conn.enable_password = Some(decrypted),
                    Err(e) => eprintln!("Failed to decrypt enable password for connection {}: {}", conn.id, e),
                }
            }
        }
    }

    Ok(connections)
}

#[tauri::command]
pub fn save_connections(app: tauri::AppHandle, mut connections: Vec<Connection>) -> Result<(), String> {
    let path = get_connections_path(&app);

    // Encrypt passwords before saving
    for conn in &mut connections {
        if let Some(plain_password) = &conn.password {
            if !plain_password.is_empty() {
                match encrypt(&app, plain_password) {
                    Ok(encrypted) => conn.password = Some(encrypted),
                    Err(e) => return Err(format!("Failed to encrypt password for connection {}: {}", conn.id, e)),
                }
            }
        }
        if let Some(plain_enable_password) = &conn.enable_password {
            if !plain_enable_password.is_empty() {
                match encrypt(&app, plain_enable_password) {
                    Ok(encrypted) => conn.enable_password = Some(encrypted),
                    Err(e) => return Err(format!("Failed to encrypt enable password for connection {}: {}", conn.id, e)),
                }
            }
        }
    }

    let data = serde_json::to_string_pretty(&connections).map_err(|e| e.to_string())?;
    fs::write(path, data).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn get_mcp_hosts() -> Result<Vec<McpHost>, String> {
    // Return empty list as mock is no longer needed
    Ok(vec![])
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

pub fn get_device_config(app: &tauri::AppHandle, host: &str) -> Option<(String, String, Option<String>, Option<String>, String)> {
    // Returns (IP, Username, Password, EnablePassword, DeviceType)
    
    // 1. Check local connections
    if let Ok(connections) = load_connections(app.clone()) {
        if let Some(conn) = connections.iter().find(|c| c.hostname.to_lowercase() == host.to_lowercase() || c.ip == host) {
            let dtype = if let Some(dt) = &conn.device_type {
                dt.clone()
            } else if conn.conn_type.contains("Cisco IOS") { "cisco_ios".to_string() }
                        else if conn.conn_type.contains("Juniper") { "juniper_junos".to_string() }
                        else if conn.conn_type.contains("Arista") { "arista_eos".to_string() }
                        else { "cisco_ios".to_string() }; // Default

            let user = conn.username.clone().unwrap_or_else(|| "admin".to_string());
            return Some((conn.ip.clone(), user, conn.password.clone(), conn.enable_password.clone(), dtype));
        }
    }

    // 2. Check MCP registry
    if let Ok(mcp_hosts) = get_mcp_hosts() {
        if let Some(mcp) = mcp_hosts.iter().find(|h| h.hostname.to_lowercase() == host.to_lowercase() || h.ip == host) {
            let dtype = if mcp.device_type.contains("Cisco IOS") { "cisco_ios" }
                        else if mcp.device_type.contains("Juniper") { "juniper_junos" }
                        else if mcp.device_type.contains("Arista") { "arista_eos" }
                        else { "cisco_ios" };
            return Some((mcp.ip.clone(), mcp.username.clone(), None, None, dtype.to_string()));
        }
    }

    None
}

pub fn resolve_host_with_preference(app: &tauri::AppHandle, host: &str) -> Result<std::net::IpAddr, String> {
    use std::net::{IpAddr, ToSocketAddrs};
    
    let parsed_ip = host.parse::<IpAddr>();
    
    let settings = crate::settings::load_settings(app.clone()).unwrap_or_default();
    let pref = settings.ip_version.as_deref().unwrap_or("auto");
    
    if let Ok(ip) = parsed_ip {
        match pref {
            "ipv4" => {
                if ip.is_ipv6() {
                    return Err("Connection target is IPv6, but IP preference is set to IPv4 Only".to_string());
                }
            }
            "ipv6" => {
                if ip.is_ipv4() {
                    return Err("Connection target is IPv4, but IP preference is set to IPv6 Only".to_string());
                }
            }
            _ => {}
        }
        return Ok(ip);
    }
    
    let addrs = format!("{}:80", host).to_socket_addrs().map_err(|e| e.to_string())?;
    let filtered: Vec<IpAddr> = addrs.into_iter().map(|a| a.ip()).filter(|ip| {
        match pref {
            "ipv4" => ip.is_ipv4(),
            "ipv6" => ip.is_ipv6(),
            _ => true,
        }
    }).collect();
    
    filtered.first().cloned().ok_or_else(|| {
        format!("Could not resolve host '{}' with IP preference '{}'", host, pref)
    })
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
            username: None,
            password: None,
            enable_password: None,
            device_type: None,
            vendor_type: None,
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
    fn test_get_mcp_hosts_returns_empty_list() {
        let hosts = get_mcp_hosts().unwrap();
        assert!(hosts.is_empty());
    }
}
