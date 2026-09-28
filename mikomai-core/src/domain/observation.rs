use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ActionType {
    Observe,
    Verify,
    Configure,
    Rollback,
    AskHuman,
    Finish,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Observation {
    pub id: Uuid,
    pub raw: String,
    pub target: Option<String>,
    pub tool: Option<String>,
    pub action: ActionType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Decision {
    pub id: Uuid,
    pub action: ActionType,
    pub objective: String,
    pub tool: Option<String>,
    pub target: Option<String>,
    pub arguments: serde_json::Value,
    pub final_answer: Option<String>,
}

impl Decision {
    pub fn finish(answer: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            action: ActionType::Finish,
            objective: "finish".into(),
            tool: None,
            target: None,
            arguments: serde_json::Value::Null,
            final_answer: Some(answer.into()),
        }
    }
}
