use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::fmt;

#[derive(Serialize, Clone, Debug, PartialEq, Eq, Hash)]
#[serde(transparent)]
pub struct ConnectionType(String);

impl ConnectionType {
    pub fn new(value: String) -> Result<Self, String> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err("ConnectionType cannot be empty".to_string());
        }
        if trimmed.len() > 100 {
            return Err("ConnectionType cannot exceed 100 characters".to_string());
        }
        Ok(Self(trimmed.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ConnectionType {
    type Error = String;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&str> for ConnectionType {
    type Error = String;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value.to_string())
    }
}

impl<'de> Deserialize<'de> for ConnectionType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::try_from(s).map_err(serde::de::Error::custom)
    }
}

impl std::ops::Deref for ConnectionType {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<str> for ConnectionType {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ConnectionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_conn_type() {
        let ct = ConnectionType::try_from("Cisco IOS (SSH)").unwrap();
        assert_eq!(ct.as_str(), "Cisco IOS (SSH)");
    }

    #[test]
    fn test_empty_conn_type() {
        assert!(ConnectionType::try_from("").is_err());
    }

    #[test]
    fn test_too_long_conn_type() {
        let long_ct = "a".repeat(101);
        assert!(ConnectionType::try_from(long_ct.as_str()).is_err());
    }
}
