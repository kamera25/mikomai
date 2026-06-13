use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::fmt;
use std::net::IpAddr;

#[derive(Serialize, Clone, Debug, PartialEq, Eq, Hash)]
#[serde(transparent)]
pub struct IpAddress(String);

impl IpAddress {
    pub fn new(value: String) -> Result<Self, String> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err("IP Address cannot be empty".to_string());
        }
        // Validate by parsing as standard IpAddr
        let _: IpAddr = trimmed.parse().map_err(|e| format!("Invalid IP address format: {}", e))?;
        Ok(Self(trimmed.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for IpAddress {
    type Error = String;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&str> for IpAddress {
    type Error = String;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value.to_string())
    }
}

impl<'de> Deserialize<'de> for IpAddress {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::try_from(s).map_err(serde::de::Error::custom)
    }
}

impl std::ops::Deref for IpAddress {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<str> for IpAddress {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for IpAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_ipv4() {
        let ip = IpAddress::try_from("192.168.1.1").unwrap();
        assert_eq!(ip.as_str(), "192.168.1.1");
    }

    #[test]
    fn test_valid_ipv6() {
        let ip = IpAddress::try_from("fe80::1").unwrap();
        assert_eq!(ip.as_str(), "fe80::1");
    }

    #[test]
    fn test_invalid_ip() {
        assert!(IpAddress::try_from("192.168.1.300").is_err());
        assert!(IpAddress::try_from("example.com").is_err());
        assert!(IpAddress::try_from("").is_err());
    }
}
