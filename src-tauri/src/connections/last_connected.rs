use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::fmt;

#[derive(Serialize, Clone, Debug, PartialEq, Eq, Hash)]
#[serde(transparent)]
pub struct LastConnected(String);

impl LastConnected {
    pub fn new(value: String) -> Result<Self, String> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err("LastConnected cannot be empty".to_string());
        }
        if trimmed.len() > 50 {
            return Err("LastConnected cannot exceed 50 characters".to_string());
        }
        Ok(Self(trimmed.to_string()))
    }

    #[allow(dead_code)]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for LastConnected {
    type Error = String;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&str> for LastConnected {
    type Error = String;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value.to_string())
    }
}

impl<'de> Deserialize<'de> for LastConnected {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::try_from(s).map_err(serde::de::Error::custom)
    }
}

impl std::ops::Deref for LastConnected {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<str> for LastConnected {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for LastConnected {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_last_connected() {
        let lc = LastConnected::try_from("2026-06-13").unwrap();
        assert_eq!(lc.as_str(), "2026-06-13");
    }

    #[test]
    fn test_empty_last_connected() {
        assert!(LastConnected::try_from("").is_err());
    }

    #[test]
    fn test_too_long_last_connected() {
        let long_lc = "a".repeat(51);
        assert!(LastConnected::try_from(long_lc.as_str()).is_err());
    }
}
