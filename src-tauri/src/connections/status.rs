use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::fmt;

#[derive(Serialize, Clone, Debug, PartialEq, Eq, Hash)]
#[serde(transparent)]
pub struct ConnectionStatus(String);

impl ConnectionStatus {
    pub fn new(value: String) -> Result<Self, String> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err("ConnectionStatus cannot be empty".to_string());
        }
        if trimmed.len() > 50 {
            return Err("ConnectionStatus cannot exceed 50 characters".to_string());
        }
        Ok(Self(trimmed.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ConnectionStatus {
    type Error = String;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&str> for ConnectionStatus {
    type Error = String;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value.to_string())
    }
}

impl<'de> Deserialize<'de> for ConnectionStatus {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::try_from(s).map_err(serde::de::Error::custom)
    }
}

impl std::ops::Deref for ConnectionStatus {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<str> for ConnectionStatus {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ConnectionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_status() {
        let status = ConnectionStatus::try_from("active").unwrap();
        assert_eq!(status.as_str(), "active");
    }

    #[test]
    fn test_empty_status() {
        assert!(ConnectionStatus::try_from("").is_err());
    }

    #[test]
    fn test_too_long_status() {
        let long_status = "a".repeat(51);
        assert!(ConnectionStatus::try_from(long_status.as_str()).is_err());
    }
}
