use crate::network::CommandResult;
use super::fetch_base::{McpCommandFetcher, CommandTemplate};
use tauri::{Emitter, Manager};

struct RoutingFetcher;

impl McpCommandFetcher for RoutingFetcher {
    fn get_command_from_template(&self, template: &CommandTemplate) -> String {
        template.fetch_route.clone()
    }
    
    fn get_log_prefix(&self) -> &'static str {
        "routing"
    }
}

#[derive(serde::Serialize, Clone)]
struct RouteYamlSavedPayload {
    #[serde(rename = "deviceName")]
    device_name: String,
    #[serde(rename = "savedPath")]
    saved_path: String,
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn fetch_routing(
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
            if let Some(conn) = connections.iter().find(|c| c.hostname.to_lowercase() == resolved_name.to_lowercase() || c.ip.as_str() == resolved_name) {
                conn.hostname.as_str().to_string()
            } else {
                resolved_name
            }
        } else {
            resolved_name
        }
    };
    
    // 1. Fetch raw routing table output using the registered host name
    let command_res = RoutingFetcher.fetch_device_info(&app, &registered_name).await?;
    
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
        let validated_yaml = match crate::mcp::route::llm::convert_raw_to_yaml(&app_clone, &llama_state, &raw_output_clone, &name_clone, &os_type).await {
            Ok(yaml) => yaml,
            Err(e) => {
                println!("LLM route conversion/validation failed in background: {}", e);
                return;
            }
        };
        
        // Save YAML log
        match crate::mcp::route::yaml::save_validated_yaml(&name_clone, &validated_yaml) {
            Ok(saved_path) => {
                println!("Background YAML normalization succeeded, saved to: {}", saved_path);
                let payload = RouteYamlSavedPayload {
                    device_name: name_clone,
                    saved_path,
                };
                if let Err(e) = app_clone.emit("route-yaml-saved", payload) {
                    println!("Error emitting route-yaml-saved event: {}", e);
                }
            }
            Err(e) => {
                println!("Warning: failed to save validated YAML artifact in background: {}", e);
            }
        }
    });
    
    Ok(command_res)
}


