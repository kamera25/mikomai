//! Pure transformations at the boundary between an agent decision and a tool.
//!
//! Tauri, MCP, and LLM calls stay in `agent_loop`; this module makes the
//! safety-critical request shaping and provenance records independently testable.

use crate::mcp::ToolKind;
use crate::state::events::{Action, Observation, ObservationSource, Provenance, ProvenanceOrigin};

pub fn prepare_tool_arguments(
    action: &Action,
    initial_objective: Option<&str>,
) -> serde_json::Value {
    let mut arguments = action.parameters.clone();
    let serde_json::Value::Object(ref mut values) = arguments else {
        return arguments;
    };

    if let Some(target) = &action.target {
        for key in ["target", "device_name", "host", "device"] {
            values
                .entry(key.to_string())
                .or_insert_with(|| serde_json::Value::String(target.clone()));
        }
    }

    if let Some(objective) = initial_objective {
        values.insert(
            "objective".to_string(),
            serde_json::Value::String(objective.to_string()),
        );
    }
    arguments
}

pub fn tool_observation(
    action: &Action,
    tool_name: String,
    tool_kind: Option<ToolKind>,
    executed_parameters: serde_json::Value,
    raw: String,
) -> Observation {
    Observation {
        id: uuid::Uuid::new_v4(),
        timestamp: chrono::Utc::now(),
        raw,
        parsed: None,
        source: ObservationSource {
            device: action.target.clone(),
            command: executed_parameters
                .get("command")
                .and_then(serde_json::Value::as_str)
                .map(ToString::to_string),
            tool_name: Some(tool_name),
            tool_kind,
            parameters: Some(executed_parameters),
        },
        provenance: Provenance {
            origin: ProvenanceOrigin::Tool,
            confidence: Some(1.0),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::events::ActionType;

    fn action(parameters: serde_json::Value) -> Action {
        Action {
            id: uuid::Uuid::new_v4(),
            decision_id: uuid::Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            action_type: ActionType::Observe,
            tool: Some("network_show".to_string()),
            target: Some("R1".to_string()),
            parameters,
        }
    }

    #[test]
    fn prepares_target_aliases_without_overwriting_explicit_values() {
        let prepared = prepare_tool_arguments(
            &action(serde_json::json!({"command": "show version", "host": "10.0.0.1"})),
            Some("R1 の状態を確認する"),
        );
        assert_eq!(prepared["host"], "10.0.0.1");
        assert_eq!(prepared["device"], "R1");
        assert_eq!(prepared["objective"], "R1 の状態を確認する");
    }

    #[test]
    fn observation_keeps_the_arguments_that_were_actually_sent() {
        let action = action(serde_json::json!({"command": "show version"}));
        let parameters = prepare_tool_arguments(&action, Some("調査"));
        let observation = tool_observation(
            &action,
            "network_show".to_string(),
            None,
            parameters,
            "Cisco IOS".to_string(),
        );
        assert_eq!(observation.source.command.as_deref(), Some("show version"));
        assert_eq!(observation.source.parameters.unwrap()["objective"], "調査");
    }
}
