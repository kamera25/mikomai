use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::str::FromStr;

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(transparent)]
pub struct IpAddress(IpAddr);

impl IpAddress
{
    pub fn new(ip: IpAddr) -> Self
    {
        Self(ip)
    }

    pub fn v4(ip: Ipv4Addr) -> Self
    {
        Self(IpAddr::V4(ip))
    }

    pub fn v6(ip: Ipv6Addr) -> Self
    {
        Self(IpAddr::V6(ip))
    }

    pub fn ip(&self) -> IpAddr
    {
        self.0
    }

    pub fn is_ipv4(&self) -> bool
    {
        self.0.is_ipv4()
    }

    pub fn is_ipv6(&self) -> bool
    {
        self.0.is_ipv6()
    }
}

impl From<IpAddr> for IpAddress
{
    fn from(ip: IpAddr) -> Self
    {
        Self::new(ip)
    }
}

impl From<Ipv4Addr> for IpAddress
{
    fn from(ip: Ipv4Addr) -> Self
    {
        Self::v4(ip)
    }
}

impl From<Ipv6Addr> for IpAddress
{
    fn from(ip: Ipv6Addr) -> Self
    {
        Self::v6(ip)
    }
}

impl FromStr for IpAddress
{
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err>
    {
        let trimmed = s.trim();
        if trimmed.is_empty()
        {
            return Err("IP address cannot be empty".to_string());
        }
        let parsed = trimmed
            .parse::<IpAddr>()
            .map_err(|e| format!("Invalid IP address '{}': {}", s, e))?;
        Ok(Self(parsed))
    }
}

impl TryFrom<&str> for IpAddress
{
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error>
    {
        Self::from_str(value)
    }
}

impl TryFrom<String> for IpAddress
{
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error>
    {
        Self::from_str(&value)
    }
}

impl<'de> Deserialize<'de> for IpAddress
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::try_from(s).map_err(serde::de::Error::custom)
    }
}

impl std::ops::Deref for IpAddress
{
    type Target = IpAddr;

    fn deref(&self) -> &Self::Target
    {
        &self.0
    }
}

impl AsRef<IpAddr> for IpAddress
{
    fn as_ref(&self) -> &IpAddr
    {
        &self.0
    }
}

impl fmt::Display for IpAddress
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result
    {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests
{
    use super::*;

    #[test]
    fn test_valid_ipv4()
    {
        let ip = IpAddress::try_from("192.168.1.1").unwrap();
        assert!(ip.is_ipv4());
        assert!(!ip.is_ipv6());
        assert_eq!(ip.to_string(), "192.168.1.1");
        assert_eq!(ip.ip(), "192.168.1.1".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn test_valid_ipv6()
    {
        let ip = IpAddress::try_from("2001:db8::1").unwrap();
        assert!(!ip.is_ipv4());
        assert!(ip.is_ipv6());
        assert_eq!(ip.to_string(), "2001:db8::1");
        assert_eq!(ip.ip(), "2001:db8::1".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn test_invalid_ip()
    {
        assert!(IpAddress::try_from("").is_err());
        assert!(IpAddress::try_from("   ").is_err());
        assert!(IpAddress::try_from("256.256.256.256").is_err());
        assert!(IpAddress::try_from("invalid-ip").is_err());
        assert!(IpAddress::try_from("192.168.1.1.1").is_err());
    }

    #[test]
    fn test_serialization()
    {
        let ip_v4 = IpAddress::try_from("10.0.0.1").unwrap();
        let serialized_v4 = serde_json::to_string(&ip_v4).unwrap();
        assert_eq!(serialized_v4, r#""10.0.0.1""#);

        let deserialized_v4: IpAddress = serde_json::from_str(r#""10.0.0.1""#).unwrap();
        assert_eq!(deserialized_v4, ip_v4);

        let ip_v6 = IpAddress::try_from("::1").unwrap();
        let serialized_v6 = serde_json::to_string(&ip_v6).unwrap();
        assert_eq!(serialized_v6, r#""::1""#);

        let deserialized_v6: IpAddress = serde_json::from_str(r#""::1""#).unwrap();
        assert_eq!(deserialized_v6, ip_v6);

        assert!(serde_json::from_str::<IpAddress>(r#""invalid""#).is_err());
    }
}
