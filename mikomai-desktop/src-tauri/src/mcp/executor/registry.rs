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

use crate::mcp::ToolKind;
use std::str::FromStr;

pub fn get_tool_label(tool_name: &str) -> String {
    if let Ok(kind) = ToolKind::from_str(tool_name) {
        kind.label().to_string()
    } else {
        tool_name.to_string()
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
