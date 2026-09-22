use crate::mcp::ToolKind;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
    pub tool_name: Option<String>,
    pub tool_kind: Option<ToolKind>,
    pub parameters: Option<serde_json::Value>,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reason: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expected_observation: Vec<String>,
    pub final_answer: Option<String>,
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

impl Action {
    pub fn compute_idempotency_key(&self) -> String {
        use ring::digest::{digest, SHA256};
        let canonical = serde_json::json!({
            "action_type": self.action_type,
            "tool": self.tool,
            "target": self.target,
            "parameters": self.parameters,
        });
        let bytes = serde_json::to_vec(&canonical).unwrap_or_default();
        digest(&SHA256, &bytes)
            .as_ref()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }
}

/// Stable error categories exposed by observation results.
///
/// These values intentionally use PascalCase because they are part of the
/// persisted audit/event contract and are also consumed by the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObservationError {
    #[serde(rename = "CapabilityNotFound")]
    CapabilityNotFound,
    #[serde(rename = "ConnectionFailed")]
    ConnectionFailed,
    #[serde(rename = "CommandFailed")]
    CommandFailed,
    #[serde(rename = "ParseFailed")]
    ParseFailed,
    #[serde(rename = "ValidationFailed")]
    ValidationFailed,
    #[serde(rename = "PersistenceFailed")]
    PersistenceFailed,
}

impl ObservationError {
    /// Classify a failed tool result at the observation boundary.
    ///
    /// Tool adapters currently return human-readable strings, so classification
    /// lives here until every adapter has a typed error source of its own.
    pub fn classify(tool_name: Option<&str>, output: &str) -> Self {
        let tool = tool_name.unwrap_or_default().to_ascii_lowercase();
        let message = output.to_ascii_lowercase();

        if message.contains("unknown tool")
            || message.contains("tool not found")
            || message.contains("alias target tool not found")
            || message.contains("no command template")
            || message.contains("no command defined")
            || message.contains("unsupported")
        {
            return Self::CapabilityNotFound;
        }

        if message.contains("save")
            || message.contains("persist")
            || message.contains("write temporary")
            || message.contains("atomically rename")
            || message.contains("artifact")
        {
            return Self::PersistenceFailed;
        }

        if message.contains("parse")
            || message.contains("parsing")
            || message.contains("deserialize")
            || message.contains("canonicalization")
        {
            return Self::ParseFailed;
        }

        if message.contains("validation")
            || message.contains("invalid")
            || message.contains("schema")
            || message.contains("policy")
            || message.contains("dry-run")
        {
            return Self::ValidationFailed;
        }

        if message.contains("connection")
            || message.contains("connect")
            || message.contains("timed out")
            || message.contains("timeout")
            || message.contains("unreachable")
            || message.contains("resolve host")
            || message.contains("ssh")
            || message.contains("icmp")
            || tool.contains("ping")
            || tool.contains("traceroute")
            || tool.contains("connection")
        {
            return Self::ConnectionFailed;
        }

        Self::CommandFailed
    }
}

/// Result returned after executing an action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    pub id: Uuid,
    pub action_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub observation: Observation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ObservationError>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempt_count: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::ObservationError;

    #[test]
    fn serializes_error_codes_with_the_public_names() {
        let values = [
            ObservationError::CapabilityNotFound,
            ObservationError::ConnectionFailed,
            ObservationError::CommandFailed,
            ObservationError::ParseFailed,
            ObservationError::ValidationFailed,
            ObservationError::PersistenceFailed,
        ];

        let serialized = values
            .iter()
            .map(|value| serde_json::to_string(value).unwrap())
            .collect::<Vec<_>>();

        assert_eq!(
            serialized,
            vec![
                "\"CapabilityNotFound\"",
                "\"ConnectionFailed\"",
                "\"CommandFailed\"",
                "\"ParseFailed\"",
                "\"ValidationFailed\"",
                "\"PersistenceFailed\"",
            ]
        );
    }

    #[test]
    fn classifies_common_observation_failures() {
        assert_eq!(
            ObservationError::classify(Some("network_show"), "Connection timed out"),
            ObservationError::ConnectionFailed
        );
        assert_eq!(
            ObservationError::classify(Some("unknown"), "Execution failed: Unknown tool ID"),
            ObservationError::CapabilityNotFound
        );
        assert_eq!(
            ObservationError::classify(Some("fetch_routing"), "canonicalization failed"),
            ObservationError::ParseFailed
        );
        assert_eq!(
            ObservationError::classify(Some("apply_config"), "validation failed"),
            ObservationError::ValidationFailed
        );
        assert_eq!(
            ObservationError::classify(Some("save_yaml"), "Failed to save artifact"),
            ObservationError::PersistenceFailed
        );
        assert_eq!(
            ObservationError::classify(Some("network_show"), "command exited with code 1"),
            ObservationError::CommandFailed
        );
    }
}

/// Unified Event Model for reconstructing NetworkState
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type", rename_all = "snake_case")]
pub enum HarnessEvent {
    TaskStarted {
        task_id: Uuid,
        timestamp: DateTime<Utc>,
    },
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
