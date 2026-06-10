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
    let device_name = match device_name {
        Some(name) if !name.trim().is_empty() => name,
        _ => return Err("Error: device_name (機器名) is required but was not provided or is empty.".to_string()),
    };
    ArpFetcher.fetch_device_info(&app, &device_name).await
}

