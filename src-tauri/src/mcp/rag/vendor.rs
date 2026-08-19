use crate::connections::Connection;
use crate::mcp::brands;
use regex::Regex;

pub struct ProcessedQuery
{
    pub query: String,
    pub brand_filter: Option<String>,
}

pub fn check_registered_device(query: &str, app: &tauri::AppHandle) -> Option<String>
{
    crate::mcp::devices::get_registered_device_info(query, app)
}

pub fn parse_vendor_context(query: &str) -> ProcessedQuery
{
    parse_vendor_context_with_connections(query, None)
}

pub fn parse_vendor_context_with_app(query: &str, app: &tauri::AppHandle) -> ProcessedQuery
{
    let connections = crate::connections::load_connections(app.clone()).ok();
    parse_vendor_context_with_connections(query, connections.as_deref())
}

pub fn parse_vendor_context_with_connections(
    query: &str,
    connections: Option<&[Connection]>,
) -> ProcessedQuery
{
    let mut brand_filter: Option<String> = None;
    let mut processed_query = query.to_string();
    let mut detected_vendor: Option<String> = None;

    // 1. Regex to match [Context: Candidate] (Candidate could be brand name or device name)
    if let Ok(context_re) = Regex::new(r"\[Context:\s*([^\]\s]+)[^\]]*\]")
    {
        if let Some(caps) = context_re.captures(query)
        {
            let candidate = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            // 1-a. Direct brand check (e.g. Cisco, Yamaha, etc.)
            if let Some(matched_brand) = brands::get_brand(candidate)
            {
                brand_filter = Some(format!(
                    "brand = '{0}' OR brand = '{1}'",
                    matched_brand, candidate
                ));
                detected_vendor = Some(matched_brand.to_string());
                processed_query = context_re
                    .replace_all(query, "")
                    .to_string()
                    .trim()
                    .to_string();
            }
            // 1-b. Check if candidate matches registered device hostname/ip
            else if let Some(conns) = connections
            {
                if let Some(conn) = conns.iter().find(|c| c.matches_host_or_ip(candidate)) {
                    if let Some(ref v_type) = conn.vendor_type {
                        let v_str = v_type.as_str();
                        let matched_brand = brands::get_brand(v_str).unwrap_or(v_str);
                        brand_filter = Some(format!(
                            "brand = '{0}' OR brand = '{1}'",
                            matched_brand, v_str
                        ));
                        detected_vendor = Some(matched_brand.to_string());
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
                for conn in conns {
                    let host_lower = conn.hostname.to_lowercase();
                    if !host_lower.is_empty() && query.to_lowercase().contains(&host_lower) {
                        if let Some(ref v_type) = conn.vendor_type {
                            let v_str = v_type.as_str();
                            let matched_brand = brands::get_brand(v_str).unwrap_or(v_str);
                            brand_filter = Some(format!(
                                "brand = '{0}' OR brand = '{1}'",
                                matched_brand, v_str
                            ));
                            detected_vendor = Some(matched_brand.to_string());
                            break;
                        }
                    }
                }
            }
        }

        // 3. Fallback: check if any known brand alias defined in brands.yaml is mentioned in the query string
        if brand_filter.is_none()
        {
            if let Some((matched_brand, matched_alias)) = brands::detect_brand_in_text(query)
            {
                brand_filter = Some(format!(
                    "brand = '{0}' OR brand = '{1}'",
                    matched_brand, matched_alias
                ));
                detected_vendor = Some(matched_brand.to_string());
            }
        }

        // If query is now empty (e.g. LLM sent ONLY the context tag), fallback
        if processed_query.is_empty() && brand_filter.is_some()
        {
            if let Some(caps) = context_re.captures(query)
            {
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

pub fn get_vector_search_instruction() -> &'static str
{
    "ネットワーク機器の操作マニュアルから、関連する設定コマンドや手順を検索します。"
}

#[cfg(test)]
mod tests
{
    use super::*;
    use crate::connections::{Connection, ConnectionId, ConnectionStatus, ConnectionType, Hostname, IpAddress, LastConnected, VendorType};

    #[test]
    fn test_parse_vendor_context_device_name_resolution()
    {
        let connections = vec![
            Connection {
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
            }
        ];

        let query = "[Context: NakaokuGW] NTP 設定 確認";
        let processed = parse_vendor_context_with_connections(query, Some(&connections));

        assert!(processed.brand_filter.is_some());
        let filter = processed.brand_filter.unwrap();
        assert!(filter.contains("yamaha"));
        // AND検索用にクエリにも yamaha が付加されていること
        assert!(processed.query.contains("yamaha"));
        assert!(processed.query.contains("NTP 設定 確認"));
    }
}
