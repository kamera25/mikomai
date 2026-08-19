use crate::state::events::{ActionType, Decision};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionDto {
    pub action_type: String,
    pub objective: String,
    #[serde(default)]
    pub tool: Option<String>,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub parameters: serde_json::Value,
    #[serde(default)]
    pub reason: Vec<String>,
    #[serde(default)]
    pub expected_observation: Vec<String>,
    #[serde(default)]
    pub final_answer: Option<String>,
}

pub fn parse_decision_from_json(raw_json: &str) -> Result<Decision, String> {
    let clean = crate::mcp::executor::extract_json_blocks(raw_json);
    let target_str = clean.first().map(|s| s.as_str()).unwrap_or(raw_json);

    let parsed: DecisionDto = serde_json::from_str(target_str)
        .or_else(|_| {
            let repaired = jsonrepair_rs::jsonrepair(target_str).map_err(|e| format!("{:?}", e))?;
            serde_json::from_str(&repaired).map_err(|e| format!("{:?}", e))
        })
        .map_err(|e| format!("Failed to parse Decision JSON: {}", e))?;

    let action_type = match parsed.action_type.to_uppercase().as_str() {
        "OBSERVE" => ActionType::Observe,
        "VERIFY" => ActionType::Verify,
        "CONFIGURE" => ActionType::Configure,
        "ROLLBACK" => ActionType::Rollback,
        "ASK_HUMAN" => ActionType::AskHuman,
        "FINISH" => ActionType::Finish,
        _ => ActionType::Observe,
    };

    Ok(Decision {
        id: uuid::Uuid::new_v4(),
        timestamp: chrono::Utc::now(),
        action_type,
        objective: parsed.objective,
        tool: parsed.tool,
        target: parsed.target,
        parameters: parsed.parameters,
        reason: parsed.reason,
        expected_observation: parsed.expected_observation,
        final_answer: parsed.final_answer,
    })
}

