use std::collections::HashMap;
use std::sync::OnceLock;

// McpTool Trait definition
pub trait McpTool: Send + Sync {
    fn name(&self) -> &'static str;
    fn execute(
        &self,
        app: tauri::AppHandle,
        args: serde_json::Value,
    ) -> futures::future::BoxFuture<'static, Result<crate::network::CommandResult, String>>;
}

pub fn get_tool_label(tool_name: &str) -> String {
    match tool_name {
        "self_network_ping" => "Ping".to_string(),
        "self_network_traceroute" => "Traceroute".to_string(),
        "self_network_test_connection" | "self_network_test_net_connection" => "Test Connection".to_string(),
        "network_get_hosts" => "Host List".to_string(),
        "network_query_nw_db" | "query_nw_db" | "query_rag" => "NWDB検索".to_string(),
        "self_network_arp" => "ARP Table".to_string(),
        "self_network_route" => "Route Table".to_string(),
        "network_get_ip_info" => "IP Info".to_string(),
        "network_list_serial_ports" => "Serial Ports".to_string(),
        "network_send_console_message" => "Console Message".to_string(),
        "network_show" => "Show Command".to_string(),
        "network_config" => "Config Command".to_string(),
        "fetch_config" => "Fetch Config".to_string(),
        "fetch_routing" => "Fetch Routing".to_string(),
        "fetch_arp" => "Fetch ARP".to_string(),
        "require_host_registered" => "ホスト登録要求".to_string(),
        "self_network_nwdiag" => "ネットワーク図生成".to_string(),
        "validate_cisco_config" => "Cisco設定検証".to_string(),
        "convert_cisco_config" => "Cisco設定変換".to_string(),
        "ask_user_choice" => "ユーザ選択".to_string(),
        "ask_interface_choice" => "インターフェース選択".to_string(),
        "ask_ipaddress_choice" => "IPアドレス選択".to_string(),
        _ => tool_name.to_string(),
    }
}

pub fn get_tool_registry() -> &'static HashMap<String, Box<dyn McpTool>> {
    static REGISTRY: OnceLock<HashMap<String, Box<dyn McpTool>>> = OnceLock::new();
    REGISTRY.get_or_init(super::tools::init_tool_registry)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_tool_label() {
        assert_eq!(get_tool_label("self_network_ping"), "Ping");
        assert_eq!(get_tool_label("network_query_nw_db"), "NWDB検索");
        assert_eq!(get_tool_label("query_nw_db"), "NWDB検索");
        assert_eq!(get_tool_label("query_rag"), "NWDB検索");
        assert_eq!(get_tool_label("unknown_tool"), "unknown_tool");
    }
}
