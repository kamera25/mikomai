pub mod id;
pub mod status;
pub mod hostname;
pub mod ip_address;
pub mod conn_type;
pub mod last_connected;
pub mod username;
pub mod password;
pub mod enable_password;
pub mod device_type;
pub mod vendor_type;

pub use id::ConnectionId;
pub use status::ConnectionStatus;
pub use hostname::Hostname;
pub use ip_address::IpAddress;
pub use conn_type::ConnectionType;
pub use last_connected::LastConnected;
pub use username::Username;
pub use password::Password;
pub use enable_password::EnablePassword;
pub use device_type::DeviceType;
pub use vendor_type::VendorType;

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::Manager;
use crate::crypto::{encrypt, decrypt};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    pub id: ConnectionId,
    pub status: ConnectionStatus,
    pub hostname: Hostname,
    pub ip: IpAddress,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(rename = "type")]
    pub conn_type: ConnectionType,
    pub last_connected: LastConnected,
    #[serde(default)]
    pub username: Option<Username>,
    #[serde(default)]
    pub password: Option<Password>,
    #[serde(default)]
    pub enable_password: Option<EnablePassword>,
    #[serde(default)]
    pub device_type: Option<DeviceType>,
    #[serde(default)]
    pub vendor_type: Option<VendorType>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct McpHost {
    pub hostname: Hostname,
    pub ip: IpAddress,
    pub device_type: DeviceType,
    pub username: Username,
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
                match decrypt(&app, encrypted_password.as_str()) {
                    Ok(decrypted) => {
                        match Password::try_from(decrypted) {
                            Ok(p) => conn.password = Some(p),
                            Err(e) => log::error!("Failed to validate decrypted password for connection {}: {}", conn.id, e),
                        }
                    }
                    Err(e) => log::error!("Failed to decrypt password for connection {}: {}", conn.id, e),
                }
            }
        }
        if let Some(encrypted_enable_password) = &conn.enable_password {
            if !encrypted_enable_password.is_empty() {
                match decrypt(&app, encrypted_enable_password.as_str()) {
                    Ok(decrypted) => {
                        match EnablePassword::try_from(decrypted) {
                            Ok(ep) => conn.enable_password = Some(ep),
                            Err(e) => log::error!("Failed to validate decrypted enable password for connection {}: {}", conn.id, e),
                        }
                    }
                    Err(e) => log::error!("Failed to decrypt enable password for connection {}: {}", conn.id, e),
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
                match encrypt(&app, plain_password.as_str()) {
                    Ok(encrypted) => {
                        match Password::try_from(encrypted) {
                            Ok(p) => conn.password = Some(p),
                            Err(e) => return Err(format!("Failed to validate encrypted password for connection {}: {}", conn.id, e)),
                        }
                    }
                    Err(e) => return Err(format!("Failed to encrypt password for connection {}: {}", conn.id, e)),
                }
            }
        }
        if let Some(plain_enable_password) = &conn.enable_password {
            if !plain_enable_password.is_empty() {
                match encrypt(&app, plain_enable_password.as_str()) {
                    Ok(encrypted) => {
                        match EnablePassword::try_from(encrypted) {
                            Ok(ep) => conn.enable_password = Some(ep),
                            Err(e) => return Err(format!("Failed to validate encrypted enable password for connection {}: {}", conn.id, e)),
                        }
                    }
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
        if let Some(conn) = connections.iter().find(|c| c.hostname.eq_ignore_ascii_case(host)) {
            return conn.ip.to_string();
        }
    }

    // 2. Check MCP registry
    if let Ok(mcp_hosts) = get_mcp_hosts() {
        if let Some(mcp) = mcp_hosts.iter().find(|h| h.hostname.eq_ignore_ascii_case(host)) {
            return mcp.ip.to_string();
        }
    }

    // 3. Fallback to original host (let DNS handle it)
    host.to_string()
}

pub fn get_device_config(app: &tauri::AppHandle, host: &str) -> Option<(String, String, Option<String>, Option<String>, String)> {
    // Returns (IP, Username, Password, EnablePassword, DeviceType)
    
    // 1. Check local connections
    if let Ok(connections) = load_connections(app.clone()) {
        if let Some(conn) = connections.iter().find(|c| c.hostname.eq_ignore_ascii_case(host) || c.ip.as_str() == host) {
            let mut dtype = if let Some(dt) = &conn.device_type {
                dt.as_str().to_string()
            } else if let Some(vt) = &conn.vendor_type {
                let vt_str = vt.as_str().to_lowercase();
                if vt_str.contains("cisco") { "cisco_ios".to_string() }
                else if vt_str.contains("juniper") || vt_str.contains("junos") { "juniper_junos".to_string() }
                else if vt_str.contains("arista") || vt_str.contains("eos") { "arista_eos".to_string() }
                else if vt_str.contains("yamaha") { "yamaha".to_string() }
                else if vt_str.contains("furukawa") || vt_str.contains("fitelnet") { "furukawa_fitelnet".to_string() }
                else { "cisco_ios".to_string() }
            } else {
                "cisco_ios".to_string()
            };

            if conn.conn_type == ConnectionType::Telnet && !dtype.ends_with("_telnet") {
                dtype = format!("{}_telnet", dtype);
            }

            let user = conn.username.as_ref().map(|u| u.as_str().to_string()).unwrap_or_else(|| "admin".to_string());
            return Some((conn.ip.to_string(), user, conn.password.as_ref().map(|p| p.to_string()), conn.enable_password.as_ref().map(|ep| ep.to_string()), dtype));
        }
    }

    // 2. Check MCP registry
    if let Ok(mcp_hosts) = get_mcp_hosts() {
        if let Some(mcp) = mcp_hosts.iter().find(|h| h.hostname.eq_ignore_ascii_case(host) || h.ip.as_str() == host) {
            let mut dtype = if mcp.device_type.contains("Cisco IOS") { "cisco_ios".to_string() }
                        else if mcp.device_type.contains("Juniper") { "juniper_junos".to_string() }
                        else if mcp.device_type.contains("Arista") { "arista_eos".to_string() }
                        else { "cisco_ios".to_string() };

            if mcp.device_type.contains("Telnet") && !dtype.ends_with("_telnet") {
                dtype = format!("{}_telnet", dtype);
            }

            return Some((mcp.ip.to_string(), mcp.username.to_string(), None, None, dtype));
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
            id: ConnectionId::try_from("test-1").unwrap(),
            status: ConnectionStatus::try_from("active").unwrap(),
            hostname: Hostname::try_from("router-1").unwrap(),
            ip: IpAddress::try_from("10.0.0.1").unwrap(),
            port: Some(22),
            conn_type: ConnectionType::try_from("SSH").unwrap(),
            last_connected: LastConnected::try_from("2023-10-27").unwrap(),
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
            hostname: Hostname::try_from("switch-1").unwrap(),
            ip: IpAddress::try_from("10.0.0.2").unwrap(),
            device_type: DeviceType::try_from("Telnet").unwrap(),
            username: Username::try_from("admin").unwrap(),
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
