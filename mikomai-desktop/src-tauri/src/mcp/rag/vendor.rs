use crate::connections::Connection;
use crate::mcp::brands;
use regex::Regex;

pub struct ProcessedQuery {
    pub query: String,
    pub brand_filter: Option<String>,
}

pub fn check_registered_device(query: &str, app: &tauri::AppHandle) -> Option<String> {
    crate::mcp::devices::get_registered_device_info(query, app)
}

pub fn parse_vendor_context_with_app(query: &str, app: &tauri::AppHandle) -> ProcessedQuery {
    let connections = crate::connections::load_connections(app.clone()).ok();
    parse_vendor_context_with_connections(query, connections.as_deref())
}

pub fn parse_vendor_context_with_connections(
    query: &str,
    connections: Option<&[Connection]>,
) -> ProcessedQuery {
    let mut brand_filter: Option<String> = None;
    let mut processed_query = query.to_string();
    let mut detected_vendor: Option<String> = None;

    // 1. Regex to match [Context: Candidate] (Candidate could be brand name or device name)
    if let Ok(context_re) = Regex::new(r"\[Context:\s*([^\]\s]+)[^\]]*\]") {
        if let Some(caps) = context_re.captures(query) {
            let candidate = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            // 1-a. Direct brand check (e.g. Cisco, Yamaha, etc.)
            if let Some(matched_brand) = brands::get_brand(candidate) {
                brand_filter = Some(matched_brand.to_string());
                detected_vendor = Some(matched_brand.to_string());
                processed_query = context_re
                    .replace_all(query, "")
                    .to_string()
                    .trim()
                    .to_string();
            }
            // 1-b. Check if candidate matches registered device hostname/ip
            else if let Some(conns) = connections {
                if let Some(conn) = conns.iter().find(|c| c.matches_host_or_ip(candidate)) {
                    if let Some(matched_brand) = registered_connection_brand(conn) {
                        brand_filter = Some(matched_brand.clone());
                        detected_vendor = Some(matched_brand);
                    }
                    processed_query = context_re
                        .replace_all(query, "")
                        .to_string()
                        .trim()
                        .to_string();
                }
            }
        }

        // 2. If no brand filter from [Context: ...], check registered devices mentioned in the raw query text
        if brand_filter.is_none() {
            if let Some(conns) = connections {
                let query_lower = query.to_lowercase();
                for conn in conns {
                    let host_lower = conn.hostname.to_lowercase();
                    let ip_lower = conn.ip_string().to_lowercase();
                    let mentions_connection = (!host_lower.is_empty()
                        && query_lower.contains(&host_lower))
                        || (!ip_lower.is_empty() && query_lower.contains(&ip_lower));
                    if mentions_connection {
                        if let Some(matched_brand) = registered_connection_brand(conn) {
                            brand_filter = Some(matched_brand.clone());
                            detected_vendor = Some(matched_brand);
                            break;
                        }
                    }
                }
            }
        }

        // 3. Fallback: check if any known brand alias defined in brands.yaml is mentioned in the query string
        if brand_filter.is_none() {
            if let Some((matched_brand, _matched_alias)) = brands::detect_brand_in_text(query) {
                brand_filter = Some(matched_brand.to_string());
                detected_vendor = Some(matched_brand.to_string());
            }
        }

        // If query is now empty (e.g. LLM sent ONLY the context tag), fallback
        if processed_query.is_empty() && brand_filter.is_some() {
            if let Some(caps) = context_re.captures(query) {
                processed_query = caps.get(1).map(|m| m.as_str()).unwrap_or("").to_string();
            }
        }
    }

    // 4. Ensure the detected vendor name is included in the query text for AND search
    if let Some(ref vendor) = detected_vendor {
        let v_lower = vendor.to_lowercase();
        if !processed_query.to_lowercase().contains(&v_lower) {
            processed_query = format!("{} {}", vendor, processed_query).trim().to_string();
        }
    }

    ProcessedQuery {
        query: processed_query,
        brand_filter,
    }
}

/// Resolve a registered connection's vendor consistently with the device
/// adapters. Older connection records often store the vendor in `deviceType`
/// and leave the newer `vendorType` field empty.
pub(crate) fn registered_connection_brand(connection: &Connection) -> Option<String> {
    if let Some(vendor) = connection.vendor_type.as_ref() {
        let value = vendor.as_str();
        return Some(brands::get_brand(value).unwrap_or(value).to_string());
    }

    connection
        .device_type
        .as_ref()
        .and_then(|device_type| brands::get_brand(device_type.as_str()))
        .map(str::to_owned)
}

pub fn get_vector_search_instruction() -> &'static str {
    "ネットワーク機器の操作マニュアルから、関連する設定コマンドや手順を検索します。"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connections::{
        Connection, ConnectionId, ConnectionStatus, ConnectionType, DeviceType, Hostname,
        IpAddress, LastConnected, VendorType,
    };

    #[test]
    fn test_parse_vendor_context_device_name_resolution() {
        let connections = vec![Connection {
            id: ConnectionId::try_from("1").unwrap(),
            status: ConnectionStatus::try_from("active").unwrap(),
            hostname: Hostname::try_from("NakaokuGW").unwrap(),
            ip: Some(IpAddress::try_from("192.168.50.1").unwrap()),
            port: None,
            conn_type: ConnectionType::try_from("ssh").unwrap(),
            last_connected: LastConnected::try_from("2026-08-20 00:00:00").unwrap(),
            username: None,
            password: None,
            enable_password: None,
            device_type: None,
            vendor_type: Some(VendorType::try_from("yamaha").unwrap()),
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
        }];

        let query = "[Context: NakaokuGW] NTP 設定 確認";
        let processed = parse_vendor_context_with_connections(query, Some(&connections));

        assert!(processed.brand_filter.is_some());
        assert_eq!(processed.brand_filter.as_deref(), Some("yamaha"));
        // AND検索用にクエリにも yamaha が付加されていること
        assert!(processed.query.contains("yamaha"));
        assert!(processed.query.contains("NTP 設定 確認"));
    }

    #[test]
    fn test_parse_vendor_context_uses_legacy_device_type_for_vlan_queries() {
        let connections = vec![Connection {
            id: ConnectionId::try_from("2").unwrap(),
            status: ConnectionStatus::try_from("active").unwrap(),
            hostname: Hostname::try_from("F220").unwrap(),
            ip: None,
            port: None,
            conn_type: ConnectionType::try_from("console").unwrap(),
            last_connected: LastConnected::try_from("2026-08-20 00:00:00").unwrap(),
            username: None,
            password: None,
            enable_password: None,
            device_type: Some(DeviceType::try_from("furukawa_fitelnet").unwrap()),
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
        }];

        let processed = parse_vendor_context_with_connections(
            "[Context: F220] VLAN トランク 設定",
            Some(&connections),
        );

        assert_eq!(processed.brand_filter.as_deref(), Some("furukawa_fitelnet"));
        assert!(processed.query.contains("furukawa_fitelnet"));
        assert!(processed.query.contains("VLAN トランク 設定"));

        let processed_without_context =
            parse_vendor_context_with_connections("F220 VLAN トランク 設定", Some(&connections));
        assert_eq!(
            processed_without_context.brand_filter.as_deref(),
            Some("furukawa_fitelnet")
        );
    }
}
