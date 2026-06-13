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
#[allow(non_snake_case)]
pub async fn fetch_arp(
    app: tauri::AppHandle, 
    llama_state: tauri::State<'_, crate::llm::llm::LlamaState>,
    device_name: Option<String>,
    deviceName: Option<String>,
) -> Result<CommandResult, String> {
    let name = device_name.or(deviceName).unwrap_or_default();
    
    // 1. Fetch raw ARP table output
    let mut command_res = ArpFetcher.fetch_device_info(&app, &name).await?;
    
    if !command_res.success || command_res.output.trim().is_empty() {
        return Ok(command_res);
    }
    
    // 2. Resolve OS type for metadata
    let target_device = match crate::mcp::fetch::fetch_base::resolve_device_config(&app, &name).await {
        Ok(cfg) => cfg,
        Err(e) => {
            println!("Warning: failed to resolve device config for metadata: {}", e);
            return Ok(command_res);
        }
    };
    
    let os_type = target_device.device_type.clone();
    
    // 3. Convert raw output to YAML using the LLM and validate it
    let validated_yaml = match crate::mcp::arp::llm::convert_raw_to_yaml(&app, &llama_state, &command_res.output, &name, &os_type).await {
        Ok(yaml) => yaml,
        Err(e) => {
            println!("LLM ARP conversion/validation failed: {}", e);
            return Err(e);
        }
    };
    
    // 4. Save YAML log
    match crate::mcp::arp::yaml::save_validated_yaml(&name, &validated_yaml) {
        Ok(saved_path) => {
            command_res.saved_path = Some(saved_path);
        }
        Err(e) => {
            println!("Warning: failed to save validated YAML artifact: {}", e);
        }
    }
    
    Ok(command_res)
}
