use crate::state::events::{Action, ActionType};

pub struct PolicyValidator;

impl PolicyValidator {
    pub fn validate_action(action: &Action) -> Result<(), String> {
        if let Some(tool) = &action.tool {
            crate::operations::allow_unattended_execution(tool)?;
        }

        if matches!(
            action.action_type,
            ActionType::Configure | ActionType::Rollback
        ) {
            return Err("Change actions require an approved operation plan; direct agent execution is disabled.".to_string());
        }

        if let Some(ref tool) = action.tool {
            if tool == "network_config" || action.action_type == ActionType::Configure {
                // Check for high-risk commands if command parameter is present
                if let Some(cmd_val) = action
                    .parameters
                    .get("command")
                    .or_else(|| action.parameters.get("commands"))
                {
                    let cmd_str = cmd_val.to_string().to_lowercase();
                    if cmd_str.contains("reload")
                        || cmd_str.contains("erase")
                        || cmd_str.contains("format")
                    {
                        return Err(format!(
                            "Blocked high-risk configuration command: {}",
                            cmd_str
                        ));
                    }
                }
            }
        }

        Ok(())
    }
}
