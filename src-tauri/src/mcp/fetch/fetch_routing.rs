use super::fetch_base::{CommandTemplate, McpCommandFetcher};
use crate::network::CommandResult;
use serde_json::Value;
use tauri::Manager;

pub(crate) struct RoutingFetcher;

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

    let graph = app.state::<crate::graph::SurrealDbState>();
    if let Some(canonical) = graph
        .fresh_canonical(&registered_name, crate::graph::GraphDataKind::Routing)
        .await?
    {
        return Ok(canonical_command_result(canonical, true));
    }
    if graph
        .fresh_raw(&registered_name, crate::graph::GraphDataKind::Routing)
        .await?
        .is_some()
    {
        crate::graph::canonicalize_route_on_read(&app, &graph, &registered_name).await?;
        if let Some(canonical) = graph
            .fresh_canonical(&registered_name, crate::graph::GraphDataKind::Routing)
            .await?
        {
            return Ok(canonical_command_result(canonical, true));
        }
        return Err("routing raw observation could not be canonicalized".to_string());
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
    graph
        .ingest(crate::graph::GraphIngestInput {
            source_id: "mcp.fetch_routing".to_string(),
            collected_at: chrono::Utc::now(),
            device_name: registered_name.clone(),
            kind: crate::graph::GraphDataKind::Routing,
            raw: command_res.output.clone(),
            normalized: None,
            canonical: None,
            evidence: None,
            normalizer_version: "route-raw-v1".to_string(),
        })
        .await?;

    crate::graph::canonicalize_route_on_read(&app, &graph, &registered_name).await?;
    let canonical = graph
        .fresh_canonical(&registered_name, crate::graph::GraphDataKind::Routing)
        .await?
        .ok_or_else(|| {
            "routing canonicalization did not produce a canonical observation".to_string()
        })?;
    Ok(canonical_command_result(canonical, false))
}

fn canonical_command_result(canonical: Value, cached: bool) -> CommandResult {
    CommandResult {
        success: true,
        output: serde_json::to_string_pretty(&canonical).unwrap_or_else(|_| canonical.to_string()),
        saved_path: None,
        is_cached: Some(cached),
        cache_time: None,
    }
}
