use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::fmt;

#[derive(Serialize, Clone, Debug, PartialEq, Eq, Hash)]
#[serde(transparent)]
pub struct Hostname(String);

impl Hostname
{
    pub fn new(value: String) -> Result<Self, String>
    {
        let trimmed = value.trim();
        if trimmed.is_empty()
        {
            return Err("Hostname cannot be empty".to_string());
        }
        if trimmed.len() > 255
        {
            return Err("Hostname cannot exceed 255 characters".to_string());
        }
        for (i, c) in trimmed.chars().enumerate()
        {
            if !c.is_alphanumeric() && c != '-' && c != '.' && c != '_'
            {
                return Err(format!("Hostname contains invalid character: '{}'", c));
            }
            if (i == 0 || i == trimmed.len() - 1) && (c == '-' || c == '.' || c == '_')
            {
                return Err(
                    "Hostname cannot start or end with dash, dot, or underscore".to_string()
                );
            }
        }
        Ok(Self(trimmed.to_string()))
    }

    pub fn as_str(&self) -> &str
    {
        &self.0
    }
}

impl TryFrom<String> for Hostname
{
    type Error = String;
    fn try_from(value: String) -> Result<Self, Self::Error>
    {
        Self::new(value)
    }
}

impl TryFrom<&str> for Hostname
{
    type Error = String;
    fn try_from(value: &str) -> Result<Self, Self::Error>
    {
        Self::new(value.to_string())
    }
}

impl<'de> Deserialize<'de> for Hostname
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::try_from(s).map_err(serde::de::Error::custom)
    }
}

impl std::ops::Deref for Hostname
{
    type Target = str;
    fn deref(&self) -> &Self::Target
    {
        &self.0
    }
}

impl AsRef<str> for Hostname
{
    fn as_ref(&self) -> &str
    {
        &self.0
    }
}

impl fmt::Display for Hostname
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
    fn test_valid_hostname()
    {
        let h = Hostname::try_from("router-1.tokyo.local").unwrap();
        assert_eq!(h.as_str(), "router-1.tokyo.local");

        let single_char = Hostname::try_from("a").unwrap();
        assert_eq!(single_char.as_str(), "a");

        let with_underscore = Hostname::try_from("switch_2").unwrap();
        assert_eq!(with_underscore.as_str(), "switch_2");
    }

    #[test]
    fn test_invalid_hostname_chars()
    {
        assert!(Hostname::try_from("router$1").is_err());
        assert!(Hostname::try_from("router@home").is_err());
    }

    #[test]
    fn test_invalid_start_end()
    {
        assert!(Hostname::try_from("-router").is_err());
        assert!(Hostname::try_from("router.").is_err());
        assert!(Hostname::try_from("_switch").is_err());
        assert!(Hostname::try_from("switch_").is_err());
    }

    #[test]
    fn test_empty_and_too_long()
    {
        assert!(Hostname::try_from("").is_err());
        let long_name = "a".repeat(256);
        assert!(Hostname::try_from(long_name.as_str()).is_err());
    }
}
