use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::fmt;

#[derive(Serialize, Clone, Debug, PartialEq, Eq, Hash)]
#[serde(transparent)]
pub struct VendorType(String);

impl VendorType {
    pub fn new(value: String) -> Result<Self, String> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err("VendorType cannot be empty".to_string());
        }
        if trimmed.len() > 100 {
            return Err("VendorType cannot exceed 100 characters".to_string());
        }
        Ok(Self(trimmed.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for VendorType {
    type Error = String;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&str> for VendorType {
    type Error = String;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value.to_string())
    }
}

impl<'de> Deserialize<'de> for VendorType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::try_from(s).map_err(serde::de::Error::custom)
    }
}

impl std::ops::Deref for VendorType {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<str> for VendorType {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for VendorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_vendor_type() {
        let vt = VendorType::try_from("cisco").unwrap();
        assert_eq!(vt.as_str(), "cisco");
    }

    #[test]
    fn test_empty_vendor_type() {
        assert!(VendorType::try_from("").is_err());
    }

    #[test]
    fn test_too_long_vendor_type() {
        let long_vt = "a".repeat(101);
        assert!(VendorType::try_from(long_vt.as_str()).is_err());
    }
}
