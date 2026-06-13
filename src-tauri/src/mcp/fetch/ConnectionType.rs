use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionType {
    SSH,
    Console,
    Telnet,
}

impl ConnectionType {
    pub fn from_str(s: &str) -> Option<Self> {
        let s_lower = s.to_lowercase();
        if s_lower.contains("console") || s_lower.contains("serial") {
            Some(ConnectionType::Console)
        } else if s_lower.contains("telnet") {
            Some(ConnectionType::Telnet)
        } else if s_lower.contains("ssh") {
            Some(ConnectionType::SSH)
        } else {
            None
        }
    }
}
