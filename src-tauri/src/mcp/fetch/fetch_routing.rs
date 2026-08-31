use super::fetch_base::{CommandTemplate, McpCommandFetcher};
use crate::network::CommandResult;
use tauri::{Emitter, Manager};

struct RoutingFetcher;

impl McpCommandFetcher for RoutingFetcher {
    fn get_command_from_template(&self, template: &CommandTemplate) -> String {
        if !template.fetch_route.trim().is_empty() {
            template.fetch_route.clone()
        } else {
            "show ip route".to_string()
        }
    }

    fn get_log_prefix(&self) -> &'static str {
        "routing"
    }
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn fetch_routing(
    app: tauri::AppHandle,
    _llama_state: tauri::State<'_, crate::llm::llm::LlamaState>,
    device_name: Option<String>,
    deviceName: Option<String>,
    device: Option<String>,
    host: Option<String>,
    user_message: Option<String>,
    userMessage: Option<String>,
) -> Result<CommandResult, String> {
    let name = crate::mcp::args::normalize_device_args(
        &app,
        device_name,
        deviceName,
        device,
        host,
        user_message,
        userMessage,
    )?;

    // Resolve the device name using device_resolver
    let (resolved_name, _) = super::device_resolver::resolve_device_name_and_type(&app, &name)?;

    // Resolve the registered host name from connections
    let registered_name = {
        if let Ok(connections) = crate::connections::load_connections(app.clone()) {
            if let Some(conn) = connections
                .iter()
                .find(|c| c.matches_host_or_ip(&resolved_name))
            {
                conn.hostname.as_str().to_string()
            } else {
                resolved_name
            }
        } else {
            resolved_name
        }
    };

    // Check if within cache expiry duration
    if let Some(cached_res) = super::fetch_base::check_yaml_cache(&app, &registered_name, "route") {
        return Ok(cached_res);
    }

    // 1. Fetch raw routing table output using the registered host name
    let command_res = RoutingFetcher
        .fetch_device_info(&app, &registered_name)
        .await?;

    if !command_res.success || command_res.output.trim().is_empty() {
        return Ok(command_res);
    }

    // Preserve the authoritative raw result immediately. The validated
    // normalized form is added by the conversion task below.
    app.state::<crate::graph::SurrealDbState>()
        .ingest(crate::graph::GraphIngestInput {
            source_id: "mcp.fetch_routing".to_string(),
            collected_at: chrono::Utc::now(),
            device_name: registered_name.clone(),
            kind: crate::graph::GraphDataKind::Routing,
            raw: command_res.output.clone(),
            normalized: None,
            evidence: None,
            normalizer_version: "route-raw-v1".to_string(),
        })
        .await?;

    // 2. Spawn background task to resolve OS, convert to YAML via LLM, validate and save
    let app_clone = app.clone();
    let name_clone = registered_name.clone();
    let raw_output_clone = command_res.output.clone();

    tauri::async_runtime::spawn(async move {
        // Delay slightly to allow the subsequent agent (triggered by the frontend) to acquire the LLM inference lock first.
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // Resolve OS type for metadata
        let target_device =
            match crate::mcp::fetch::fetch_base::resolve_device_config(&app_clone, &name_clone)
                .await
            {
                Ok(cfg) => cfg,
                Err(e) => {
                    log::warn!(
                        "Warning: failed to resolve device config for metadata in background: {}",
                        e
                    );
                    return;
                }
            };

        let os_type = target_device.device_type.clone();
        let llama_state = app_clone.state::<crate::llm::llm::LlamaState>();

        // Convert raw output to YAML using the LLM and validate it
        let validated_yaml = match tokio::time::timeout(
            std::time::Duration::from_secs(30),
            crate::mcp::route::llm::convert_raw_to_yaml(
                &app_clone,
                &llama_state,
                &raw_output_clone,
                &name_clone,
                &os_type,
            ),
        )
        .await
        {
            Ok(Ok(yaml)) => yaml,
            Ok(Err(e)) => {
                log::error!(
                    "LLM route conversion/validation failed in background: {}",
                    e
                );
                return;
            }
            Err(_) => {
                log::warn!("LLM route normalization timed out; raw graph observation was retained");
                return;
            }
        };

        // Save YAML log
        match crate::mcp::route::yaml::save_validated_yaml(&app_clone, &name_clone, &validated_yaml)
        {
            Ok(saved_path) => {
                if let Err(e) = app_clone
                    .state::<crate::graph::SurrealDbState>()
                    .ingest(crate::graph::GraphIngestInput {
                        source_id: "mcp.fetch_routing".to_string(),
                        collected_at: chrono::Utc::now(),
                        device_name: name_clone.clone(),
                        kind: crate::graph::GraphDataKind::Routing,
                        raw: raw_output_clone.clone(),
                        normalized: crate::graph::normalize_yaml(
                            crate::graph::GraphDataKind::Routing,
                            &validated_yaml,
                        ),
                        evidence: None,
                        normalizer_version: "route-llm-yaml-v1".to_string(),
                    })
                    .await
                {
                    log::error!("Failed to ingest normalized routing graph data: {}", e);
                }
                log::info!(
                    "Background YAML normalization succeeded, saved to: {}",
                    saved_path.display()
                );
                if let Err(e) = app_clone.emit(
                    "chat-event",
                    crate::mcp::protocol::ChatEvent::RouteYamlSaved {
                        device_name: name_clone,
                        saved_path,
                    },
                ) {
                    log::error!("Error emitting route-yaml-saved event: {}", e);
                }
            }
            Err(e) => {
                log::warn!(
                    "Warning: failed to save validated YAML artifact in background: {}",
                    e
                );
            }
        }
    });

    Ok(command_res)
}
