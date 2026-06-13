use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::fmt;

#[derive(Serialize, Clone, Debug, PartialEq, Eq, Hash)]
#[serde(transparent)]
pub struct Password(String);

impl Password {
    pub fn new(value: String) -> Result<Self, String> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err("Password cannot be empty".to_string());
        }
        if trimmed.len() > 128 {
            return Err("Password cannot exceed 128 characters".to_string());
        }
        Ok(Self(trimmed.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Password {
    type Error = String;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&str> for Password {
    type Error = String;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value.to_string())
    }
}

impl<'de> Deserialize<'de> for Password {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::try_from(s).map_err(serde::de::Error::custom)
    }
}

impl std::ops::Deref for Password {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<str> for Password {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Password {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_password() {
        let p = Password::try_from("supersecret").unwrap();
        assert_eq!(p.as_str(), "supersecret");
    }

    #[test]
    fn test_empty_password() {
        assert!(Password::try_from("").is_err());
    }

    #[test]
    fn test_too_long_password() {
        let long_p = "a".repeat(129);
        assert!(Password::try_from(long_p.as_str()).is_err());
    }
}
