use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::fmt;

#[derive(Serialize, Clone, Debug, PartialEq, Eq, Hash)]
#[serde(transparent)]
pub struct Username(String);

impl Username
{
    pub fn new(value: String) -> Result<Self, String>
    {
        let trimmed = value.trim();
        if trimmed.is_empty()
        {
            return Err("Username cannot be empty".to_string());
        }
        if trimmed.len() > 100
        {
            return Err("Username cannot exceed 100 characters".to_string());
        }
        Ok(Self(trimmed.to_string()))
    }

    pub fn as_str(&self) -> &str
    {
        &self.0
    }
}

impl TryFrom<String> for Username
{
    type Error = String;
    fn try_from(value: String) -> Result<Self, Self::Error>
    {
        Self::new(value)
    }
}

impl TryFrom<&str> for Username
{
    type Error = String;
    fn try_from(value: &str) -> Result<Self, Self::Error>
    {
        Self::new(value.to_string())
    }
}

impl<'de> Deserialize<'de> for Username
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::try_from(s).map_err(serde::de::Error::custom)
    }
}

impl std::ops::Deref for Username
{
    type Target = str;
    fn deref(&self) -> &Self::Target
    {
        &self.0
    }
}

impl AsRef<str> for Username
{
    fn as_ref(&self) -> &str
    {
        &self.0
    }
}

impl fmt::Display for Username
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
    fn test_valid_username()
    {
        let u = Username::try_from("admin").unwrap();
        assert_eq!(u.as_str(), "admin");
    }

    #[test]
    fn test_empty_username()
    {
        assert!(Username::try_from("").is_err());
    }

    #[test]
    fn test_too_long_username()
    {
        let long_u = "a".repeat(101);
        assert!(Username::try_from(long_u.as_str()).is_err());
    }
}
