use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Facts observed from network devices
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ObservedState {
    /// Ordered list of all acquired observations
    pub observations: Vec<crate::state::events::Observation>,
    /// Device-level facts, e.g. hostname -> interface/route maps
    pub devices: HashMap<String, DeviceFact>,
    /// Summary fact statements with timestamps
    pub facts: Vec<FactItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceFact {
    pub name: String,
    pub vendor: Option<String>,
    pub status: Option<String>,
    pub interfaces: HashMap<String, serde_json::Value>,
    pub routes: HashMap<String, serde_json::Value>,
    pub raw_snapshots: HashMap<String, String>,
}

impl DeviceFact {
    pub fn new(name: String) -> Self {
        Self {
            name,
            vendor: None,
            status: None,
            interfaces: HashMap::new(),
            routes: HashMap::new(),
            raw_snapshots: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactItem {
    pub key: String,
    pub value: String,
    pub source_device: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}
