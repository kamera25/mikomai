use crate::network::CommandResult;
use super::fetch_base::{McpCommandFetcher, CommandTemplate};
use tauri::{Emitter, Manager};

struct ArpFetcher;

impl McpCommandFetcher for ArpFetcher {
    fn get_command_from_template(&self, template: &CommandTemplate) -> String {
        template.fetch_arp.clone()
    }
    
    fn get_log_prefix(&self) -> &'static str {
        "ARP"
    }
}

#[derive(serde::Serialize, Clone)]
struct ArpYamlSavedPayload {
    #[serde(rename = "deviceName")]
    device_name: String,
    #[serde(rename = "savedPath")]
    saved_path: String,
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn fetch_arp(
    app: tauri::AppHandle, 
    _llama_state: tauri::State<'_, crate::llm::llm::LlamaState>,
    device_name: Option<String>,
    deviceName: Option<String>,
) -> Result<CommandResult, String> {
    let name = device_name.or(deviceName).unwrap_or_default();
    
    // Resolve the device name using device_resolver
    let (resolved_name, _) = super::device_resolver::resolve_device_name_and_type(&app, &name)?;
    
    // Resolve the registered host name from connections
    let registered_name = {
        if let Ok(connections) = crate::connections::load_connections(app.clone()) {
            if let Some(conn) = connections.iter().find(|c| c.hostname.eq_ignore_ascii_case(&resolved_name) || c.ip.as_str() == resolved_name) {
                conn.hostname.as_str().to_string()
            } else {
                resolved_name
            }
        } else {
            resolved_name
        }
    };
    
    // 1. Fetch raw ARP table output using the registered host name
    let command_res = ArpFetcher.fetch_device_info(&app, &registered_name).await?;
    
    if !command_res.success || command_res.output.trim().is_empty() {
        return Ok(command_res);
    }
    
    // 2. Spawn background task to resolve OS, convert to YAML via LLM, validate and save
    let app_clone = app.clone();
    let name_clone = registered_name.clone();
    let raw_output_clone = command_res.output.clone();
    
    tauri::async_runtime::spawn(async move {
        // Resolve OS type for metadata
        let target_device = match crate::mcp::fetch::fetch_base::resolve_device_config(&app_clone, &name_clone).await {
            Ok(cfg) => cfg,
            Err(e) => {
                println!("Warning: failed to resolve device config for metadata in background: {}", e);
                return;
            }
        };
        
        let os_type = target_device.device_type.clone();
        let llama_state = app_clone.state::<crate::llm::llm::LlamaState>();
        
        // Convert raw output to YAML using the LLM and validate it
        let validated_yaml = match crate::mcp::arp::llm::convert_raw_to_yaml(&app_clone, &llama_state, &raw_output_clone, &name_clone, &os_type).await {
            Ok(yaml) => yaml,
            Err(e) => {
                println!("LLM ARP conversion/validation failed in background: {}", e);
                return;
            }
        };
        
        // Save YAML log
        match crate::mcp::arp::yaml::save_validated_yaml(&name_clone, &validated_yaml) {
            Ok(saved_path) => {
                println!("Background YAML normalization succeeded, saved to: {}", saved_path);
                let payload = ArpYamlSavedPayload {
                    device_name: name_clone,
                    saved_path,
                };
                if let Err(e) = app_clone.emit("arp-yaml-saved", payload) {
                    println!("Error emitting arp-yaml-saved event: {}", e);
                }
            }
            Err(e) => {
                println!("Warning: failed to save validated YAML artifact in background: {}", e);
            }
        }
    });
    
    Ok(command_res)
}

