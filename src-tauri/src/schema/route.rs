use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationError};

#[derive(Debug, Serialize, Deserialize, Validate, Clone, PartialEq)]
pub struct UniversalRouteTable
{
    #[validate(custom(function = "validate_version"))]
    pub version: String,

    #[validate(nested)]
    pub metadata: RouteMetadata,

    #[validate(nested)]
    pub routes: Vec<RouteEntry>,
}

fn validate_version(val: &str) -> Result<(), ValidationError>
{
    if val == "1.0"
    {
        Ok(())
    }
    else
    {
        Err(ValidationError::new("invalid_version"))
    }
}

#[derive(Debug, Serialize, Deserialize, Validate, Clone, PartialEq)]
pub struct RouteMetadata
{
    #[validate(custom(function = "validate_iso8601"))]
    pub generated_at: String,

    #[validate(length(min = 1))]
    pub source_device: String,

    #[validate(length(min = 1))]
    pub os_type: String,
}

fn validate_iso8601(val: &str) -> Result<(), ValidationError>
{
    if chrono::DateTime::parse_from_rfc3339(val).is_ok()
    {
        Ok(())
    }
    else
    {
        Err(ValidationError::new("invalid_iso8601"))
    }
}

#[derive(Debug, Serialize, Deserialize, Validate, Clone, PartialEq)]
pub struct RouteEntry
{
    #[validate(length(min = 1))]
    pub destination: String,

    #[validate(length(min = 1))]
    pub gateway: String,

    pub flags: Option<String>,

    #[validate(length(min = 1))]
    pub interface: String,

    #[validate(range(min = 0))]
    pub metric: Option<i32>,
}

#[cfg(test)]
mod tests
{
    use super::*;

    #[test]
    fn test_universal_route_validation_success()
    {
        let yaml_content = r#"
version: "1.0"
metadata:
  generated_at: "2026-06-13T13:51:38Z"
  source_device: "Core-Router-01"
  os_type: "routeros"
routes:
  - destination: "default"
    gateway: "192.168.1.1"
    flags: "UG"
    interface: "Ethernet1"
    metric: 10
  - destination: "192.168.1.0/24"
    gateway: "link#11"
    flags: "U"
    interface: "Ethernet2"
"#;
        let parsed: UniversalRouteTable = serde_yaml::from_str(yaml_content).unwrap();
        assert!(parsed.validate().is_ok());

        assert_eq!(parsed.version, "1.0");
        assert_eq!(parsed.metadata.source_device, "Core-Router-01");
        assert_eq!(parsed.routes[0].destination, "default");
        assert_eq!(parsed.routes[0].gateway, "192.168.1.1");
        assert_eq!(parsed.routes[0].flags, Some("UG".to_string()));
        assert_eq!(parsed.routes[0].interface, "Ethernet1");
        assert_eq!(parsed.routes[0].metric, Some(10));

        assert_eq!(parsed.routes[1].destination, "192.168.1.0/24");
        assert_eq!(parsed.routes[1].gateway, "link#11");
        assert_eq!(parsed.routes[1].flags, Some("U".to_string()));
        assert_eq!(parsed.routes[1].interface, "Ethernet2");
        assert_eq!(parsed.routes[1].metric, None);
    }

    #[test]
    fn test_universal_route_validation_fail_invalid_version()
    {
        let yaml_content = r#"
version: "2.0"
metadata:
  generated_at: "2026-06-13T13:51:38Z"
  source_device: "Core-Router-01"
  os_type: "routeros"
routes:
  - destination: "default"
    gateway: "192.168.1.1"
    interface: "Ethernet1"
"#;
        let parsed: UniversalRouteTable = serde_yaml::from_str(yaml_content).unwrap();
        let validation_res = parsed.validate();
        assert!(validation_res.is_err());
    }

    #[test]
    fn test_universal_route_validation_fail_invalid_timestamp()
    {
        let yaml_content = r#"
version: "1.0"
metadata:
  generated_at: "2026-06-13 13:51:38"
  source_device: "Core-Router-01"
  os_type: "routeros"
routes:
  - destination: "default"
    gateway: "192.168.1.1"
    interface: "Ethernet1"
"#;
        let parsed: UniversalRouteTable = serde_yaml::from_str(yaml_content).unwrap();
        let validation_res = parsed.validate();
        assert!(validation_res.is_err());
    }
}
