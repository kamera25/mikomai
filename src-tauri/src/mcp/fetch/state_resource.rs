use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum StateResource {
    #[serde(rename = "arp")]
    Arp,
    #[serde(rename = "routes", alias = "route", alias = "routing")]
    Routes,
    #[serde(rename = "interfaces", alias = "interface", alias = "int")]
    Interfaces,
    #[serde(rename = "lldp", alias = "cdp")]
    Lldp,
    #[serde(
        rename = "mac_table",
        alias = "mac-table",
        alias = "mactable",
        alias = "mac_address_table",
        alias = "mac"
    )]
    MacTable,
    #[serde(rename = "bgp")]
    Bgp,
    #[serde(rename = "ospf")]
    Ospf,
}

impl StateResource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Arp => "arp",
            Self::Routes => "routes",
            Self::Interfaces => "interfaces",
            Self::Lldp => "lldp",
            Self::MacTable => "mac_table",
            Self::Bgp => "bgp",
            Self::Ospf => "ospf",
        }
    }

    pub fn valid_resources() -> &'static [&'static str] {
        &[
            "arp",
            "routes",
            "interfaces",
            "lldp",
            "mac_table",
            "bgp",
            "ospf",
        ]
    }
}

impl fmt::Display for StateResource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for StateResource {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let normalized = s.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "arp" => Ok(Self::Arp),
            "routes" | "route" | "routing" => Ok(Self::Routes),
            "interfaces" | "interface" | "int" | "iface" => Ok(Self::Interfaces),
            "lldp" | "cdp" => Ok(Self::Lldp),
            "mac_table" | "mac-table" | "mactable" | "mac_address_table" | "mac" => {
                Ok(Self::MacTable)
            }
            "bgp" => Ok(Self::Bgp),
            "ospf" => Ok(Self::Ospf),
            _ => Err(format!(
                "Invalid resource '{}'. Supported resources are: {}",
                s,
                Self::valid_resources().join(", ")
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_resource_from_str() {
        assert_eq!("arp".parse::<StateResource>().unwrap(), StateResource::Arp);
        assert_eq!(
            "routes".parse::<StateResource>().unwrap(),
            StateResource::Routes
        );
        assert_eq!(
            "route".parse::<StateResource>().unwrap(),
            StateResource::Routes
        );
        assert_eq!(
            "routing".parse::<StateResource>().unwrap(),
            StateResource::Routes
        );
        assert_eq!(
            "interfaces".parse::<StateResource>().unwrap(),
            StateResource::Interfaces
        );
        assert_eq!(
            "interface".parse::<StateResource>().unwrap(),
            StateResource::Interfaces
        );
        assert_eq!(
            "lldp".parse::<StateResource>().unwrap(),
            StateResource::Lldp
        );
        assert_eq!(
            "mac_table".parse::<StateResource>().unwrap(),
            StateResource::MacTable
        );
        assert_eq!(
            "mac-table".parse::<StateResource>().unwrap(),
            StateResource::MacTable
        );
        assert_eq!(
            "mactable".parse::<StateResource>().unwrap(),
            StateResource::MacTable
        );
        assert_eq!("bgp".parse::<StateResource>().unwrap(), StateResource::Bgp);
        assert_eq!(
            "ospf".parse::<StateResource>().unwrap(),
            StateResource::Ospf
        );

        assert!("unknown_resource".parse::<StateResource>().is_err());
    }

    #[test]
    fn test_state_resource_serialization() {
        assert_eq!(
            serde_json::to_string(&StateResource::Arp).unwrap(),
            "\"arp\""
        );
        assert_eq!(
            serde_json::to_string(&StateResource::Routes).unwrap(),
            "\"routes\""
        );
        assert_eq!(
            serde_json::to_string(&StateResource::MacTable).unwrap(),
            "\"mac_table\""
        );

        let parsed: StateResource = serde_json::from_str("\"route\"").unwrap();
        assert_eq!(parsed, StateResource::Routes);
    }
}
