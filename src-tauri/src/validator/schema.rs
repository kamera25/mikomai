use crate::state::events::{Action, ActionType, Decision};

pub struct SchemaValidator;

impl SchemaValidator {
    pub fn validate_decision(decision: &Decision) -> Result<Action, String> {
        if decision.objective.trim().is_empty() {
            return Err("Decision objective cannot be empty".to_string());
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
