use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct HostListResult {
    pub success: bool,
    pub output: String,
}

impl From<HostListResult> for crate::network::CommandResult {
    fn from(res: HostListResult) -> Self {
        Self {
            success: res.success,
            output: res.output,
            saved_path: None,
            is_cached: None,
            cache_time: None,
        }
    }
}


#[tauri::command]
pub async fn network_get_hosts(app: tauri::AppHandle) -> Result<HostListResult, String> {
    use crate::connections::load_connections;

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

    if count == 0 {
        output = "登録されているホストが見つかりませんでした。".to_string();
    }

    Ok(HostListResult {
        success: true,
        output,
    })
}

#[tauri::command]
pub fn require_host_registered() -> Result<HostListResult, String> {
    Ok(HostListResult {
        success: false,
        output: "ホスト名の登録が必要です。IPアドレスおよびFQDNを直接指定したリモート接続は行えません。".to_string(),
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
