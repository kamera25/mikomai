use crate::network::CommandResult;
use super::fetch_base::{McpCommandFetcher, CommandTemplate};

struct ConfigFetcher;

impl McpCommandFetcher for ConfigFetcher {
    fn get_command_from_template(&self, template: &CommandTemplate) -> String {
        template.fetch_config.clone()
    }
    
    fn get_log_prefix(&self) -> &'static str {
        "config"
    }
}

#[tauri::command]
pub async fn fetch_config(app: tauri::AppHandle, device_name: Option<String>) -> Result<CommandResult, String> {
    let device_name = match device_name {
        Some(name) if !name.trim().is_empty() => name,
        _ => return Err("Error: device_name (機器名) is required but was not provided or is empty.".to_string()),
    };
    ConfigFetcher.fetch_device_info(&app, &device_name).await
}


#[cfg(test)]
mod tests {
    use super::super::fetch_base::{get_default_templates, get_template_for_dtype};

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
