use crate::network::CommandResult;
use super::fetch_base::{McpCommandFetcher, CommandTemplate};

struct ArpFetcher;

impl McpCommandFetcher for ArpFetcher {
    fn get_command_from_template(&self, template: &CommandTemplate) -> String {
        template.fetch_arp.clone()
    }
    
    fn get_log_prefix(&self) -> &'static str {
        "ARP"
    }
}

#[tauri::command]
pub async fn fetch_arp(app: tauri::AppHandle, device_name: Option<String>) -> Result<CommandResult, String> {
    let name = device_name.unwrap_or_default();
    ArpFetcher.fetch_device_info(&app, &name).await
}

