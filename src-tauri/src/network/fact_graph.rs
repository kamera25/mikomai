#![allow(dead_code)]
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::str::FromStr;
use validator::{Validate, ValidationError};

/// Custom validator for verifying that a single string is a valid IPv4 address.
pub fn validate_ipv4_str(ip: &str) -> Result<(), ValidationError>
{
    if Ipv4Addr::from_str(ip).is_ok()
    {
        Ok(())
    }
    else
    {
        Err(ValidationError::new("invalid_ipv4"))
    }
}

/// Custom validator for verifying that all strings in a list are valid IPv4 addresses.
pub fn validate_ipv4_list(ips: &[String]) -> Result<(), ValidationError>
{
    for ip in ips
    {
        validate_ipv4_str(ip)?;
    }
    Ok(())
}

/// Custom validator for verifying network specifications (either plain IPv4 or CIDR prefix notation).
pub fn validate_network_str(net: &str) -> Result<(), ValidationError>
{
    if Ipv4Addr::from_str(net).is_ok()
    {
        return Ok(());
    }

    let parts: Vec<&str> = net.split('/').collect();
    if parts.len() == 2
    {
        if Ipv4Addr::from_str(parts[0]).is_ok()
        {
            if let Ok(prefix) = parts[1].parse::<u8>()
            {
                if prefix <= 32
                {
                    return Ok(());
                }
            }
        }
    }

    Err(ValidationError::new("invalid_network_format"))
}

// 1. FactGraph (Root Struct)
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct FactGraph
{
    #[validate(nested)]
    pub nodes: Vec<Node>,
    #[validate(nested)]
    pub edges: Vec<Edge>,
    #[validate(nested)]
    pub policies: Vec<Policy>,
}

// 2. Node Structure
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct Node
{
    pub id: String,
    pub name: String,
    #[validate(nested)]
    pub interfaces: Vec<Interface>,
    #[validate(nested)]
    pub routing_protocols: Vec<RoutingProtocol>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum InterfaceStatus
{
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct Interface
{
    pub id: String,
    pub name: String,
    pub status: InterfaceStatus,
    #[validate(custom(function = "validate_ipv4_list"))]
    pub ipv4_addresses: Vec<String>,
    #[validate(range(min = 0, max = 32))]
    pub prefix_len: Option<u8>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RoutingProtocolType
{
    Ospf,
    Bgp,
    Static,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct RoutingProtocol
{
    #[serde(rename = "type")]
    pub protocol_type: RoutingProtocolType,
    pub process_id: Option<u32>,
    #[validate(custom(function = "validate_ipv4_str"))]
    pub router_id: Option<String>,
}

// 3. Edge Structure
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EdgeType
{
    RoutingMembership,
    PolicyAttachment,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct Edge
{
    pub id: String,
    pub edge_type: EdgeType,
    pub source: String,
    pub target: String,
    pub properties: HashMap<String, String>,
}

// 4. Policy Structure
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PolicyType
{
    AccessControlList,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct Policy
{
    pub id: String,
    pub policy_type: PolicyType,
    pub name: String,
    #[validate(nested)]
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RuleAction
{
    Permit,
    Deny,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RuleProtocol
{
    Tcp,
    Udp,
    Any,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct Rule
{
    pub sequence: u32,
    pub action: RuleAction,
    pub protocol: RuleProtocol,
    #[validate(custom(function = "validate_network_str"))]
    pub source_network: String,
    #[validate(custom(function = "validate_network_str"))]
    pub destination_network: String,
}

#[cfg(test)]
mod tests
{
    use super::*;

    // Helper to generate a valid template FactGraph
    fn valid_fact_graph() -> FactGraph
    {
        FactGraph {
            nodes: vec![Node {
                id: "node-1".to_string(),
                name: "Router-A".to_string(),
                interfaces: vec![Interface {
                    id: "if-1".to_string(),
                    name: "GigabitEthernet1/1".to_string(),
                    status: InterfaceStatus::Enabled,
                    ipv4_addresses: vec!["192.168.1.1".to_string()],
                    prefix_len: Some(24),
                }],
                routing_protocols: vec![RoutingProtocol {
                    protocol_type: RoutingProtocolType::Ospf,
                    process_id: Some(1),
                    router_id: Some("1.1.1.1".to_string()),
                }],
            }],
            edges: vec![Edge {
                id: "edge-1".to_string(),
                edge_type: EdgeType::RoutingMembership,
                source: "node-1".to_string(),
                target: "ospf-area-0".to_string(),
                properties: {
                    let mut m = HashMap::new();
                    m.insert("area".to_string(), "0".to_string());
                    m
                },
            }],
            policies: vec![Policy {
                id: "policy-1".to_string(),
                policy_type: PolicyType::AccessControlList,
                name: "ACL_INBOUND".to_string(),
                rules: vec![Rule {
                    sequence: 10,
                    action: RuleAction::Permit,
                    protocol: RuleProtocol::Tcp,
                    source_network: "192.168.1.0/24".to_string(),
                    destination_network: "10.0.0.1".to_string(),
                }],
            }],
        }
    }

    #[test]
    fn test_valid_fact_graph()
    {
        let fg = valid_fact_graph();
        assert!(fg.validate().is_ok());
    }

    #[test]
    fn test_invalid_prefix_len()
    {
        let mut fg = valid_fact_graph();
        // prefix_len exceeds max limit (32)
        fg.nodes[0].interfaces[0].prefix_len = Some(35);
        let result = fg.validate();
        assert!(result.is_err());

        let errors = result.unwrap_err();
        let error_map = errors.errors();
        assert!(error_map.contains_key("nodes"));
    }

    #[test]
    fn test_invalid_ipv4_interface()
    {
        let mut fg = valid_fact_graph();
        // Invalid IPv4 address format
        fg.nodes[0].interfaces[0].ipv4_addresses = vec!["999.999.999.999".to_string()];
        let result = fg.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_router_id()
    {
        let mut fg = valid_fact_graph();
        // Invalid router_id format
        fg.nodes[0].routing_protocols[0].router_id = Some("invalid-ip".to_string());
        let result = fg.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_rule_network()
    {
        let mut fg = valid_fact_graph();
        // Invalid CIDR/IP format in rules
        fg.policies[0].rules[0].source_network = "192.168.1.300/24".to_string();
        let result = fg.validate();
        assert!(result.is_err());

        let mut fg2 = valid_fact_graph();
        fg2.policies[0].rules[0].destination_network = "10.0.0.1/35".to_string(); // Invalid prefix_len in CIDR
        let result2 = fg2.validate();
        assert!(result2.is_err());
    }

    #[test]
    fn test_serialization_and_deserialization()
    {
        let fg = valid_fact_graph();
        let serialized = serde_json::to_string(&fg).unwrap();

        // Ensure serialization produces the expected structure
        let deserialized: FactGraph = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.nodes[0].name, "Router-A");
        assert_eq!(
            deserialized.nodes[0].interfaces[0].status,
            InterfaceStatus::Enabled
        );
    }
}
