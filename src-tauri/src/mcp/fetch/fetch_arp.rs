use super::fetch_base::{CommandTemplate, McpCommandFetcher};
use crate::network::CommandResult;
use tauri::Manager;

struct ArpFetcher;

impl McpCommandFetcher for ArpFetcher {
    fn get_command_from_template(&self, template: &CommandTemplate) -> String {
        if !template.fetch_arp.trim().is_empty() {
            template.fetch_arp.clone()
        } else {
            "show ip arp".to_string()
        }
    }

    fn get_log_prefix(&self) -> &'static str {
        "ARP"
    }
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn fetch_arp(
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

    // SurrealDB is the authoritative read-through cache for fetched state.
    // A fresh graph observation is returned to the Agent; otherwise we must
    // contact the device and commit the new observation before returning.
    if let Some((raw, collected_at)) = app
        .state::<crate::graph::SurrealDbState>()
        .fresh_raw(&registered_name, crate::graph::GraphDataKind::Arp)
        .await?
    {
        return Ok(CommandResult {
            success: true,
            output: raw,
            saved_path: None,
            is_cached: Some(true),
            cache_time: Some(collected_at),
        });
    }

    // 1. Fetch raw ARP table output using the registered host name
    let command_res = ArpFetcher.fetch_device_info(&app, &registered_name).await?;

    if !command_res.success || command_res.output.trim().is_empty() {
        return Ok(command_res);
    }

    app.state::<crate::graph::SurrealDbState>()
        .ingest(crate::graph::GraphIngestInput {
            source_id: "mcp.fetch_arp".to_string(),
            collected_at: chrono::Utc::now(),
            device_name: registered_name.clone(),
            kind: crate::graph::GraphDataKind::Arp,
            raw: command_res.output.clone(),
            normalized: None,
            evidence: None,
            normalizer_version: "arp-raw-v1".to_string(),
        })
        .await?;

    // LLM canonicalization must not be enqueued here. The Agent loop uses the
    // same single inference worker for its next planning step; a background
    // normalization request can otherwise block that loop even after its
    // timeout fires (queued llama.cpp inference is not cancellable).
    //
    // The raw observation is already persisted above. Canonicalization remains
    // available as an explicit, low-priority operation after the agent run.
    log::info!(
        "ARP raw observation stored for {}; deferred canonicalization to avoid blocking the Agent loop",
        registered_name
    );

    Ok(command_res)
}
