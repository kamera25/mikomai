use super::fetch_base::{CommandTemplate, McpCommandFetcher};
use crate::network::CommandResult;
use tauri::Manager;

pub(crate) struct ConfigFetcher;

impl McpCommandFetcher for ConfigFetcher {
    fn get_command_from_template(&self, template: &CommandTemplate) -> String {
        template.fetch_config.clone()
    }

    fn get_log_prefix(&self) -> &'static str {
        "config"
    }
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn fetch_config(
    app: tauri::AppHandle,
    device_name: Option<String>,
    deviceName: Option<String>,
    device: Option<String>,
    host: Option<String>,
    user_message: Option<String>,
    userMessage: Option<String>,
) -> Result<CommandResult, String> {
    let resolved_name = crate::mcp::args::normalize_device_args(
        &app,
        device_name,
        deviceName,
        device,
        host,
        user_message,
        userMessage,
    )?;
    let result = ConfigFetcher
        .fetch_device_info(&app, &resolved_name)
        .await?;
    if result.success && !result.output.trim().is_empty() {
        let graph = app.state::<crate::graph::SurrealDbState>();
        graph
            .ingest(crate::graph::GraphIngestInput {
                source_id: "mcp.fetch_config".to_string(),
                collected_at: chrono::Utc::now(),
                device_name: resolved_name,
                kind: crate::graph::GraphDataKind::Config,
                raw: result.output.clone(),
                // A config fetch must never wait for local LLM inference.
                // The immutable raw snapshot is immediately useful for
                // provenance, diffing, and later asynchronous normalization.
                normalized: None,
                canonical: None,
                evidence: None,
                normalizer_version: "config-raw-v1".to_string(),
            })
            .await?;
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::super::command_template::{get_default_templates, get_template_for_dtype};

    #[test]
    fn test_default_templates() {
        let templates = get_default_templates();
        assert!(templates.contains_key("cisco_ios"));
        assert!(templates.contains_key("juniper_junos"));
        assert!(templates.contains_key("arista_eos"));
        assert!(templates.contains_key("yamaha"));
        assert!(templates.contains_key("furukawa_fitelnet"));

        let cisco = templates.get("cisco_ios").unwrap();
        assert_eq!(cisco.fetch_config, "show running-config");
        assert_eq!(cisco.fetch_arp, "show ip arp");

        let yamaha = templates.get("yamaha").unwrap();
        assert_eq!(yamaha.fetch_config, "show config");
        assert_eq!(yamaha.fetch_arp, "show arp");

        let furukawa = templates.get("furukawa_fitelnet").unwrap();
        assert_eq!(furukawa.fetch_config, "show running-config");
        assert_eq!(furukawa.fetch_arp, "show ip arp");
    }

    #[test]
    fn test_get_template_for_dtype() {
        let templates = get_default_templates();

        // Exact match
        let t1 = get_template_for_dtype(&templates, "cisco_ios").unwrap();
        assert_eq!(t1.fetch_config, "show running-config");

        // Substring matches
        let t2 = get_template_for_dtype(&templates, "Cisco IOS Switch").unwrap();
        assert_eq!(t2.fetch_config, "show running-config");

        let t3 = get_template_for_dtype(&templates, "Juniper SRX").unwrap();
        assert_eq!(t3.fetch_config, "show configuration");

        let t4 = get_template_for_dtype(&templates, "Arista EOS").unwrap();
        assert_eq!(t4.fetch_config, "show running-config");

        let t_yamaha = get_template_for_dtype(&templates, "Yamaha RTX").unwrap();
        assert_eq!(t_yamaha.fetch_config, "show config");

        let t_furukawa = get_template_for_dtype(&templates, "Furukawa Fitelnet").unwrap();
        assert_eq!(t_furukawa.fetch_config, "show running-config");

        // Default fallback
        let t5 = get_template_for_dtype(&templates, "unknown_vendor").unwrap();
        assert_eq!(t5.fetch_config, "show running-config");
    }
}
