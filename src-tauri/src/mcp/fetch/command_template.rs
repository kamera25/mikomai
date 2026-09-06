use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::PathBuf;
use tauri::Manager;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TemplateCommands(pub Vec<String>);

impl TemplateCommands {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn to_vec(&self) -> Vec<String> {
        self.0.clone()
    }

    #[allow(dead_code)]
    pub fn as_slice(&self) -> &[String] {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty() || self.0.iter().all(|s| s.trim().is_empty())
    }

    #[allow(dead_code)]
    pub fn first_command(&self) -> Option<&str> {
        self.0.iter().find(|s| !s.trim().is_empty()).map(|s| s.as_str())
    }
}

impl std::ops::Deref for TemplateCommands {
    type Target = [String];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<&str> for TemplateCommands {
    fn from(s: &str) -> Self {
        if s.trim().is_empty() {
            Self(Vec::new())
        } else {
            Self(vec![s.to_string()])
        }
    }
}

impl From<String> for TemplateCommands {
    fn from(s: String) -> Self {
        if s.trim().is_empty() {
            Self(Vec::new())
        } else {
            Self(vec![s])
        }
    }
}

impl From<Vec<String>> for TemplateCommands {
    fn from(v: Vec<String>) -> Self {
        Self(v)
    }
}

impl From<&[&str]> for TemplateCommands {
    fn from(v: &[&str]) -> Self {
        Self(v.iter().map(|s| s.to_string()).collect())
    }
}

impl PartialEq<&str> for TemplateCommands {
    fn eq(&self, other: &&str) -> bool {
        if self.0.len() == 1 {
            self.0[0] == *other
        } else if self.0.is_empty() {
            other.trim().is_empty()
        } else {
            false
        }
    }
}

impl PartialEq<str> for TemplateCommands {
    fn eq(&self, other: &str) -> bool {
        if self.0.len() == 1 {
            self.0[0] == other
        } else if self.0.is_empty() {
            other.trim().is_empty()
        } else {
            false
        }
    }
}

impl PartialEq<String> for TemplateCommands {
    fn eq(&self, other: &String) -> bool {
        self.eq(other.as_str())
    }
}

impl fmt::Display for TemplateCommands {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.len() == 1 {
            write!(f, "{}", self.0[0])
        } else {
            write!(f, "{:?}", self.0)
        }
    }
}

impl Serialize for TemplateCommands {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if self.0.is_empty() {
            serializer.serialize_str("")
        } else if self.0.len() == 1 {
            serializer.serialize_str(&self.0[0])
        } else {
            self.0.serialize(serializer)
        }
    }
}

impl<'de> Deserialize<'de> for TemplateCommands {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct TemplateCommandsVisitor;

        impl<'de> serde::de::Visitor<'de> for TemplateCommandsVisitor {
            type Value = TemplateCommands;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a string or a sequence of strings")
            }

            fn visit_str<E>(self, value: &str) -> Result<TemplateCommands, E>
            where
                E: serde::de::Error,
            {
                if value.trim().is_empty() {
                    Ok(TemplateCommands(Vec::new()))
                } else {
                    Ok(TemplateCommands(vec![value.to_string()]))
                }
            }

            fn visit_string<E>(self, value: String) -> Result<TemplateCommands, E>
            where
                E: serde::de::Error,
            {
                self.visit_str(&value)
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<TemplateCommands, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut vec = Vec::new();
                while let Some(element) = seq.next_element::<String>()? {
                    if !element.trim().is_empty() {
                        vec.push(element);
                    }
                }
                Ok(TemplateCommands(vec))
            }

            fn visit_none<E>(self) -> Result<TemplateCommands, E>
            where
                E: serde::de::Error,
            {
                Ok(TemplateCommands(Vec::new()))
            }

            fn visit_unit<E>(self) -> Result<TemplateCommands, E>
            where
                E: serde::de::Error,
            {
                Ok(TemplateCommands(Vec::new()))
            }
        }

        deserializer.deserialize_any(TemplateCommandsVisitor)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct CommandTemplate {
    #[serde(default)]
    pub fetch_config: TemplateCommands,
    #[serde(default)]
    pub fetch_route: TemplateCommands,
    #[serde(default)]
    pub fetch_bgp: TemplateCommands,
    #[serde(default)]
    pub fetch_arp: TemplateCommands,
    #[serde(default)]
    pub fetch_interfaces: TemplateCommands,
    #[serde(default)]
    pub fetch_lldp: TemplateCommands,
    #[serde(default)]
    pub fetch_mac_table: TemplateCommands,
    #[serde(default)]
    pub fetch_ospf: TemplateCommands,
    #[serde(default)]
    pub fetch_cpu: TemplateCommands,
}

pub type CommandTemplates = HashMap<String, CommandTemplate>;

pub fn get_templates_path(app: &tauri::AppHandle) -> PathBuf {
    let path = app
        .path()
        .app_data_dir()
        .expect("Failed to get app data dir");
    if !path.exists() {
        let _ = fs::create_dir_all(&path);
    }
    path.join("command_templates.json")
}

pub fn get_default_templates() -> CommandTemplates {
    const DEFAULT_YAML: &str = include_str!("../config/default_templates.yaml");
    serde_yaml::from_str(DEFAULT_YAML).expect("Failed to parse default_templates.yaml")
}

pub fn load_templates(app: &tauri::AppHandle) -> CommandTemplates {
    let mut defaults = get_default_templates();
    let path = get_templates_path(app);
    if !path.exists() {
        if let Ok(data) = serde_json::to_string_pretty(&defaults) {
            let _ = fs::write(&path, data);
        }
        defaults
    } else {
        let loaded: CommandTemplates = match fs::read_to_string(&path) {
            Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
            Err(_) => HashMap::new(),
        };
        // Merge loaded with defaults (fill in missing vendors and empty fields)
        for (vendor, loaded_template) in loaded {
            let entry = defaults
                .entry(vendor)
                .or_insert_with(CommandTemplate::default);
            if !loaded_template.fetch_config.is_empty() {
                entry.fetch_config = loaded_template.fetch_config;
            }
            if !loaded_template.fetch_route.is_empty() {
                entry.fetch_route = loaded_template.fetch_route;
            }
            if !loaded_template.fetch_bgp.is_empty() {
                entry.fetch_bgp = loaded_template.fetch_bgp;
            }
            if !loaded_template.fetch_arp.is_empty() {
                entry.fetch_arp = loaded_template.fetch_arp;
            }
            if !loaded_template.fetch_interfaces.is_empty() {
                entry.fetch_interfaces = loaded_template.fetch_interfaces;
            }
            if !loaded_template.fetch_lldp.is_empty() {
                entry.fetch_lldp = loaded_template.fetch_lldp;
            }
            if !loaded_template.fetch_mac_table.is_empty() {
                entry.fetch_mac_table = loaded_template.fetch_mac_table;
            }
            if !loaded_template.fetch_ospf.is_empty() {
                entry.fetch_ospf = loaded_template.fetch_ospf;
            }
            if !loaded_template.fetch_cpu.is_empty() {
                entry.fetch_cpu = loaded_template.fetch_cpu;
            }
        }
        defaults
    }
}

pub fn get_template_for_dtype<'a>(
    templates: &'a CommandTemplates,
    dtype: &str,
) -> Option<&'a CommandTemplate> {
    let dtype_lower = dtype.to_lowercase();
    if templates.contains_key(&dtype_lower) {
        return templates.get(&dtype_lower);
    }

    let mapped = map_vendor_type(dtype);
    templates
        .get(&mapped)
        .or_else(|| templates.get("cisco_ios"))
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FallbackConfig {
    pub device_type: Option<String>,
    pub command: String,
    pub description: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VendorConfig {
    pub command: String,
    pub description: Option<String>,
    pub aliases: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ShowRunningConfigRules {
    pub fallback: FallbackConfig,
    pub vendors: HashMap<String, VendorConfig>,
}

use std::sync::LazyLock;

static SHOW_RUNNING_CONFIG_RULES: LazyLock<Option<ShowRunningConfigRules>> = LazyLock::new(|| {
    let yaml_content =
        std::fs::read_to_string("src-tauri/src/mcp/config/show_running_config_commands.yaml")
            .or_else(|_| {
                std::fs::read_to_string("src/mcp/config/show_running_config_commands.yaml")
            })
            .unwrap_or_else(|_| {
                include_str!("../config/show_running_config_commands.yaml").to_string()
            });
    serde_yaml::from_str(&yaml_content).ok()
});

static APPLY_CONFIG_RULES: LazyLock<Option<ApplyConfigRules>> = LazyLock::new(|| {
    let yaml_content =
        std::fs::read_to_string("src-tauri/src/mcp/config/apply_config_commands.yaml")
            .or_else(|_| std::fs::read_to_string("src/mcp/config/apply_config_commands.yaml"))
            .unwrap_or_else(|_| include_str!("../config/apply_config_commands.yaml").to_string());
    serde_yaml::from_str(&yaml_content).ok()
});

pub fn get_show_running_config_command(device_type: &str) -> String {
    if let Some(rules) = SHOW_RUNNING_CONFIG_RULES.as_ref() {
        let dt_lower = device_type.to_lowercase();
        if let Some(v) = rules.vendors.get(&dt_lower) {
            return v.command.clone();
        }
        for (vendor_key, v) in &rules.vendors {
            if dt_lower.contains(vendor_key) {
                return v.command.clone();
            }
            if let Some(aliases) = &v.aliases {
                for alias in aliases {
                    if dt_lower == alias.to_lowercase() || dt_lower.contains(&alias.to_lowercase())
                    {
                        return v.command.clone();
                    }
                }
            }
        }
        return rules.fallback.command.clone();
    }

    "show running-config".to_string()
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FallbackApplySaveConfig {
    pub device_type: Option<String>,
    pub apply_command: Option<String>,
    pub save_command: Option<String>,
    pub command: Option<String>,
    pub description: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VendorApplySaveConfig {
    pub apply_command: Option<String>,
    pub save_command: Option<String>,
    pub command: Option<String>,
    pub description: Option<String>,
    pub aliases: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ApplyConfigRules {
    pub fallback: FallbackApplySaveConfig,
    pub vendors: HashMap<String, VendorApplySaveConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyAndSaveCommands {
    pub apply_command: String,
    pub save_command: String,
}

pub fn get_apply_and_save_config_commands(device_type: &str) -> ApplyAndSaveCommands {
    if let Some(rules) = APPLY_CONFIG_RULES.as_ref() {
        let dt_lower = device_type.to_lowercase();

        let find_vendor = rules.vendors.get(&dt_lower).or_else(|| {
            rules.vendors.values().find(|v| {
                if let Some(aliases) = &v.aliases {
                    aliases.iter().any(|a| {
                        dt_lower == a.to_lowercase() || dt_lower.contains(&a.to_lowercase())
                    })
                } else {
                    false
                }
            })
        });

        if let Some(v) = find_vendor {
            let apply = v
                .apply_command
                .clone()
                .or_else(|| v.command.clone())
                .unwrap_or_default();
            let save = v.save_command.clone().unwrap_or_default();
            return ApplyAndSaveCommands {
                apply_command: apply,
                save_command: save,
            };
        }

        let fallback_apply = rules
            .fallback
            .apply_command
            .clone()
            .or_else(|| rules.fallback.command.clone())
            .unwrap_or_default();
        let fallback_save = rules.fallback.save_command.clone().unwrap_or_default();
        return ApplyAndSaveCommands {
            apply_command: fallback_apply,
            save_command: fallback_save,
        };
    }

    ApplyAndSaveCommands {
        apply_command: String::new(),
        save_command: "write memory".to_string(),
    }
}

pub fn map_vendor_type(conn_type: &str) -> String {
    let conn_type_trimmed = conn_type.trim();
    if let Some(brand) = crate::mcp::brands::get_brand(conn_type_trimmed) {
        return brand.to_string();
    }

    if let Some((brand, _)) = crate::mcp::brands::detect_brand_in_text(conn_type_trimmed) {
        return brand.to_string();
    }

    // フェイルオーバーとして「Cisco IOS」を選択
    "cisco_ios".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_show_running_config_command() {
        assert_eq!(
            get_show_running_config_command("furukawa_fitelnet"),
            "show running.cfg"
        );
        assert_eq!(
            get_show_running_config_command("cisco_ios"),
            "show running-config"
        );
        assert_eq!(
            get_show_running_config_command("juniper_junos"),
            "show configuration"
        );
        assert_eq!(get_show_running_config_command("yamaha"), "show config");
        assert_eq!(
            get_show_running_config_command("unknown_device_type"),
            "show running-config"
        );
    }

    #[test]
    fn test_get_apply_and_save_config_commands() {
        assert_eq!(
            get_apply_and_save_config_commands("furukawa_fitelnet"),
            ApplyAndSaveCommands {
                apply_command: "commit".to_string(),
                save_command: "save moff".to_string()
            }
        );
        assert_eq!(
            get_apply_and_save_config_commands("cisco_ios"),
            ApplyAndSaveCommands {
                apply_command: "".to_string(),
                save_command: "write memory".to_string()
            }
        );
        assert_eq!(
            get_apply_and_save_config_commands("juniper_junos"),
            ApplyAndSaveCommands {
                apply_command: "commit".to_string(),
                save_command: "".to_string()
            }
        );
        assert_eq!(
            get_apply_and_save_config_commands("yamaha"),
            ApplyAndSaveCommands {
                apply_command: "".to_string(),
                save_command: "save".to_string()
            }
        );
        assert_eq!(
            get_apply_and_save_config_commands("unknown_device_type"),
            ApplyAndSaveCommands {
                apply_command: "".to_string(),
                save_command: "write memory".to_string()
            }
        );
    }

    #[test]
    fn test_map_vendor_type() {
        assert_eq!(map_vendor_type("Cisco IOS"), "cisco_ios");
        assert_eq!(map_vendor_type("cisco"), "cisco_ios");
        assert_eq!(map_vendor_type("Juniper"), "juniper_junos");
        assert_eq!(map_vendor_type("Fortigate"), "fortinet");
        assert_eq!(map_vendor_type("Yamaha"), "yamaha");
        assert_eq!(map_vendor_type("Furukawa"), "furukawa_fitelnet");
        assert_eq!(map_vendor_type("A10"), "a10");
        assert_eq!(map_vendor_type("PaloAlto"), "paloalto_panos");
        assert_eq!(map_vendor_type("unknown_device"), "cisco_ios");
    }

    #[test]
    fn test_template_commands_serde() {
        // Test single string
        let yaml_single = "fetch_config: \"show run\"\n";
        #[derive(Deserialize, Serialize)]
        struct TestConfig {
            fetch_config: TemplateCommands,
        }
        let parsed: TestConfig = serde_yaml::from_str(yaml_single).unwrap();
        assert_eq!(parsed.fetch_config, "show run");
        assert_eq!(parsed.fetch_config.to_vec(), vec!["show run"]);

        // Test array of strings
        let yaml_array = "fetch_config:\n  - \"show interface\"\n  - \"show ip status\"\n";
        let parsed_array: TestConfig = serde_yaml::from_str(yaml_array).unwrap();
        assert_eq!(
            parsed_array.fetch_config.to_vec(),
            vec!["show interface", "show ip status"]
        );
        assert!(!parsed_array.fetch_config.is_empty());

        // Test serialization
        let serialized_single = serde_json::to_string(&parsed).unwrap();
        assert_eq!(serialized_single, "{\"fetch_config\":\"show run\"}");

        let serialized_array = serde_json::to_string(&parsed_array).unwrap();
        assert_eq!(
            serialized_array,
            "{\"fetch_config\":[\"show interface\",\"show ip status\"]}"
        );
    }
}
