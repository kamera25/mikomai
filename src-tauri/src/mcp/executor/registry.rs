use std::collections::HashMap;
use std::sync::OnceLock;

// McpTool Trait definition
pub trait McpTool: Send + Sync
{
    fn name(&self) -> &'static str;
    fn execute(
        &self,
        app: tauri::AppHandle,
        args: serde_json::Value,
    ) -> futures::future::BoxFuture<'static, Result<crate::network::CommandResult, String>>;
}

static TOOL_LABELS: std::sync::LazyLock<HashMap<String, String>> = std::sync::LazyLock::new(|| {
    let yaml_str = include_str!("../config/tool_labels.yaml");
    serde_yaml::from_str(yaml_str).unwrap_or_else(|e| {
        log::error!("Failed to parse tool_labels.yaml: {}", e);
        HashMap::new()
    })
});

pub fn get_tool_label(tool_name: &str) -> String
{
    TOOL_LABELS
        .get(tool_name)
        .cloned()
        .unwrap_or_else(|| tool_name.to_string())
}

pub fn get_tool_registry() -> &'static HashMap<String, Box<dyn McpTool>>
{
    static REGISTRY: OnceLock<HashMap<String, Box<dyn McpTool>>> = OnceLock::new();
    REGISTRY.get_or_init(super::tools::init_tool_registry)
}

#[cfg(test)]
mod tests
{
    use super::*;

    #[test]
    fn test_get_tool_label()
    {
        assert_eq!(get_tool_label("self_network_ping"), "Ping");
        assert_eq!(get_tool_label("network_query_nw_db"), "NWDB検索");
        assert_eq!(get_tool_label("query_nw_db"), "NWDB検索");
        assert_eq!(get_tool_label("query_rag"), "NWDB検索");
        assert_eq!(get_tool_label("unknown_tool"), "unknown_tool");
    }
}
