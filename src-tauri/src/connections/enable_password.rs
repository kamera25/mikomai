use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::fmt;

#[derive(Serialize, Clone, Debug, PartialEq, Eq, Hash)]
#[serde(transparent)]
pub struct EnablePassword(String);

impl EnablePassword
{
    pub fn new(value: String) -> Result<Self, String>
    {
        let trimmed = value.trim();
        if trimmed.is_empty()
        {
            return Err("EnablePassword cannot be empty".to_string());
        }
        if trimmed.len() > 2048
        {
            return Err("EnablePassword cannot exceed 2048 characters".to_string());
        }
        Ok(Self(trimmed.to_string()))
    }

    pub fn as_str(&self) -> &str
    {
        &self.0
    }
}

impl TryFrom<String> for EnablePassword
{
    type Error = String;
    fn try_from(value: String) -> Result<Self, Self::Error>
    {
        Self::new(value)
    }
}

impl TryFrom<&str> for EnablePassword
{
    type Error = String;
    fn try_from(value: &str) -> Result<Self, Self::Error>
    {
        Self::new(value.to_string())
    }
}

impl<'de> Deserialize<'de> for EnablePassword
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::try_from(s).map_err(serde::de::Error::custom)
    }
}

impl std::ops::Deref for EnablePassword
{
    type Target = str;
    fn deref(&self) -> &Self::Target
    {
        &self.0
    }
}

impl AsRef<str> for EnablePassword
{
    fn as_ref(&self) -> &str
    {
        &self.0
    }
}

impl fmt::Display for EnablePassword
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
    fn test_valid_enable_password()
    {
        let ep = EnablePassword::try_from("enablesecret").unwrap();
        assert_eq!(ep.as_str(), "enablesecret");
    }

    #[test]
    fn test_empty_enable_password()
    {
        assert!(EnablePassword::try_from("").is_err());
    }

    #[test]
    fn test_too_long_enable_password()
    {
        let long_ep = "a".repeat(2049);
        assert!(EnablePassword::try_from(long_ep.as_str()).is_err());
    }
}
