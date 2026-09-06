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
    async fn plan(&self, state: &NetworkState) -> Result<Decision, String>;
}

#[allow(async_fn_in_trait)]
pub trait ToolExecutorPort {
    async fn execute_tool(
        &self,
        task_id: uuid::Uuid,
        tool: String,
        goal: String,
        arguments: serde_json::Value,
    ) -> Result<crate::network::CommandResult, String>;
    async fn execute_builder(
        &self,
        goal: String,
        tool: String,
        arguments: serde_json::Value,
    ) -> Result<String, String>;
    async fn execute_rag_co_worker(
        &self,
        _goal: String,
        raw_output: String,
    ) -> Result<String, String> {
        Ok(raw_output)
    }

    /// Presentation is deliberately separated from planning and tool work.
    /// Test doubles retain the factual brief; the live port invokes Fast Agent.
    async fn present_completion(
        &self,
        _goal: String,
        completion_brief: String,
    ) -> Result<String, String> {
        Ok(completion_brief)
    }
}

#[derive(Clone, Debug)]
pub enum AgentReport {
    Chat(ChatEvent),
    CommitLog(String),
}

pub trait ReporterPort {
    fn report(&self, report: AgentReport);
}

pub struct LlmPlannerPort<'a> {
    app: AppHandle,
    llama: &'a LlamaState,
}

impl<'a> LlmPlannerPort<'a> {
    pub fn new(app: AppHandle, llama: &'a LlamaState) -> Self {
        Self { app, llama }
    }
}

impl<'a> PlannerPort for LlmPlannerPort<'a> {
    async fn plan(&self, state: &NetworkState) -> Result<Decision, String> {
        LlmPlanner::plan(&self.app, self.llama, state).await
    }
}

pub struct McpToolExecutorPort<'a> {
    app: AppHandle,
    window: Window,
    llama: &'a LlamaState,
}

impl<'a> McpToolExecutorPort<'a> {
    pub fn new(app: AppHandle, window: Window, llama: &'a LlamaState) -> Self {
        Self { app, window, llama }
    }
}

impl<'a> ToolExecutorPort for McpToolExecutorPort<'a> {
    async fn execute_tool(
        &self,
        task_id: uuid::Uuid,
        tool: String,
        goal: String,
        arguments: serde_json::Value,
    ) -> Result<crate::network::CommandResult, String> {
        crate::mcp::executor::flow::execute_mcp_tool_raw(
            self.app.clone(),
            self.window.clone(),
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
        goal: String,
        tool: String,
        arguments: serde_json::Value,
    ) -> Result<String, String> {
        crate::mcp::executor::flow::execute_mcp_tools_flow(
            self.app.clone(),
            self.window.clone(),
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
    async fn execute_rag_co_worker(
        &self,
        goal: String,
        raw_output: String,
    ) -> Result<String, String> {
        crate::llm::llm::ask_rag_co_worker(&self.app, goal, raw_output, self.llama)
            .await
            .map_err(|e| e.to_string())
    }

    async fn present_completion(
        &self,
        goal: String,
        completion_brief: String,
    ) -> Result<String, String> {
        crate::llm::fast_agent::present_completion(&self.app, self.llama, &goal, &completion_brief)
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
