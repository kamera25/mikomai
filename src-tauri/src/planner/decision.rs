use crate::state::events::{ActionType, Decision};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DecisionDto {
    #[serde(default)]
    pub action_type: String,
    #[serde(default)]
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

    let repaired = jsonrepair_rs::jsonrepair(target_str).unwrap_or_else(|_| target_str.to_string());
    let mut val: serde_json::Value = serde_json::from_str(&repaired)
        .or_else(|_| serde_json::from_str(target_str))
        .map_err(|e| format!("Failed to parse Decision JSON: {}", e))?;

    // Top-level unwrapping if LLM wraps output in a "decision" key
    if let Some(inner) = val.get("decision").or_else(|| val.get("Decision")) {
        if inner.is_object() {
            val = inner.clone();
        }
    }

    let parsed: DecisionDto = serde_json::from_value(val)
        .map_err(|e| format!("Failed to deserialize DecisionDto: {}", e))?;

    let mut action_type = match parsed.action_type.to_uppercase().as_str() {
        "OBSERVE" => ActionType::Observe,
        "VERIFY" => ActionType::Verify,
        "CONFIGURE" => ActionType::Configure,
        "ROLLBACK" => ActionType::Rollback,
        "ASK_HUMAN" => ActionType::AskHuman,
        "FINISH" => ActionType::Finish,
        _ => ActionType::Observe,
    };

    if parsed.action_type.is_empty() && parsed.final_answer.is_some() {
        action_type = ActionType::Finish;
    }

    let reason = if action_type == ActionType::Finish {
        Vec::new()
    } else {
        parsed.reason
    };

    Ok(Decision {
        id: uuid::Uuid::new_v4(),
        timestamp: chrono::Utc::now(),
        action_type,
        objective: if parsed.objective.is_empty() {
            "処理完了/状態確認".to_string()
        } else {
            parsed.objective
        },
        tool: parsed.tool,
        target: parsed.target,
        parameters: parsed.parameters,
        reason,
        expected_observation: parsed.expected_observation,
        final_answer: parsed.final_answer,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_flat_decision() {
        let json = r#"{
            "action_type": "FINISH",
            "objective": "完了",
            "final_answer": "ネットワーク調査が完了しました。"
        }"#;
        let decision = parse_decision_from_json(json).unwrap();
        assert_eq!(decision.action_type, ActionType::Finish);
        assert_eq!(
            decision.final_answer,
            Some("ネットワーク調査が完了しました。".to_string())
        );
    }

    #[test]
    fn test_finish_discards_reason_and_does_not_serialize_it() {
        let json = r#"{
            "action_type": "FINISH",
            "objective": "完了",
            "reason": ["内部的な補足"],
            "final_answer": "ネットワーク調査が完了しました。"
        }"#;
        let decision = parse_decision_from_json(json).unwrap();

        assert!(decision.reason.is_empty());
        let serialized = serde_json::to_value(decision).unwrap();
        assert!(serialized.get("reason").is_none());
    }

    #[test]
    fn test_schema_conversion() {
        let schema = r#"{
          "type": "object",
          "properties": {
            "action_type": {
              "type": "string",
              "enum": ["OBSERVE", "VERIFY", "CONFIGURE", "ROLLBACK", "ASK_HUMAN", "FINISH"]
            },
            "objective": { "type": "string" },
            "tool": { "type": ["string", "null"] },
            "target": { "type": ["string", "null"] },
            "parameters": {},
            "reason": {
              "type": "array",
              "items": { "type": "string" }
            },
            "expected_observation": {
              "type": "array",
              "items": { "type": "string" }
            },
            "final_answer": { "type": ["string", "null"] }
          },
          "required": ["action_type", "objective"]
        }"#;

        let res = llama_cpp_2::json_schema_to_grammar(schema);
        assert!(res.is_ok(), "Schema conversion failed: {:?}", res);
    }
}
