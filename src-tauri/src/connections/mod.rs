pub mod conn_type;
pub mod device_type;
pub mod device_types_data;
pub mod enable_password;
pub mod hostname;
pub mod id;
pub mod ip_address;
pub mod last_connected;
pub mod password;
pub mod port;
pub mod status;
pub mod username;
pub mod vendor_type;

pub use conn_type::ConnectionType;
pub use device_type::DeviceType;
pub use device_types_data::*;
pub use enable_password::EnablePassword;
pub use hostname::Hostname;
pub use id::ConnectionId;
pub use ip_address::IpAddress;
pub use last_connected::LastConnected;
pub use password::Password;
pub use port::Port;
pub use status::ConnectionStatus;
pub use username::Username;
pub use vendor_type::VendorType;

use crate::crypto::{decrypt, encrypt};
use crate::error::TauriError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tauri::Manager;
use validator::Validate;

#[derive(Debug, thiserror::Error)]
pub enum ConnectionError {
    #[error("File I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization/Deserialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Crypto error: {0}")]
    Crypto(#[from] crate::crypto::CryptoError),
    #[allow(dead_code)]
    #[error("Old connection not found for ID {0}")]
    OldConnectionNotFound(String),
    #[error("Failed to validate encrypted password for connection {0}: {1}")]
    PasswordValidation(String, String),
    #[error("Failed to validate encrypted enable password for connection {0}: {1}")]
    EnablePasswordValidation(String, String),
    #[error("Failed to encrypt password for connection {0}: {1}")]
    PasswordEncryption(String, String),
    #[error("Failed to encrypt enable password for connection {0}: {1}")]
    EnablePasswordEncryption(String, String),
    #[error("Connection target is {0}, but IP preference is set to {1}")]
    IpPreferenceMismatch(String, String),
    #[error("Could not resolve host '{0}' with IP preference '{1}'")]
    HostResolutionFailed(String, String),
}

fn deserialize_empty_as_none<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: std::convert::TryFrom<String>,
    <T as std::convert::TryFrom<String>>::Error: std::fmt::Display,
{
    let s: Option<String> = serde::Deserialize::deserialize(deserializer)?;
    match s {
        Some(val) => {
            if val.trim().is_empty() {
                Ok(None)
            } else {
                T::try_from(val).map(Some).map_err(serde::de::Error::custom)
            }
        }
        None => Ok(None),
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Validate)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    pub id: ConnectionId,
    pub status: ConnectionStatus,
    pub hostname: Hostname,
    #[serde(default, deserialize_with = "deserialize_empty_as_none")]
    pub ip: Option<IpAddress>,
    #[serde(default)]
    pub port: Option<Port>,
    #[serde(rename = "type")]
    pub conn_type: ConnectionType,
    pub last_connected: LastConnected,
    #[serde(default, deserialize_with = "deserialize_empty_as_none")]
    pub username: Option<Username>,
    #[serde(default, deserialize_with = "deserialize_empty_as_none")]
    pub password: Option<Password>,
    #[serde(default, deserialize_with = "deserialize_empty_as_none")]
    pub enable_password: Option<EnablePassword>,
    #[serde(default, deserialize_with = "deserialize_empty_as_none")]
    pub device_type: Option<DeviceType>,
    #[serde(default, deserialize_with = "deserialize_empty_as_none")]
    pub vendor_type: Option<VendorType>,
    #[serde(default)]
    pub auth_method: Option<String>,
    #[serde(default)]
    pub private_key_path: Option<String>,
    #[serde(default)]
    pub passphrase: Option<String>,
    #[serde(default)]
    pub agent_forwarding: Option<bool>,
    #[serde(default)]
    pub remember_password: Option<bool>,
    #[serde(default, skip_serializing)]
    pub has_password: Option<bool>,
    #[serde(default, skip_serializing)]
    pub has_enable_password: Option<bool>,
    #[serde(default, skip_serializing)]
    pub has_passphrase: Option<bool>,
    #[serde(default, skip_serializing)]
    pub password_changed: Option<bool>,
    #[serde(default, skip_serializing)]
    pub enable_password_changed: Option<bool>,
    #[serde(default, skip_serializing)]
    pub passphrase_changed: Option<bool>,
}

impl Connection {
    pub fn ip_string(&self) -> String {
        self.ip.as_ref().map(|i| i.to_string()).unwrap_or_default()
    }

    pub fn matches_host_or_ip(&self, target: &str) -> bool {
        self.hostname.eq_ignore_ascii_case(target)
            || self.ip.as_ref().map(|i| i.to_string()).as_deref() == Some(target)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct McpHost {
    pub hostname: Hostname,
    pub ip: IpAddress,
    pub device_type: DeviceType,
    pub username: Username,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CsvImportWarning {
    pub row: usize,
    pub reason: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CsvImportResult {
    pub connections: Vec<Connection>,
    pub imported_count: usize,
    pub warnings: Vec<CsvImportWarning>,
}

fn get_connections_path<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> PathBuf {
    let path = app
        .path()
        .app_data_dir()
        .expect("Failed to get app data dir");
    if !path.exists() {
        let _ = fs::create_dir_all(&path);
    }
    path.join("connections.json")
}

pub(crate) fn load_connections_raw<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<Vec<Connection>, ConnectionError> {
    let path = get_connections_path(app);
    if !path.exists() {
        return Ok(vec![]);
    }
    let data = fs::read_to_string(path)?;
    let connections: Vec<Connection> = serde_json::from_str(&data)?;
    Ok(connections)
}

#[tauri::command]
pub fn load_connections<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Vec<Connection>, TauriError> {
    let mut connections = load_connections_raw(&app)?;

    // Mask passwords for frontend so keychain is not accessed on startup/load.
    for conn in &mut connections {
        conn.has_password = Some(
            conn.password
                .as_ref()
                .map(|p| !p.is_empty())
                .unwrap_or(false),
        );
        conn.has_enable_password = Some(
            conn.enable_password
                .as_ref()
                .map(|ep| !ep.is_empty())
                .unwrap_or(false),
        );
        conn.has_passphrase = Some(
            conn.passphrase
                .as_ref()
                .map(|p| !p.is_empty())
                .unwrap_or(false),
        );
        conn.password = None;
        conn.enable_password = None;
        conn.passphrase = None;
    }

    Ok(connections)
}

#[tauri::command]
pub fn get_mcp_hosts() -> Result<Vec<McpHost>, TauriError> {
    let hosts = vec![];
    Ok(hosts)
}

#[tauri::command]
pub fn save_connections(
    app: tauri::AppHandle,
    mut connections: Vec<Connection>,
) -> Result<(), TauriError> {
    for conn in &connections {
        conn.validate()
            .map_err(|e| TauriError(crate::error::MikomaiError::Validation(e.to_string())))?;
    }
    let old_connections = load_connections_raw(&app).unwrap_or_default();
    let path = get_connections_path(&app);

    // Encrypt passwords before saving if they have changed from the placeholder flags
    for conn in &mut connections {
        let old_conn = old_connections.iter().find(|oc| oc.id == conn.id);

        let password_changed = conn.password_changed.unwrap_or(old_conn.is_none());
        if password_changed {
            if let Some(plain_password) = &conn.password {
                if !plain_password.is_empty() {
                    match encrypt(&app, plain_password.as_str()) {
                        Ok(encrypted) => match Password::try_from(encrypted) {
                            Ok(p) => conn.password = Some(p),
                            Err(e) => {
                                return Err(ConnectionError::PasswordValidation(
                                    conn.id.to_string(),
                                    e,
                                )
                                .into())
                            }
                        },
                        Err(e) => {
                            return Err(ConnectionError::PasswordEncryption(
                                conn.id.to_string(),
                                e.to_string(),
                            )
                            .into())
                        }
                    }
                } else {
                    conn.password = None;
                }
            } else {
                conn.password = None;
            }
        } else {
            if let Some(oc) = old_conn {
                conn.password = oc.password.clone();
            }
        }

        let enable_password_changed = conn.enable_password_changed.unwrap_or(old_conn.is_none());
        if enable_password_changed {
            if let Some(plain_enable_password) = &conn.enable_password {
                if !plain_enable_password.is_empty() {
                    match encrypt(&app, plain_enable_password.as_str()) {
                        Ok(encrypted) => match EnablePassword::try_from(encrypted) {
                            Ok(ep) => conn.enable_password = Some(ep),
                            Err(e) => {
                                return Err(ConnectionError::EnablePasswordValidation(
                                    conn.id.to_string(),
                                    e,
                                )
                                .into())
                            }
                        },
                        Err(e) => {
                            return Err(ConnectionError::EnablePasswordEncryption(
                                conn.id.to_string(),
                                e.to_string(),
                            )
                            .into())
                        }
                    }
                } else {
                    conn.enable_password = None;
                }
            } else {
                conn.enable_password = None;
            }
        } else {
            if let Some(oc) = old_conn {
                conn.enable_password = oc.enable_password.clone();
            }
        }

        let passphrase_changed = conn.passphrase_changed.unwrap_or(old_conn.is_none());
        if passphrase_changed {
            if let Some(plain_passphrase) = &conn.passphrase {
                if !plain_passphrase.is_empty() {
                    match encrypt(&app, plain_passphrase.as_str()) {
                        Ok(encrypted) => {
                            conn.passphrase = Some(encrypted);
                        }
                        Err(e) => {
                            return Err(ConnectionError::PasswordEncryption(
                                conn.id.to_string(),
                                e.to_string(),
                            )
                            .into())
                        }
                    }
                } else {
                    conn.passphrase = None;
                }
            } else {
                conn.passphrase = None;
            }
        } else {
            if let Some(oc) = old_conn {
                conn.passphrase = oc.passphrase.clone();
            }
        }
    }

    let data = serde_json::to_string_pretty(&connections)?;
    fs::write(path, data)?;
    Ok(())
}

fn csv_connection(record: &csv::StringRecord, headers: &csv::StringRecord) -> Result<Connection, String> {
    let fields: HashMap<&str, &str> = headers.iter().zip(record.iter()).collect();
    let value = |name: &str| fields.get(name).copied().unwrap_or("").trim();
    let hostname = value("hostname");
    let ip = value("ip");
    if hostname.is_empty() || ip.is_empty() {
        return Err("hostname と ip は必須です".to_string());
    }
    let connection = serde_json::json!({
        "id": if value("id").is_empty() { uuid::Uuid::new_v4().to_string() } else { value("id").to_string() },
        "status": if matches!(value("status"), "online" | "offline") { value("status") } else { "offline" },
        "hostname": hostname,
        "ip": ip,
        "port": if value("port").is_empty() { serde_json::Value::Null } else { serde_json::json!(value("port").parse::<u16>().map_err(|_| "port が不正です")?) },
        "type": if value("type").is_empty() { "SSH" } else { value("type") },
        "lastConnected": if value("lastConnected").is_empty() { "Never" } else { value("lastConnected") },
        "username": if value("username").is_empty() { serde_json::Value::Null } else { serde_json::Value::String(value("username").to_string()) },
        "deviceType": if value("deviceType").is_empty() { serde_json::Value::Null } else { serde_json::Value::String(value("deviceType").to_string()) },
        "vendorType": if value("vendorType").is_empty() { serde_json::Value::Null } else { serde_json::Value::String(value("vendorType").to_string()) },
    });
    serde_json::from_value(connection).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn import_connections_csv(app: tauri::AppHandle, path: String) -> Result<CsvImportResult, TauriError> {
    let mut reader = csv::ReaderBuilder::new().flexible(true).from_path(path)
        .map_err(|error| TauriError(crate::error::MikomaiError::Validation(error.to_string())))?;
    let headers = reader.headers().map_err(|error| TauriError(crate::error::MikomaiError::Validation(error.to_string())))?.clone();
    let mut warnings = Vec::new();
    let mut imported = Vec::new();
    for (index, row) in reader.records().enumerate() {
        match row.map_err(|error| error.to_string()).and_then(|record| csv_connection(&record, &headers)) {
            Ok(connection) => imported.push(connection),
            Err(reason) => warnings.push(CsvImportWarning { row: index + 2, reason }),
        }
    }
    let mut merged: HashMap<String, Connection> = load_connections_raw(&app)?.into_iter().map(|connection| (connection.id.to_string(), connection)).collect();
    let imported_count = imported.len();
    for connection in imported { merged.insert(connection.id.to_string(), connection); }
    let connections: Vec<Connection> = merged.into_values().collect();
    save_connections(app.clone(), connections)?;
    Ok(CsvImportResult { connections: load_connections(app)?, imported_count, warnings })
}

#[tauri::command]
pub fn export_connections_csv(app: tauri::AppHandle, path: String) -> Result<(), TauriError> {
    let connections = load_connections_raw(&app)?;
    let mut writer = csv::WriterBuilder::new().has_headers(true).from_path(path)
        .map_err(|error| TauriError(crate::error::MikomaiError::Validation(error.to_string())))?;
    writer.write_record(["id", "status", "hostname", "ip", "port", "type", "lastConnected", "deviceType", "vendorType", "username"])
        .map_err(|error| TauriError(crate::error::MikomaiError::Validation(error.to_string())))?;
    for connection in connections {
        writer.write_record([
            connection.id.to_string(), connection.status.to_string(), connection.hostname.to_string(), connection.ip_string(),
            connection.port.map(|port| port.to_string()).unwrap_or_default(), connection.conn_type.to_string(), connection.last_connected.to_string(),
            connection.device_type.map(|value| value.to_string()).unwrap_or_default(), connection.vendor_type.map(|value| value.to_string()).unwrap_or_default(),
            connection.username.map(|value| value.to_string()).unwrap_or_default(),
        ]).map_err(|error| TauriError(crate::error::MikomaiError::Validation(error.to_string())))?;
    }
    writer.flush().map_err(|error| TauriError(crate::error::MikomaiError::Validation(error.to_string())))?;
    Ok(())
}

pub fn resolve_host_with_mcp<R: tauri::Runtime>(app: &tauri::AppHandle<R>, host: &str) -> String {
    // 1. Check local connections first
    if let Ok(connections) = load_connections_raw(app) {
        if let Some(conn) = connections
            .iter()
            .find(|c| c.hostname.eq_ignore_ascii_case(host))
        {
            let ip_str = conn.ip_string();
            if !ip_str.is_empty() {
                return ip_str;
            }
        }
    }

    // 2. Fallback to original host (let DNS handle it)
    host.to_string()
}

pub fn get_device_config(
    app: &tauri::AppHandle,
    host: &str,
) -> Option<(String, String, Option<String>, Option<String>, String)> {
    // Returns (IP, Username, Password, EnablePassword, DeviceType)

    // 1. Check local connections
    if let Ok(connections) = load_connections_raw(app) {
        if let Some(conn) = connections.iter().find(|c| c.matches_host_or_ip(host)) {
            let mut dtype = if let Some(dt) = &conn.device_type {
                dt.as_str().to_string()
            } else if let Some(vt) = &conn.vendor_type {
                let vt_str = vt.as_str().to_lowercase();
                if vt_str.contains("cisco") {
                    "cisco_ios".to_string()
                } else if vt_str.contains("juniper") || vt_str.contains("junos") {
                    "juniper_junos".to_string()
                } else if vt_str.contains("arista") || vt_str.contains("eos") {
                    "arista_eos".to_string()
                } else if vt_str.contains("yamaha") {
                    "yamaha".to_string()
                } else if vt_str.contains("furukawa") || vt_str.contains("fitelnet") {
                    "furukawa_fitelnet".to_string()
                } else {
                    "cisco_ios".to_string()
                }
            } else {
                "cisco_ios".to_string()
            };

            if conn.conn_type == ConnectionType::Telnet && !dtype.ends_with("_telnet") {
                dtype = format!("{}_telnet", dtype);
            }

            let user = conn
                .username
                .as_ref()
                .map(|u| u.as_str().to_string())
                .unwrap_or_else(|| "admin".to_string());

            // Decrypt password on-demand when accessing the device
            let decrypted_password = conn.password.as_ref().and_then(|p| {
                if p.is_empty() {
                    None
                } else {
                    match decrypt(app, p.as_str()) {
                        Ok(decrypted) => Some(decrypted),
                        Err(e) => {
                            log::error!(
                                "Failed to decrypt password for connection {}: {}",
                                conn.id,
                                e
                            );
                            None
                        }
                    }
                }
            });

            let decrypted_enable_password = conn.enable_password.as_ref().and_then(|ep| {
                if ep.is_empty() {
                    None
                } else {
                    match decrypt(app, ep.as_str()) {
                        Ok(decrypted) => Some(decrypted),
                        Err(e) => {
                            log::error!(
                                "Failed to decrypt enable password for connection {}: {}",
                                conn.id,
                                e
                            );
                            None
                        }
                    }
                }
            });

            return Some((
                conn.ip_string(),
                user,
                decrypted_password,
                decrypted_enable_password,
                dtype,
            ));
        }
    }

    None
}

pub fn resolve_host_with_preference<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    host: &str,
) -> Result<std::net::IpAddr, ConnectionError> {
    use std::net::{IpAddr, ToSocketAddrs};

    let parsed_ip = host.parse::<IpAddr>();

    let settings = crate::settings::load_settings(app.clone()).unwrap_or_default();
    let pref = settings.ip_version.as_deref().unwrap_or("auto");

    if let Ok(ip) = parsed_ip {
        match pref {
            "ipv4" => {
                if ip.is_ipv6() {
                    return Err(ConnectionError::IpPreferenceMismatch(
                        "IPv6".to_string(),
                        "IPv4 Only".to_string(),
                    ));
                }
            }
            "ipv6" => {
                if ip.is_ipv4() {
                    return Err(ConnectionError::IpPreferenceMismatch(
                        "IPv4".to_string(),
                        "IPv6 Only".to_string(),
                    ));
                }
            }
            _ => {}
        }
        return Ok(ip);
    }

    let addrs = format!("{}:80", host).to_socket_addrs()?;
    let filtered: Vec<IpAddr> = addrs
        .into_iter()
        .map(|a| a.ip())
        .filter(|ip| match pref {
            "ipv4" => ip.is_ipv4(),
            "ipv6" => ip.is_ipv6(),
            _ => true,
        })
        .collect();

    filtered
        .first()
        .cloned()
        .ok_or_else(|| ConnectionError::HostResolutionFailed(host.to_string(), pref.to_string()))
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
            ip: Some(IpAddress::try_from("10.0.0.1").unwrap()),
            port: Some(Port::try_from(22).unwrap()),
            conn_type: ConnectionType::try_from("SSH").unwrap(),
            last_connected: LastConnected::try_from("2023-10-27").unwrap(),
            username: None,
            password: None,
            enable_password: None,
            device_type: None,
            vendor_type: None,
            auth_method: None,
            private_key_path: None,
            passphrase: None,
            agent_forwarding: None,
            remember_password: None,
            has_password: None,
            has_enable_password: None,
            has_passphrase: None,
            password_changed: None,
            enable_password_changed: None,
            passphrase_changed: None,
        };

        let serialized = serde_json::to_string(&conn).unwrap();
        assert!(serialized.contains(r#""id":"test-1""#));
        assert!(serialized.contains(r#""ip":"10.0.0.1""#));
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
        assert!(serialized.contains(r#""ip":"10.0.0.2""#));
        assert!(serialized.contains(r#""deviceType":"Telnet""#));
    }

    #[test]
    fn test_multiple_connections_deserialization_roundtrip() {
        let json_input = r#"[
            {
                "id": "conn-1",
                "status": "offline",
                "hostname": "host-1",
                "ip": "192.168.1.1",
                "port": 22,
                "type": "SSH (Password)",
                "lastConnected": "Never",
                "username": "root",
                "password": "long_encrypted_ciphertext_base64_string_that_exceeds_128_characters_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "enablePassword": null,
                "deviceType": "cisco_ios",
                "vendorType": "Cisco",
                "authMethod": "plain"
            },
            {
                "id": "conn-2",
                "status": "offline",
                "hostname": "host-2",
                "ip": "192.168.1.2",
                "port": 23,
                "type": "Telnet",
                "lastConnected": "Never",
                "username": "admin",
                "password": null,
                "enablePassword": null,
                "deviceType": "cisco_ios",
                "vendorType": "Cisco",
                "authMethod": "plain"
            }
        ]"#;

        let connections: Vec<Connection> = serde_json::from_str(json_input).unwrap();
        assert_eq!(connections.len(), 2);
        assert_eq!(connections[0].hostname.as_str(), "host-1");
        assert_eq!(connections[1].hostname.as_str(), "host-2");
        assert_eq!(connections[0].ip_string(), "192.168.1.1");
        assert_eq!(connections[1].ip_string(), "192.168.1.2");

        let serialized = serde_json::to_string(&connections).unwrap();
        let deserialized_again: Vec<Connection> = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized_again.len(), 2);
    }

    #[test]
    fn test_console_connection_without_ip() {
        let json_input = r#"[
            {
                "id": "console-conn-1",
                "status": "offline",
                "hostname": "Console-Router",
                "ip": "",
                "type": "Console (Serial)",
                "lastConnected": "Never",
                "deviceType": "cisco_ios"
            },
            {
                "id": "console-conn-2",
                "status": "offline",
                "hostname": "Console-Switch",
                "ip": null,
                "type": "Console (Serial)",
                "lastConnected": "Never"
            }
        ]"#;

        let connections: Vec<Connection> = serde_json::from_str(json_input).unwrap();
        assert_eq!(connections.len(), 2);
        assert_eq!(connections[0].hostname.as_str(), "Console-Router");
        assert_eq!(connections[0].ip, None);
        assert_eq!(connections[0].ip_string(), "");
        assert_eq!(connections[1].hostname.as_str(), "Console-Switch");
        assert_eq!(connections[1].ip, None);
        assert_eq!(connections[1].ip_string(), "");

        let serialized = serde_json::to_string(&connections).unwrap();
        let deserialized_again: Vec<Connection> = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized_again.len(), 2);
        assert_eq!(deserialized_again[0].ip, None);
        assert_eq!(deserialized_again[1].ip, None);
    }

    #[test]
    fn csv_connection_generates_an_id_and_discards_password_columns() {
        let headers = csv::StringRecord::from(vec!["hostname", "ip", "password", "type"]);
        let row = csv::StringRecord::from(vec!["router-1", "192.168.10.1", "secret", "SSH"]);
        let connection = csv_connection(&row, &headers).unwrap();
        assert!(!connection.id.to_string().is_empty());
        assert_eq!(connection.hostname.as_str(), "router-1");
        assert!(connection.password.is_none());
    }

    #[test]
    fn csv_connection_rejects_missing_required_address_data() {
        let headers = csv::StringRecord::from(vec!["hostname", "ip"]);
        let row = csv::StringRecord::from(vec!["router-1", ""]);
        assert!(csv_connection(&row, &headers).is_err());
    }
}
