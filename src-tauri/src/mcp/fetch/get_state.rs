use super::fetch_base::{CommandTemplate, McpCommandFetcher};
use super::state_resource::StateResource;
use crate::graph::{GraphDataKind, GraphIngestInput, SurrealDbState};
use crate::network::CommandResult;
use std::str::FromStr;
use tauri::Manager;

pub struct InterfacesFetcher;
impl McpCommandFetcher for InterfacesFetcher {
    fn get_command_from_template(&self, template: &CommandTemplate) -> String {
        if !template.fetch_interfaces.is_empty() {
            template.fetch_interfaces.clone()
        } else {
            "show interfaces".to_string()
        }
    }

    fn get_log_prefix(&self) -> &'static str {
        "interfaces"
    }
}

pub struct LldpFetcher;
impl McpCommandFetcher for LldpFetcher {
    fn get_command_from_template(&self, template: &CommandTemplate) -> String {
        if !template.fetch_lldp.is_empty() {
            template.fetch_lldp.clone()
        } else {
            "show lldp neighbors".to_string()
        }
    }

    fn get_log_prefix(&self) -> &'static str {
        "lldp"
    }
}

pub struct MacTableFetcher;
impl McpCommandFetcher for MacTableFetcher {
    fn get_command_from_template(&self, template: &CommandTemplate) -> String {
        if !template.fetch_mac_table.is_empty() {
            template.fetch_mac_table.clone()
        } else {
            "show mac address-table".to_string()
        }
    }

    fn get_log_prefix(&self) -> &'static str {
        "mac_table"
    }
}

pub struct BgpFetcher;
impl McpCommandFetcher for BgpFetcher {
    fn get_command_from_template(&self, template: &CommandTemplate) -> String {
        if !template.fetch_bgp.is_empty() {
            template.fetch_bgp.clone()
        } else {
            "show ip bgp summary".to_string()
        }
    }

    fn get_log_prefix(&self) -> &'static str {
        "bgp"
    }
}

pub struct OspfFetcher;
impl McpCommandFetcher for OspfFetcher {
    fn get_command_from_template(&self, template: &CommandTemplate) -> String {
        if !template.fetch_ospf.is_empty() {
            template.fetch_ospf.clone()
        } else {
            "show ip ospf neighbor".to_string()
        }
    }

    fn get_log_prefix(&self) -> &'static str {
        "ospf"
    }
}

async fn fetch_and_ingest_state<F: McpCommandFetcher>(
    app: &tauri::AppHandle,
    fetcher: F,
    device_name: &str,
    kind: GraphDataKind,
    normalizer_version: &str,
) -> Result<CommandResult, String> {
    let result = fetcher.fetch_device_info(app, device_name).await?;
    if result.success && !result.output.trim().is_empty() {
        let graph = app.state::<SurrealDbState>();
        let _ = graph
            .ingest(GraphIngestInput {
                source_id: format!("mcp.get_state.{}", kind.as_str()),
                collected_at: chrono::Utc::now(),
                device_name: device_name.to_string(),
                kind,
                raw: result.output.clone(),
                normalized: None,
                normalizer_version: normalizer_version.to_string(),
            })
            .await;
    }
    Ok(result)
}

pub async fn dispatch_get_state(
    app: &tauri::AppHandle,
    device_name: &str,
    resource: StateResource,
    user_msg: Option<String>,
) -> Result<CommandResult, String> {
    match resource {
        StateResource::Arp => {
            let llama_state = app.state::<crate::llm::llm::LlamaState>();
            crate::mcp::fetch::fetch_arp::fetch_arp(
                app.clone(),
                llama_state,
                Some(device_name.to_string()),
                None,
                None,
                None,
                user_msg.clone(),
                user_msg,
            )
            .await
        }
        StateResource::Routes => {
            let llama_state = app.state::<crate::llm::llm::LlamaState>();
            crate::mcp::fetch::fetch_routing::fetch_routing(
                app.clone(),
                llama_state,
                Some(device_name.to_string()),
                None,
                None,
                None,
                user_msg.clone(),
                user_msg,
            )
            .await
        }
        StateResource::Interfaces => {
            fetch_and_ingest_state(
                app,
                InterfacesFetcher,
                device_name,
                GraphDataKind::Interfaces,
                "interfaces-raw-v1",
            )
            .await
        }
        StateResource::Lldp => {
            fetch_and_ingest_state(
                app,
                LldpFetcher,
                device_name,
                GraphDataKind::Lldp,
                "lldp-raw-v1",
            )
            .await
        }
        StateResource::MacTable => {
            fetch_and_ingest_state(
                app,
                MacTableFetcher,
                device_name,
                GraphDataKind::MacTable,
                "mac-table-raw-v1",
            )
            .await
        }
        StateResource::Bgp => {
            fetch_and_ingest_state(
                app,
                BgpFetcher,
                device_name,
                GraphDataKind::Bgp,
                "bgp-raw-v1",
            )
            .await
        }
        StateResource::Ospf => {
            fetch_and_ingest_state(
                app,
                OspfFetcher,
                device_name,
                GraphDataKind::Ospf,
                "ospf-raw-v1",
            )
            .await
        }
    }
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn get_state(
    app: tauri::AppHandle,
    device: Option<String>,
    deviceName: Option<String>,
    device_name: Option<String>,
    host: Option<String>,
    target: Option<String>,
    resource: Option<String>,
    resourceType: Option<String>,
    resource_type: Option<String>,
    userMessage: Option<String>,
    user_message: Option<String>,
) -> Result<CommandResult, String> {
    let resolved_device_arg = device
        .or(deviceName)
        .or(device_name)
        .or(host)
        .or(target);

    let normalized_device = crate::mcp::args::normalize_device_args(
        &app,
        resolved_device_arg.clone(),
        resolved_device_arg.clone(),
        resolved_device_arg.clone(),
        resolved_device_arg,
        user_message.clone(),
        userMessage.clone(),
    )?;

    // Resolve the device name using device_resolver
    let (resolved_name, _) =
        super::device_resolver::resolve_device_name_and_type(&app, &normalized_device)?;

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

    let raw_resource = resource
        .or(resourceType)
        .or(resource_type)
        .ok_or_else(|| {
            format!(
                "Error: 'resource' parameter is required. Supported values: {}",
                StateResource::valid_resources().join(", ")
            )
        })?;

    let parsed_resource = StateResource::from_str(&raw_resource)?;
    let final_user_msg = user_message.or(userMessage);

    dispatch_get_state(&app, &registered_name, parsed_resource, final_user_msg).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fetchers_commands() {
        let template = CommandTemplate {
            fetch_config: "show run".to_string(),
            fetch_route: "show ip route".to_string(),
            fetch_bgp: "show ip bgp summary".to_string(),
            fetch_arp: "show ip arp".to_string(),
            fetch_interfaces: "show interfaces".to_string(),
            fetch_lldp: "show lldp neighbors".to_string(),
            fetch_mac_table: "show mac address-table".to_string(),
            fetch_ospf: "show ip ospf neighbor".to_string(),
        };

        assert_eq!(
            InterfacesFetcher.get_command_from_template(&template),
            "show interfaces"
        );
        assert_eq!(
            LldpFetcher.get_command_from_template(&template),
            "show lldp neighbors"
        );
        assert_eq!(
            MacTableFetcher.get_command_from_template(&template),
            "show mac address-table"
        );
        assert_eq!(
            BgpFetcher.get_command_from_template(&template),
            "show ip bgp summary"
        );
        assert_eq!(
            OspfFetcher.get_command_from_template(&template),
            "show ip ospf neighbor"
        );
    }
}
