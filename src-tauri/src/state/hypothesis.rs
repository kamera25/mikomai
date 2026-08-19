use serde::{Deserialize, Serialize};

/// Hypotheses reasoned by LLM, strictly separated from observed facts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hypothesis {
    pub id: String,
    pub description: String,
    pub confidence: f64,
    pub verified: Option<bool>,
}
