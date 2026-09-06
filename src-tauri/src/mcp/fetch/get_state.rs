use super::fetch_base::{CommandTemplate, McpCommandFetcher};
use super::state_resource::StateResource;
use crate::graph::{GraphDataKind, GraphIngestInput, SurrealDbState};
use crate::network::CommandResult;
use regex::Regex;
use std::str::FromStr;
use tauri::Manager;

pub(crate) struct InterfacesFetcher;
impl McpCommandFetcher for InterfacesFetcher {
    fn get_commands_from_template(&self, template: &CommandTemplate) -> Vec<String> {
        if !template.fetch_interfaces.is_empty() {
            template.fetch_interfaces.to_vec()
        } else {
            vec!["show interfaces".to_string()]
        }
    }

    fn get_command_from_template(&self, template: &CommandTemplate) -> String {
        self.get_commands_from_template(template).into_iter().next().unwrap_or_default()
    }

    fn get_log_prefix(&self) -> &'static str {
        "interfaces"
    }
}

pub(crate) struct LldpFetcher;
impl McpCommandFetcher for LldpFetcher {
    fn get_commands_from_template(&self, template: &CommandTemplate) -> Vec<String> {
        if !template.fetch_lldp.is_empty() {
            template.fetch_lldp.to_vec()
        } else {
            vec!["show lldp neighbors".to_string()]
        }
    }

    fn get_command_from_template(&self, template: &CommandTemplate) -> String {
        self.get_commands_from_template(template).into_iter().next().unwrap_or_default()
    }

    fn get_log_prefix(&self) -> &'static str {
        "lldp"
    }
}

pub(crate) struct MacTableFetcher;
impl McpCommandFetcher for MacTableFetcher {
    fn get_commands_from_template(&self, template: &CommandTemplate) -> Vec<String> {
        if !template.fetch_mac_table.is_empty() {
            template.fetch_mac_table.to_vec()
        } else {
            vec!["show mac address-table".to_string()]
        }
    }

    fn get_command_from_template(&self, template: &CommandTemplate) -> String {
        self.get_commands_from_template(template).into_iter().next().unwrap_or_default()
    }

    fn get_log_prefix(&self) -> &'static str {
        "mac_table"
    }
}

pub(crate) struct BgpFetcher;
impl McpCommandFetcher for BgpFetcher {
    fn get_commands_from_template(&self, template: &CommandTemplate) -> Vec<String> {
        if !template.fetch_bgp.is_empty() {
            template.fetch_bgp.to_vec()
        } else {
            vec!["show ip bgp summary".to_string()]
        }
    }

    fn get_command_from_template(&self, template: &CommandTemplate) -> String {
        self.get_commands_from_template(template).into_iter().next().unwrap_or_default()
    }

    fn get_log_prefix(&self) -> &'static str {
        "bgp"
    }
}

pub(crate) struct OspfFetcher;
impl McpCommandFetcher for OspfFetcher {
    fn get_commands_from_template(&self, template: &CommandTemplate) -> Vec<String> {
        if !template.fetch_ospf.is_empty() {
            template.fetch_ospf.to_vec()
        } else {
            vec!["show ip ospf neighbor".to_string()]
        }
    }

    fn get_command_from_template(&self, template: &CommandTemplate) -> String {
        self.get_commands_from_template(template).into_iter().next().unwrap_or_default()
    }

    fn get_log_prefix(&self) -> &'static str {
        "ospf"
    }
}

/// Read-only CPU primitive.  The command is selected from the existing
/// per-vendor template; only its small, structured result is exposed to Watch.
pub struct CpuFetcher;
impl McpCommandFetcher for CpuFetcher {
    fn get_commands_from_template(&self, template: &CommandTemplate) -> Vec<String> {
        if !template.fetch_cpu.is_empty() {
            template.fetch_cpu.to_vec()
        } else {
            vec!["show processes cpu".to_string()]
        }
    }

    fn get_command_from_template(&self, template: &CommandTemplate) -> String {
        self.get_commands_from_template(template).into_iter().next().unwrap_or_default()
    }

    fn get_log_prefix(&self) -> &'static str {
        "cpu"
    }
}

pub fn parse_cpu_usage(output: &str) -> Result<f64, String> {
    let patterns = [
        r"(?i)cpu\s+utilization[^\n:]*:\s*(\d+(?:\.\d+)?)\s*%",
        r"(?i)cpu[^\n]*?\b(\d+(?:\.\d+)?)\s*(?:%|percent)\b",
    ];
    for pattern in patterns {
        let regex = Regex::new(pattern).expect("CPU parser regex is valid");
        if let Some(captures) = regex.captures(output) {
            let usage = captures[1]
                .parse::<f64>()
                .map_err(|_| "CPU usage is not numeric".to_string())?;
            if (0.0..=100.0).contains(&usage) {
                return Ok(usage);
            }
        }
    }
    Err("Could not parse CPU usage from device output".to_string())
}

pub async fn fetch_cpu_usage(app: &tauri::AppHandle, device_name: &str) -> Result<f64, String> {
    let result = CpuFetcher.fetch_device_info(app, device_name).await?;
    if !result.success {
        return Err(result.output);
    }
    parse_cpu_usage(&result.output)
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
                canonical: None,
                evidence: None,
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
        StateResource::Interfaces => fetch_and_canonicalize_interfaces(app, device_name).await,
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
        StateResource::Cpu => {
            let usage = fetch_cpu_usage(app, device_name).await?;
            Ok(CommandResult {
                success: true,
                output: serde_json::json!({ "usage": usage }).to_string(),
                saved_path: None,
                is_cached: None,
                cache_time: None,
            })
        }
    }
}

async fn fetch_and_canonicalize_interfaces(
    app: &tauri::AppHandle,
    device_name: &str,
) -> Result<CommandResult, String> {
    let graph = app.state::<SurrealDbState>();
    if let Some(canonical) = graph
        .fresh_canonical(device_name, GraphDataKind::Interfaces)
        .await?
    {
        return Ok(CommandResult {
            success: true,
            output: serde_json::to_string_pretty(&canonical)
                .unwrap_or_else(|_| canonical.to_string()),
            saved_path: None,
            is_cached: Some(true),
            cache_time: None,
        });
    }
    let result = InterfacesFetcher
        .fetch_device_info(app, device_name)
        .await?;
    if !result.success || result.output.trim().is_empty() {
        return Ok(result);
    }
    graph
        .ingest(GraphIngestInput {
            source_id: "mcp.get_state.interfaces".to_string(),
            collected_at: chrono::Utc::now(),
            device_name: device_name.to_string(),
            kind: GraphDataKind::Interfaces,
            raw: result.output.clone(),
            normalized: None,
            canonical: None,
            evidence: None,
            normalizer_version: "interfaces-raw-v1".to_string(),
        })
        .await?;
    crate::graph::canonicalize_interfaces_on_read(app, &graph, device_name).await?;
    let canonical = graph
        .fresh_canonical(device_name, GraphDataKind::Interfaces)
        .await?
        .ok_or_else(|| {
            "interface canonicalization did not produce a canonical observation".to_string()
        })?;
    Ok(CommandResult {
        success: true,
        output: serde_json::to_string_pretty(&canonical).unwrap_or_else(|_| canonical.to_string()),
        saved_path: None,
        is_cached: Some(false),
        cache_time: None,
    })
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
    let resolved_device_arg = device.or(deviceName).or(device_name).or(host).or(target);

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

    let raw_resource = resource.or(resourceType).or(resource_type).ok_or_else(|| {
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
            fetch_config: "show run".into(),
            fetch_route: "show ip route".into(),
            fetch_bgp: "show ip bgp summary".into(),
            fetch_arp: "show ip arp".into(),
            fetch_interfaces: "show interfaces".into(),
            fetch_lldp: "show lldp neighbors".into(),
            fetch_mac_table: "show mac address-table".into(),
            fetch_ospf: "show ip ospf neighbor".into(),
            fetch_cpu: "show processes cpu".into(),
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
        assert_eq!(
            CpuFetcher.get_command_from_template(&template),
            "show processes cpu"
        );

        // Test array commands
        let multi_template = CommandTemplate {
            fetch_interfaces: vec!["show interfaces".to_string(), "show ip status".to_string()].into(),
            ..Default::default()
        };
        assert_eq!(
            InterfacesFetcher.get_commands_from_template(&multi_template),
            vec!["show interfaces", "show ip status"]
        );
    }

    #[test]
    fn parses_common_cpu_formats() {
        assert_eq!(
            parse_cpu_usage("CPU utilization for five seconds: 82%/10%").unwrap(),
            82.0
        );
        assert_eq!(
            parse_cpu_usage("CPU utilization: 17 percent").unwrap(),
            17.0
        );
        assert!(parse_cpu_usage("no cpu data").is_err());
    }
}
