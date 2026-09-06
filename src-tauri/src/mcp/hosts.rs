use crate::mcp::protocol::McpToolResult;

#[tauri::command]
pub fn require_host_registered() -> Result<McpToolResult, String> {
    Ok(McpToolResult {
        success: false,
        output:
            "ホスト名の登録が必要です。IPアドレスおよびFQDNを直接指定したリモート接続は行えません。"
                .to_string(),
    })
}
