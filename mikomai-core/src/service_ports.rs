//! Shared service-name and transport/port resolution, without OS service databases.
//! Defaults and aliases are maintained in service_ports.json. DNS defaults to TCP
//! for the connection checker; explicit UDP is preserved and never probed as TCP.
use serde::Deserialize;
use std::{collections::HashSet, sync::OnceLock};

#[derive(Debug, Deserialize)]
pub struct ServiceDefinition {
    pub name: String,
    pub port: u16,
    pub protocol: String,
    pub transports: Vec<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ResolvedPort {
    pub port: u16,
    pub protocol: String,
}

/// A separately definable registry; integrations may load their own JSON data.
pub struct ServiceRegistry(Vec<ServiceDefinition>);
impl ServiceRegistry {
    pub fn from_json(json: &str) -> Result<Self, String> {
        let definitions: Vec<ServiceDefinition> =
            serde_json::from_str(json).map_err(|e| e.to_string())?;
        let mut names = HashSet::new();
        for definition in &definitions {
            if definition.port == 0
                || !definition
                    .transports
                    .iter()
                    .any(|p| p == &definition.protocol)
                || definition
                    .transports
                    .iter()
                    .any(|p| !matches!(p.as_str(), "tcp" | "udp"))
            {
                return Err(format!("invalid service definition: {}", definition.name));
            }
            for name in std::iter::once(&definition.name).chain(&definition.aliases) {
                if name.is_empty()
                    || name.chars().all(|c| c.is_ascii_digit())
                    || !name
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || "_-".contains(c))
                    || matches!(name.to_ascii_lowercase().as_str(), "tcp" | "udp")
                    || !names.insert(name.to_ascii_lowercase())
                {
                    return Err(format!("invalid or duplicate service name: {name}"));
                }
            }
        }
        Ok(Self(definitions))
    }

    /// Accept a number, service name, tcp/22, 22/tcp, tcp/dns, or dns/tcp.
    pub fn resolve(&self, spec: &str, protocol: Option<&str>) -> Result<ResolvedPort, String> {
        let spec = spec.trim().to_ascii_lowercase();
        let (value, embedded) = if let Some((left, right)) = spec.split_once('/') {
            let (left, right) = (left.trim(), right.trim());
            if matches!(left, "tcp" | "udp") {
                (right, Some(left))
            } else if matches!(right, "tcp" | "udp") {
                (left, Some(right))
            } else {
                return Err(format!("invalid port specification: {spec}"));
            }
        } else {
            (spec.as_str(), None)
        };
        let protocol = protocol.map(|p| p.trim().to_ascii_lowercase());
        if protocol
            .as_deref()
            .is_some_and(|p| !matches!(p, "tcp" | "udp"))
            || embedded
                .zip(protocol.as_deref())
                .is_some_and(|(a, b)| a != b)
        {
            return Err("invalid or conflicting port transport".into());
        }
        let transport = embedded.or(protocol.as_deref());
        if let Ok(port) = value.parse::<u16>() {
            if port == 0 {
                return Err("port must be between 1 and 65535".into());
            }
            return Ok(ResolvedPort {
                port,
                protocol: transport.unwrap_or("tcp").into(),
            });
        }
        let service = self
            .0
            .iter()
            .find(|s| {
                s.name.eq_ignore_ascii_case(value)
                    || s.aliases.iter().any(|a| a.eq_ignore_ascii_case(value))
            })
            .ok_or_else(|| format!("unknown service or invalid port: {value}"))?;
        let protocol = transport.unwrap_or(&service.protocol);
        if !service.transports.iter().any(|p| p == protocol) {
            return Err(format!("{} does not define {protocol}", service.name));
        }
        Ok(ResolvedPort {
            port: service.port,
            protocol: protocol.into(),
        })
    }
}

pub fn resolve(spec: &str, protocol: Option<&str>) -> Result<ResolvedPort, String> {
    static REGISTRY: OnceLock<ServiceRegistry> = OnceLock::new();
    REGISTRY
        .get_or_init(|| {
            ServiceRegistry::from_json(include_str!("service_ports.json"))
                .expect("valid built-in service definitions")
        })
        .resolve(spec, protocol)
}

/// Normalize model-generated names before planner validation and tool execution.
pub fn normalize_parameters(parameters: &mut serde_json::Value) -> Result<(), String> {
    let spec = parameters.get("port").and_then(|p| p.as_str()).or_else(|| {
        if parameters.get("port").is_none() {
            parameters
                .get("service")
                .or_else(|| parameters.get("query"))
                .and_then(|p| p.as_str())
        } else {
            None
        }
    });
    if let Some(spec) = spec {
        let protocol = parameters
            .get("protocol")
            .map(|p| p.as_str().ok_or("port transport must be a string"))
            .transpose()?;
        let resolved = resolve(spec, protocol)?;
        parameters["port"] = resolved.port.into();
        parameters["protocol"] = resolved.protocol.into();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resolves_names_transports_and_boundaries_without_reinterpreting_udp() {
        for (spec, port, protocol) in [
            ("SSH", 22, "tcp"),
            ("dns", 53, "tcp"),
            ("domain", 53, "tcp"),
            ("udp/22", 22, "udp"),
            ("53/UDP", 53, "udp"),
            ("udp/dns", 53, "udp"),
            ("https/tcp", 443, "tcp"),
            ("ntp", 123, "udp"),
            ("65535", 65535, "tcp"),
        ] {
            assert_eq!(
                resolve(spec, None).unwrap(),
                ResolvedPort {
                    port,
                    protocol: protocol.into()
                }
            );
        }
        for spec in ["0", "65536", "unknown", "udp/ssh", "tcp/22/udp", "ssh;ping"] {
            assert!(resolve(spec, None).is_err(), "{spec}");
        }
        assert!(resolve("udp/22", Some("tcp")).is_err());
    }
    #[test]
    fn custom_definitions_validate_names_and_aliases() {
        let registry = ServiceRegistry::from_json(r#"[{"name":"app","port":12345,"protocol":"tcp","transports":["tcp"],"aliases":["custom"]}]"#).unwrap();
        assert_eq!(registry.resolve("CUSTOM", None).unwrap().port, 12345);
        assert!(ServiceRegistry::from_json(
            r#"[{"name":"app","port":0,"protocol":"tcp","transports":["tcp"]}]"#
        )
        .is_err());
        assert!(ServiceRegistry::from_json(
            r#"[{"name":"app","port":22,"protocol":"tcp","transports":["tcp"],"aliases":["APP"]}]"#
        )
        .is_err());
    }
}
