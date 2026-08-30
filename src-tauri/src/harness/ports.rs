//! Replaceable boundaries used by the agent orchestrator.
//!
//! LLM inference, MCP execution, and UI reporting live behind these ports so
//! the orchestration policy can be exercised without a device or a window.

use crate::llm::llm::LlamaState;
use crate::mcp::protocol::ChatEvent;
use crate::planner::llm_planner::LlmPlanner;
use crate::state::events::Decision;
use crate::state::network_state::NetworkState;
use tauri::{AppHandle, Emitter, Window};

#[allow(async_fn_in_trait)]
pub trait PlannerPort {
    async fn plan(
        &self,
        app: &AppHandle,
        llama: &LlamaState,
        state: &NetworkState,
    ) -> Result<Decision, String>;
}

#[allow(async_fn_in_trait)]
pub trait ToolExecutorPort {
    async fn execute_tool(
        &self,
        app: AppHandle,
        window: Window,
        task_id: uuid::Uuid,
        tool: String,
        goal: String,
        arguments: serde_json::Value,
    ) -> Result<crate::network::CommandResult, String>;
    async fn execute_builder(
        &self,
        app: AppHandle,
        window: Window,
        goal: String,
        tool: String,
        arguments: serde_json::Value,
    ) -> Result<String, String>;
}

pub enum AgentReport {
    Chat(ChatEvent),
    CommitLog(String),
}

pub trait ReporterPort {
    fn report(&self, report: AgentReport);
}

pub struct LlmPlannerPort;
impl PlannerPort for LlmPlannerPort {
    async fn plan(
        &self,
        app: &AppHandle,
        llama: &LlamaState,
        state: &NetworkState,
    ) -> Result<Decision, String> {
        LlmPlanner::plan(app, llama, state).await
    }
}

pub struct McpToolExecutorPort;
impl ToolExecutorPort for McpToolExecutorPort {
    async fn execute_tool(
        &self,
        app: AppHandle,
        window: Window,
        task_id: uuid::Uuid,
        tool: String,
        goal: String,
        arguments: serde_json::Value,
    ) -> Result<crate::network::CommandResult, String> {
        crate::mcp::executor::flow::execute_mcp_tool_raw(
            app,
            window,
            task_id,
            tool,
            goal,
            arguments,
            vec![],
            120,
        )
        .await
    }
    async fn execute_builder(
        &self,
        app: AppHandle,
        window: Window,
        goal: String,
        tool: String,
        arguments: serde_json::Value,
    ) -> Result<String, String> {
        crate::mcp::executor::flow::execute_mcp_tools_flow(
            app,
            window,
            goal,
            vec![crate::mcp::executor::flow::ToolCall {
                tool,
                args: arguments,
            }],
            vec![],
            vec![],
            0,
            120,
            0,
            true,
        )
        .await
    }
}

pub struct TauriReporterPort {
    window: Window,
}
impl TauriReporterPort {
    pub fn new(window: Window) -> Self {
        Self { window }
    }
}
impl ReporterPort for TauriReporterPort {
    fn report(&self, report: AgentReport) {
        match report {
            AgentReport::Chat(event) => {
                let _ = self.window.emit("chat-event", event);
            }
            AgentReport::CommitLog(line) => {
                let _ = self.window.emit(
                    "commit-log",
                    serde_json::json!({ "line": line, "stream": "stdout" }),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reports_are_typed_at_the_orchestrator_boundary() {
        assert!(
            matches!(AgentReport::CommitLog("started".into()), AgentReport::CommitLog(line) if line == "started")
        );
    }
}
