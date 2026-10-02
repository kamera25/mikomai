use serde::{Deserialize, Serialize};
use uuid::Uuid;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProvenanceOrigin {
    Tool,
    Parser,
    Inference,
    Human,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Provenance {
    pub origin: ProvenanceOrigin,
    pub confidence: Option<f64>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObservationSource {
    pub target: Option<String>,
    pub tool: Option<String>,
    pub request: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub success: Option<bool>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Evidence {
    pub id: Uuid,
    pub content: String,
    pub source: ObservationSource,
    pub provenance: Provenance,
}
impl Evidence {
    pub fn from_tool(
        content: impl Into<String>,
        target: Option<String>,
        tool: Option<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            content: content.into(),
            source: ObservationSource {
                target,
                tool,
                request: None,
                success: Some(true),
            },
            provenance: Provenance {
                origin: ProvenanceOrigin::Tool,
                confidence: None,
            },
        }
    }
}
