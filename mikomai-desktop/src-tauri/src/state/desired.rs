use serde::{Deserialize, Serialize};

/// Target desired state for the network
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DesiredState {
    pub raw_goal: String,
    pub requirements: Vec<RequirementItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequirementItem {
    pub description: String,
    pub satisfied: bool,
}
