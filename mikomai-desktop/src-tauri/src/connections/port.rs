use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::fmt;

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(transparent)]
pub struct Port(u16);

impl Port {
    pub fn new(value: u16) -> Result<Self, String> {
        if value < 1 {
            return Err("Port must be between 1 and 65535".to_string());
        }
        Ok(Self(value))
    }

    #[allow(dead_code)]
    pub fn value(&self) -> u16 {
        self.0
    }
}

impl TryFrom<u16> for Port {
    type Error = String;
    fn try_from(value: u16) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<i32> for Port {
    type Error = String;
    fn try_from(value: i32) -> Result<Self, Self::Error> {
        if value < 1 || value > 65535 {
            return Err(format!("Port {} is out of range (1..=65535)", value));
        }
        Ok(Self(value as u16))
    }
}

impl TryFrom<u32> for Port {
    type Error = String;
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        if value < 1 || value > 65535 {
            return Err(format!("Port {} is out of range (1..=65535)", value));
        }
        Ok(Self(value as u16))
    }
}

impl TryFrom<usize> for Port {
    type Error = String;
    fn try_from(value: usize) -> Result<Self, Self::Error> {
        if value < 1 || value > 65535 {
            return Err(format!("Port {} is out of range (1..=65535)", value));
        }
        Ok(Self(value as u16))
    }
}

impl TryFrom<i64> for Port {
    type Error = String;
    fn try_from(value: i64) -> Result<Self, Self::Error> {
        if value < 1 || value > 65535 {
            return Err(format!("Port {} is out of range (1..=65535)", value));
        }
        Ok(Self(value as u16))
    }
}

impl TryFrom<&str> for Port {
    type Error = String;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let trimmed = value.trim();
        let parsed = trimmed
            .parse::<u16>()
            .map_err(|e| format!("Invalid port '{}': {}", value, e))?;
        Self::new(parsed)
    }
}

impl TryFrom<String> for Port {
    type Error = String;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_from(value.as_str())
    }
}

impl<'de> Deserialize<'de> for Port {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let val = u16::deserialize(deserializer)?;
        Self::try_from(val).map_err(serde::de::Error::custom)
    }
}

impl std::ops::Deref for Port {
    type Target = u16;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<u16> for Port {
    fn as_ref(&self) -> &u16 {
        &self.0
    }
}

impl fmt::Display for Port {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_port() {
        let p = Port::try_from(22).unwrap();
        assert_eq!(p.value(), 22);
        assert_eq!(*p, 22);

        let p_min = Port::try_from(1).unwrap();
        assert_eq!(p_min.value(), 1);

        let p_max = Port::try_from(65535).unwrap();
        assert_eq!(p_max.value(), 65535);

        let p_str = Port::try_from("8080").unwrap();
        assert_eq!(p_str.value(), 8080);
    }

    #[test]
    fn test_invalid_port() {
        assert!(Port::try_from(0).is_err());
        assert!(Port::try_from("0").is_err());
        assert!(Port::try_from("65536").is_err());
        assert!(Port::try_from("abc").is_err());
        assert!(Port::try_from(70000u32).is_err());
        assert!(Port::try_from(-1i64).is_err());
    }

    #[test]
    fn test_port_serialization() {
        let p = Port::try_from(22).unwrap();
        let serialized = serde_json::to_string(&p).unwrap();
        assert_eq!(serialized, "22");

        let deserialized: Port = serde_json::from_str("80").unwrap();
        assert_eq!(deserialized.value(), 80);

        assert!(serde_json::from_str::<Port>("0").is_err());
        assert!(serde_json::from_str::<Port>("70000").is_err());
    }
}
