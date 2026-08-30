use crate::state::events::{Action, ActionType, Decision};

pub struct SchemaValidator;

impl SchemaValidator {
    pub fn validate_decision(decision: &Decision) -> Result<Action, String> {
        if decision.objective.trim().is_empty() {
            return Err("Decision objective cannot be empty".to_string());
        }

        if !decision.parameters.is_null() && !decision.parameters.is_object() {
            return Err("Decision parameters must be a JSON object or null".to_string());
        }

        match decision.action_type {
            ActionType::Observe | ActionType::Verify => {
                if decision.tool.is_none() && decision.target.is_none() {
                    return Err(format!(
                        "Action type {:?} requires a tool or target",
                        decision.action_type
                    ));
                }
            }
            ActionType::Configure => {
                if decision.target.is_none() {
                    return Err("Configure action requires target device".to_string());
                }
            }
            ActionType::Rollback => {
                if decision.target.is_none() {
                    return Err("Rollback action requires target device".to_string());
                }
            }
            ActionType::AskHuman => {
                // Requires message or prompt
            }
            ActionType::Finish => {
                // Valid by itself
            }
        }

        Ok(Action {
            id: uuid::Uuid::new_v4(),
            decision_id: decision.id,
            timestamp: chrono::Utc::now(),
            action_type: decision.action_type,
            tool: decision.tool.clone(),
            target: decision.target.clone(),
            parameters: decision.parameters.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::events::{ActionType, Decision};

    fn decision(action_type: ActionType, parameters: serde_json::Value) -> Decision {
        Decision {
            id: uuid::Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            action_type,
            objective: "確認する".to_string(),
            tool: Some("network_show".to_string()),
            target: Some("R1".to_string()),
            parameters,
            reason: vec![],
            expected_observation: vec![],
            final_answer: None,
        }
    }

    #[test]
    fn rejects_non_object_tool_parameters() {
        let error = SchemaValidator::validate_decision(&decision(
            ActionType::Observe,
            serde_json::json!(["show version"]),
        ))
        .unwrap_err();
        assert!(error.contains("JSON object or null"));
    }

    #[test]
    fn accepts_null_parameters_for_terminal_decision() {
        assert!(SchemaValidator::validate_decision(&decision(
            ActionType::Finish,
            serde_json::Value::Null
        ))
        .is_ok());
    }
}
