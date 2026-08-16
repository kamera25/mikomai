use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::fmt;

#[derive(Serialize, Clone, Debug, PartialEq, Eq, Hash)]
#[serde(transparent)]
pub struct DeviceType(String);

impl DeviceType
{
    pub fn new(value: String) -> Result<Self, String>
    {
        let trimmed = value.trim();
        if trimmed.is_empty()
        {
            return Err("DeviceType cannot be empty".to_string());
        }
        if trimmed.len() > 100
        {
            return Err("DeviceType cannot exceed 100 characters".to_string());
        }
        Ok(Self(trimmed.to_string()))
    }

    pub fn as_str(&self) -> &str
    {
        &self.0
    }
}

impl TryFrom<String> for DeviceType
{
    type Error = String;
    fn try_from(value: String) -> Result<Self, Self::Error>
    {
        Self::new(value)
    }
}

impl TryFrom<&str> for DeviceType
{
    type Error = String;
    fn try_from(value: &str) -> Result<Self, Self::Error>
    {
        Self::new(value.to_string())
    }
}

impl<'de> Deserialize<'de> for DeviceType
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::try_from(s).map_err(serde::de::Error::custom)
    }
}

impl std::ops::Deref for DeviceType
{
    type Target = str;
    fn deref(&self) -> &Self::Target
    {
        &self.0
    }
}

impl AsRef<str> for DeviceType
{
    fn as_ref(&self) -> &str
    {
        &self.0
    }
}

impl fmt::Display for DeviceType
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
    fn test_valid_device_type()
    {
        let dt = DeviceType::try_from("cisco_ios").unwrap();
        assert_eq!(dt.as_str(), "cisco_ios");
    }

    #[test]
    fn test_empty_device_type()
    {
        assert!(DeviceType::try_from("").is_err());
    }

    #[test]
    fn test_too_long_device_type()
    {
        let long_dt = "a".repeat(101);
        assert!(DeviceType::try_from(long_dt.as_str()).is_err());
    }
}
