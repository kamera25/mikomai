use crate::network::CommandResult;
use super::fetch_base::{McpCommandFetcher, CommandTemplate};

struct RoutingFetcher;

impl McpCommandFetcher for RoutingFetcher {
    fn get_command_from_template(&self, template: &CommandTemplate) -> String {
        template.fetch_route.clone()
    }
    
    fn get_log_prefix(&self) -> &'static str {
        "routing"
    }
}

#[tauri::command]
pub async fn fetch_routing(app: tauri::AppHandle, device_name: Option<String>) -> Result<CommandResult, String> {
    let name = device_name.unwrap_or_default();
    RoutingFetcher.fetch_device_info(&app, &name).await
}

