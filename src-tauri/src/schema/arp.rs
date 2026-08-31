use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use validator::{Validate, ValidationError};

#[derive(Debug, Serialize, Deserialize, Validate, Clone, PartialEq)]
pub struct UniversalArpTable {
    #[validate(custom(function = "validate_version"))]
    pub version: String,

    #[validate(nested)]
    pub metadata: ArpMetadata,

    #[validate(nested)]
    pub arp_table: Vec<ArpEntry>,
}

fn validate_version(val: &str) -> Result<(), ValidationError> {
    if val == "1.0" {
        Ok(())
    } else {
        Err(ValidationError::new("invalid_version"))
    }
}

use chrono::{DateTime, Utc};

#[derive(Debug, Serialize, Deserialize, Validate, Clone, PartialEq)]
pub struct ArpMetadata {
    pub generated_at: DateTime<Utc>,

    #[validate(length(min = 1))]
    pub source_device: String,

    #[validate(length(min = 1))]
    pub os_type: String,
}

#[derive(Debug, Serialize, Deserialize, Validate, Clone, PartialEq)]
pub struct ArpEntry {
    #[validate(ip)]
    pub ip_address: String,

    #[validate(custom(function = "validate_mac_address"))]
    pub mac_address: Option<String>,

    pub r#type: ArpEntryType,

    #[validate(length(min = 1))]
    pub interface: Option<String>,

    #[validate(range(min = 0))]
    pub age_seconds: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ArpEntryType {
    Dynamic,
    Static,
    Incomplete,
    Permanent,
}

fn validate_mac_address(val: &str) -> Result<(), ValidationError> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"^([0-9a-f]{2}:){5}[0-9a-f]{2}$").unwrap());
    if re.is_match(val) {
        Ok(())
    } else {
        Err(ValidationError::new("invalid_mac_address"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_universal_arp_validation_success() {
        let yaml_content = r#"
version: "1.0"
metadata:
  generated_at: "2026-06-13T13:51:38Z"
  source_device: "Core-Router-01"
  os_type: "routeros"
arp_table:
  - ip_address: "192.168.1.1"
    mac_address: "00:11:22:33:44:55"
    type: "dynamic"
    interface: "Ethernet1"
    age_seconds: 120
  - ip_address: "10.0.0.1"
    mac_address: "aa:bb:cc:dd:ee:ff"
    type: "static"
    interface: "Ethernet2"
"#;
        let parsed: UniversalArpTable = serde_yaml::from_str(yaml_content).unwrap();
        assert!(parsed.validate().is_ok());

        assert_eq!(parsed.version, "1.0");
        assert_eq!(parsed.metadata.source_device, "Core-Router-01");
        assert_eq!(parsed.arp_table[0].ip_address, "192.168.1.1");
        assert_eq!(parsed.arp_table[0].mac_address.as_deref(), Some("00:11:22:33:44:55"));
        assert_eq!(parsed.arp_table[0].r#type, ArpEntryType::Dynamic);
        assert_eq!(parsed.arp_table[0].interface.as_deref(), Some("Ethernet1"));
        assert_eq!(parsed.arp_table[0].age_seconds, Some(120));

        assert_eq!(parsed.arp_table[1].ip_address, "10.0.0.1");
        assert_eq!(parsed.arp_table[1].mac_address.as_deref(), Some("aa:bb:cc:dd:ee:ff"));
        assert_eq!(parsed.arp_table[1].r#type, ArpEntryType::Static);
        assert_eq!(parsed.arp_table[1].interface.as_deref(), Some("Ethernet2"));
        assert_eq!(parsed.arp_table[1].age_seconds, None);
    }

    #[test]
    fn test_universal_arp_validation_fail_invalid_mac() {
        let yaml_content = r#"
version: "1.0"
metadata:
  generated_at: "2026-06-13T13:51:38Z"
  source_device: "Core-Router-01"
  os_type: "routeros"
arp_table:
  - ip_address: "192.168.1.1"
    mac_address: "00-11-22-33-44-55" # invalid separator
    type: "dynamic"
    interface: "Ethernet1"
"#;
        let parsed: UniversalArpTable = serde_yaml::from_str(yaml_content).unwrap();
        let validation_res = parsed.validate();
        assert!(validation_res.is_err());
        let errors = validation_res.unwrap_err().to_string();
        assert!(errors.contains("mac_address"));
    }

    #[test]
    fn test_universal_arp_validation_fail_uppercase_mac() {
        let yaml_content = r#"
version: "1.0"
metadata:
  generated_at: "2026-06-13T13:51:38Z"
  source_device: "Core-Router-01"
  os_type: "routeros"
arp_table:
  - ip_address: "192.168.1.1"
    mac_address: "00:11:22:33:44:AA" # uppercase not allowed
    type: "dynamic"
    interface: "Ethernet1"
"#;
        let parsed: UniversalArpTable = serde_yaml::from_str(yaml_content).unwrap();
        let validation_res = parsed.validate();
        assert!(validation_res.is_err());
    }

    #[test]
    fn test_universal_arp_validation_fail_invalid_ip() {
        let yaml_content = r#"
version: "1.0"
metadata:
  generated_at: "2026-06-13T13:51:38Z"
  source_device: "Core-Router-01"
  os_type: "routeros"
arp_table:
  - ip_address: "999.999.999.999" # invalid IP
    mac_address: "00:11:22:33:44:55"
    type: "dynamic"
    interface: "Ethernet1"
"#;
        let parsed: UniversalArpTable = serde_yaml::from_str(yaml_content).unwrap();
        let validation_res = parsed.validate();
        assert!(validation_res.is_err());
    }

    #[test]
    fn test_universal_arp_validation_fail_invalid_version() {
        let yaml_content = r#"
version: "2.0" # only "1.0" allowed
metadata:
  generated_at: "2026-06-13T13:51:38Z"
  source_device: "Core-Router-01"
  os_type: "routeros"
arp_table:
  - ip_address: "192.168.1.1"
    mac_address: "00:11:22:33:44:55"
    type: "dynamic"
    interface: "Ethernet1"
"#;
        let parsed: UniversalArpTable = serde_yaml::from_str(yaml_content).unwrap();
        let validation_res = parsed.validate();
        assert!(validation_res.is_err());
    }

    #[test]
    fn test_universal_arp_validation_fail_invalid_timestamp() {
        let yaml_content = r#"
version: "1.0"
metadata:
  generated_at: "2026-06-13 13:51:38" # not ISO 8601 / RFC 3339 format
  source_device: "Core-Router-01"
  os_type: "routeros"
arp_table:
  - ip_address: "192.168.1.1"
    mac_address: "00:11:22:33:44:55"
    type: "dynamic"
    interface: "Ethernet1"
"#;
        let parsed_res: Result<UniversalArpTable, _> = serde_yaml::from_str(yaml_content);
        assert!(parsed_res.is_err());
    }
}
