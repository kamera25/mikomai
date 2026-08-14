use serde_json::Value;
use crate::llm::worker::{DeviceContext, Route};

#[derive(Debug, Clone, PartialEq)]
pub enum RouteAction {
    /// Direct execution of an MCP tool
    DirectToolCall {
        tool_name: String,
        params: Value,
        message: String,
    },
    /// Static reply without calling LLM worker
    StaticReply {
        message: String,
    },
    /// Delegate to LLM worker
    WorkerRoute {
        route: Route,
        subsequent_route: Option<Route>,
        subsequent_task: Option<String>,
    },
    /// Low confidence fallback, ask user for clarification
    AskClarification,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutingSource {
    Shortcut,
    LlmRouter,
    Fallback,
}

#[derive(Debug, Clone)]
pub struct RoutingDecision {
    pub action: RouteAction,
    pub confidence: f64,
    pub device_contexts: Vec<DeviceContext>,
    pub source: RoutingSource,
}
