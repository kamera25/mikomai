use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::mcp::ToolKind;

/// Action space supported by the harness
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ActionType {
    Observe,
    Verify,
    Configure,
    Rollback,
    AskHuman,
    Finish,
}

impl ActionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Observe => "OBSERVE",
            Self::Verify => "VERIFY",
            Self::Configure => "CONFIGURE",
            Self::Rollback => "ROLLBACK",
            Self::AskHuman => "ASK_HUMAN",
            Self::Finish => "FINISH",
        }
    }
}

/// Source of provenance for an observation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProvenanceOrigin {
    Tool,
    Parser,
    Llm,
    Human,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provenance {
    pub origin: ProvenanceOrigin,
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationSource {
    pub device: Option<String>,
    pub command: Option<String>,
    pub tool_kind: Option<ToolKind>,
}

/// Fact acquired by the harness from the network or environment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub raw: String,
    pub parsed: Option<serde_json::Value>,
    pub source: ObservationSource,
    pub provenance: Provenance,
}

/// Structured decision proposed by the Planner
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub action_type: ActionType,
    pub objective: String,
    pub tool: Option<String>,
    pub target: Option<String>,
    pub parameters: serde_json::Value,
    pub reason: Vec<String>,
    pub expected_observation: Vec<String>,
}

/// Executable action validated and prepared by the harness
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    pub id: Uuid,
    pub decision_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub action_type: ActionType,
    pub tool: Option<String>,
    pub target: Option<String>,
    pub parameters: serde_json::Value,
}

/// Result returned after executing an action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    pub id: Uuid,
    pub action_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub success: bool,
    pub observation: Observation,
}

/// Unified Event Model for reconstructing NetworkState
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type", rename_all = "snake_case")]
pub enum HarnessEvent {
    GoalSet {
        goal: String,
        timestamp: DateTime<Utc>,
    },
    Observation(Observation),
    Decision(Decision),
    Action(Action),
    Result(ActionResult),
    StateUpdated {
        summary: String,
        timestamp: DateTime<Utc>,
    },
    Finished {
        reason: String,
        timestamp: DateTime<Utc>,
    },
}
