use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationError};

#[derive(Debug, Serialize, Deserialize, Validate, Clone, PartialEq, Eq)]
pub struct UniversalInterfaceTable {
    #[validate(custom(function = "validate_version"))]
    pub version: String,
    #[validate(nested)]
    pub metadata: InterfaceMetadata,
    #[validate(nested)]
    pub interfaces: Vec<InterfaceEntry>,
}

#[derive(Debug, Serialize, Deserialize, Validate, Clone, PartialEq, Eq)]
pub struct InterfaceMetadata {
    #[validate(length(min = 1))]
    pub generated_at: String,
    #[validate(length(min = 1))]
    pub source_device: String,
    #[validate(length(min = 1))]
    pub os_type: String,
}

#[derive(Debug, Serialize, Deserialize, Validate, Clone, PartialEq, Eq)]
pub struct InterfaceEntry {
    #[validate(length(min = 1))]
    pub name: String,
    pub status: InterfaceStatus,
    #[validate(custom(function = "validate_ipv4_addresses"))]
    pub ipv4_addresses: Vec<String>,
    #[validate(range(min = 0, max = 32))]
    pub prefix_len: Option<u8>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum InterfaceStatus {
    Up,
    Down,
    Unknown,
}

fn validate_version(value: &str) -> Result<(), ValidationError> {
    (value == "1.0")
        .then_some(())
        .ok_or_else(|| ValidationError::new("invalid_version"))
}

fn validate_ipv4_addresses(values: &[String]) -> Result<(), ValidationError> {
    for value in values {
        let address = value.split('/').next().unwrap_or(value);
        if address.parse::<std::net::Ipv4Addr>().is_err() {
            return Err(ValidationError::new("invalid_ipv4"));
        }
    }
    Ok(())
}
