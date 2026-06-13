use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::fmt;

#[derive(Serialize, Clone, Debug, PartialEq, Eq, Hash)]
#[serde(transparent)]
pub struct ConnectionId(String);

impl ConnectionId {
    pub fn new(value: String) -> Result<Self, String> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err("ConnectionId cannot be empty".to_string());
        }
        if trimmed.len() > 100 {
            return Err("ConnectionId cannot exceed 100 characters".to_string());
        }
        Ok(Self(trimmed.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ConnectionId {
    type Error = String;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&str> for ConnectionId {
    type Error = String;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value.to_string())
    }
}

impl<'de> Deserialize<'de> for ConnectionId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::try_from(s).map_err(serde::de::Error::custom)
    }
}

impl std::ops::Deref for ConnectionId {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<str> for ConnectionId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ConnectionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_id() {
        let id = ConnectionId::try_from("conn-123").unwrap();
        assert_eq!(id.as_str(), "conn-123");
        assert_eq!(format!("{}", id), "conn-123");
    }

    #[test]
    fn test_empty_id() {
        assert!(ConnectionId::try_from("   ").is_err());
        assert!(ConnectionId::try_from("").is_err());
    }

    #[test]
    fn test_too_long_id() {
        let long_id = "a".repeat(101);
        assert!(ConnectionId::try_from(long_id.as_str()).is_err());
    }
}
