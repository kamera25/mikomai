use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct HostListResult {
    pub success: bool,
    pub output: String,
}

#[tauri::command]
pub async fn network_get_hosts(app: tauri::AppHandle) -> Result<HostListResult, String> {
    use crate::connections::{load_connections, get_mcp_hosts};

    let var_name = "登録されている接続可能なホスト一覧:\n\n".to_string();
    let mut output = var_name;
    output.push_str("| ホスト名 | IPアドレス | 接続タイプ | ソース |\n");
    output.push_str("|----------|------------|------------|--------|\n");

    let mut count = 0;

    // Load local connections
    if let Ok(connections) = load_connections(app.clone()) {
        for conn in connections {
            output.push_str(&format!("| {} | {} | {} | ローカル設定 |\n", conn.hostname, conn.ip, conn.conn_type));
            count += 1;
        }
    }

    // Load MCP hosts
    if let Ok(mcp_hosts) = get_mcp_hosts() {
        for host in mcp_hosts {
            output.push_str(&format!("| {} | {} | {} | MCPレジストリ |\n", host.hostname, host.ip, host.device_type));
            count += 1;
        }
    }

    if count == 0 {
        output = "登録されているホストが見つかりませんでした。".to_string();
    }

    Ok(HostListResult {
        success: true,
        output,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_host_list_result_serialization() {
        let result = HostListResult {
            success: true,
            output: "Mock output".to_string(),
        };
        let serialized = serde_json::to_string(&result).unwrap();
        assert_eq!(serialized, r#"{"success":true,"output":"Mock output"}"#);
    }
}
