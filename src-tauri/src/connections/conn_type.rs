use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::convert::TryFrom;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConnectionType
{
    SSH,
    Console,
    Telnet,
}

impl ConnectionType
{
    pub fn from_str(s: &str) -> Option<Self>
    {
        let s_lower = s.to_lowercase();
        if s_lower.contains("console") || s_lower.contains("serial")
        {
            Some(ConnectionType::Console)
        }
        else if s_lower.contains("telnet")
        {
            Some(ConnectionType::Telnet)
        }
        else if s_lower.contains("ssh")
        {
            Some(ConnectionType::SSH)
        }
        else
        {
            None
        }
    }

    pub fn as_str(&self) -> &'static str
    {
        match self
        {
            ConnectionType::SSH => "SSH",
            ConnectionType::Console => "Console",
            ConnectionType::Telnet => "Telnet",
        }
    }
}

impl fmt::Display for ConnectionType
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result
    {
        write!(f, "{}", self.as_str())
    }
}

impl Serialize for ConnectionType
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ConnectionType
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::from_str(&s)
            .ok_or_else(|| serde::de::Error::custom(format!("Unknown connection type: {}", s)))
    }
}

impl TryFrom<&str> for ConnectionType
{
    type Error = String;
    fn try_from(value: &str) -> Result<Self, Self::Error>
    {
        Self::from_str(value).ok_or_else(|| format!("Invalid ConnectionType: {}", value))
    }
}

impl TryFrom<String> for ConnectionType
{
    type Error = String;
    fn try_from(value: String) -> Result<Self, Self::Error>
    {
        Self::try_from(value.as_str())
    }
}

#[cfg(test)]
mod tests
{
    use super::*;

    #[test]
    fn test_connection_type_from_str()
    {
        assert_eq!(ConnectionType::from_str("SSH"), Some(ConnectionType::SSH));
        assert_eq!(
            ConnectionType::from_str("Cisco IOS (SSH)"),
            Some(ConnectionType::SSH)
        );
        assert_eq!(
            ConnectionType::from_str("console"),
            Some(ConnectionType::Console)
        );
        assert_eq!(
            ConnectionType::from_str("serial-port"),
            Some(ConnectionType::Console)
        );
        assert_eq!(
            ConnectionType::from_str("telnet"),
            Some(ConnectionType::Telnet)
        );
        assert_eq!(ConnectionType::from_str("unknown"), None);
    }

    #[test]
    fn test_connection_type_serialization()
    {
        let ct = ConnectionType::SSH;
        let serialized = serde_json::to_string(&ct).unwrap();
        assert_eq!(serialized, r#""SSH""#);
    }

    #[test]
    fn test_connection_type_deserialization()
    {
        let deserialized: ConnectionType = serde_json::from_str(r#""Cisco IOS (SSH)""#).unwrap();
        assert_eq!(deserialized, ConnectionType::SSH);

        let deserialized_console: ConnectionType = serde_json::from_str(r#""Console""#).unwrap();
        assert_eq!(deserialized_console, ConnectionType::Console);
    }
}
