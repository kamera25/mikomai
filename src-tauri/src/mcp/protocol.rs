use crate::history::SummaryItem;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ChatRequest
{
    pub user_message: String,
    pub summaries: Vec<SummaryItem>,
    pub recent_ips: Vec<String>,
    pub history_limit: usize,
    pub mcp_timeout: u64,
    pub attachments: Option<Vec<crate::history::Attachment>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ToolStartedPayload
{
    pub task_id: uuid::Uuid,
    pub tool_id: String,
    pub tool_label: String,
    pub args: serde_json::Value,
    pub resolved_host: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ToolFinishedPayload
{
    pub task_id: uuid::Uuid,
    pub success: bool,
    pub output: String,
    pub saved_path: Option<String>,
    pub is_cached: Option<bool>,
    pub cache_time: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisStartedPayload
{
    pub task_id: uuid::Uuid,
    pub analysis_task_id: uuid::Uuid,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct InitialStartedPayload
{
    pub task_id: uuid::Uuid,
    #[serde(default)]
    pub has_image: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct InitialFinishedPayload
{
    pub task_id: uuid::Uuid,
    pub content: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SummarySavedPayload
{
    pub task_id: uuid::Uuid,
    pub summary_text: String,
    pub summary: SummaryItem,
    pub content: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type", content = "payload", rename_all = "camelCase")]
pub enum ChatEvent
{
    ArpYamlSaved
    {
        device_name: String,
        saved_path: String,
    },
    RouteYamlSaved
    {
        device_name: String,
        saved_path: String,
    },
    McpToolStarted(ToolStartedPayload),
    McpToolFinished(ToolFinishedPayload),
    McpAnalysisStarted(AnalysisStartedPayload),
    LlmChunk(String),
    AgentSelected(String),
    McpInitialStarted(InitialStartedPayload),
    McpInitialFinished(InitialFinishedPayload),
    McpSummarySaved(SummarySavedPayload),
}
