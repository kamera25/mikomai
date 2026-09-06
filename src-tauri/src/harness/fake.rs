//! Test doubles (fake ports) for scenario testing of AgentLoop.
//!
//! Allows full orchestration testing without real LLMs, Tauri runtime, or network devices.

use crate::harness::ports::{AgentReport, PlannerPort, ReporterPort, ToolExecutorPort};
use crate::mcp::protocol::ChatEvent;
use crate::network::CommandResult;
use crate::state::events::Decision;
use crate::state::network_state::NetworkState;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

/// A scriptable planner port for scenario testing.
/// Decisions can be queued sequentially or produced dynamically via a callback.
pub struct FakePlanner {
    decisions: Mutex<VecDeque<Decision>>,
    custom_plan: Option<Box<dyn Fn(&NetworkState) -> Result<Decision, String> + Send + Sync>>,
}

impl FakePlanner {
    pub fn new(decisions: Vec<Decision>) -> Self {
        Self {
            decisions: Mutex::new(VecDeque::from(decisions)),
            custom_plan: None,
        }
    }

    pub fn with_handler<F>(handler: F) -> Self
    where
        F: Fn(&NetworkState) -> Result<Decision, String> + Send + Sync + 'static,
    {
        Self {
            decisions: Mutex::new(VecDeque::new()),
            custom_plan: Some(Box::new(handler)),
        }
    }
}

impl PlannerPort for FakePlanner {
    async fn plan(&self, state: &NetworkState) -> Result<Decision, String> {
        if let Some(ref handler) = self.custom_plan {
            return handler(state);
        }
        let mut queue = self.decisions.lock().unwrap();
        queue
            .pop_front()
            .ok_or_else(|| "FakePlanner ran out of scripted decisions".to_string())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExecutedToolCall {
    pub task_id: uuid::Uuid,
    pub tool: String,
    pub goal: String,
    pub arguments: serde_json::Value,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExecutedBuilderCall {
    pub goal: String,
    pub tool: String,
    pub arguments: serde_json::Value,
}

pub struct FakeToolExecutor {
    tool_calls: Arc<Mutex<Vec<ExecutedToolCall>>>,
    builder_calls: Arc<Mutex<Vec<ExecutedBuilderCall>>>,
    tool_results: Mutex<HashMap<String, Result<CommandResult, String>>>,
    builder_results: Mutex<HashMap<String, Result<String, String>>>,
    default_tool_result: Mutex<Option<Result<CommandResult, String>>>,
    rag_co_worker_result: Mutex<Option<Result<String, String>>>,
}

impl Default for FakeToolExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeToolExecutor {
    pub fn new() -> Self {
        Self {
            tool_calls: Arc::new(Mutex::new(Vec::new())),
            builder_calls: Arc::new(Mutex::new(Vec::new())),
            tool_results: Mutex::new(HashMap::new()),
            builder_results: Mutex::new(HashMap::new()),
            default_tool_result: Mutex::new(None),
            rag_co_worker_result: Mutex::new(None),
        }
    }

    pub fn set_default_tool_result(&self, result: Result<CommandResult, String>) {
        *self.default_tool_result.lock().unwrap() = Some(result);
    }

    pub fn set_tool_result(&self, tool: impl Into<String>, result: Result<CommandResult, String>) {
        self.tool_results
            .lock()
            .unwrap()
            .insert(tool.into(), result);
    }

    pub fn set_builder_result(&self, tool: impl Into<String>, result: Result<String, String>) {
        self.builder_results
            .lock()
            .unwrap()
            .insert(tool.into(), result);
    }

    pub fn set_rag_co_worker_result(&self, result: Result<String, String>) {
        *self.rag_co_worker_result.lock().unwrap() = Some(result);
    }

    pub fn executed_tools(&self) -> Vec<ExecutedToolCall> {
        self.tool_calls.lock().unwrap().clone()
    }

    pub fn executed_builders(&self) -> Vec<ExecutedBuilderCall> {
        self.builder_calls.lock().unwrap().clone()
    }
}

impl ToolExecutorPort for FakeToolExecutor {
    async fn execute_tool(
        &self,
        task_id: uuid::Uuid,
        tool: String,
        goal: String,
        arguments: serde_json::Value,
    ) -> Result<CommandResult, String> {
        self.tool_calls.lock().unwrap().push(ExecutedToolCall {
            task_id,
            tool: tool.clone(),
            goal,
            arguments,
        });

        if let Some(res) = self.tool_results.lock().unwrap().get(&tool) {
            return res.clone();
        }

        if let Some(ref res) = *self.default_tool_result.lock().unwrap() {
            return res.clone();
        }

        Ok(CommandResult {
            success: true,
            output: format!("Fake output for {}", tool),
            saved_path: None,
            is_cached: None,
            cache_time: None,
        })
    }

    async fn execute_builder(
        &self,
        goal: String,
        tool: String,
        arguments: serde_json::Value,
    ) -> Result<String, String> {
        self.builder_calls
            .lock()
            .unwrap()
            .push(ExecutedBuilderCall {
                goal,
                tool: tool.clone(),
                arguments,
            });

        if let Some(res) = self.builder_results.lock().unwrap().get(&tool) {
            return res.clone();
        }

        Ok("Fake builder success".to_string())
    }

    async fn execute_rag_co_worker(
        &self,
        _goal: String,
        raw_output: String,
    ) -> Result<String, String> {
        if let Some(ref res) = *self.rag_co_worker_result.lock().unwrap() {
            return res.clone();
        }
        Ok(raw_output)
    }
}

/// A reporter port that records all emitted reports in memory for testing assertions.
#[derive(Default, Clone)]
pub struct RecordingReporter {
    reports: Arc<Mutex<Vec<AgentReport>>>,
}

impl RecordingReporter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reports(&self) -> Vec<AgentReport> {
        self.reports.lock().unwrap().clone()
    }

    pub fn chat_events(&self) -> Vec<ChatEvent> {
        self.reports
            .lock()
            .unwrap()
            .iter()
            .filter_map(|r| match r {
                AgentReport::Chat(ev) => Some(ev.clone()),
                _ => None,
            })
            .collect()
    }

    pub fn commit_logs(&self) -> Vec<String> {
        self.reports
            .lock()
            .unwrap()
            .iter()
            .filter_map(|r| match r {
                AgentReport::CommitLog(line) => Some(line.clone()),
                _ => None,
            })
            .collect()
    }

    pub fn llm_chunks(&self) -> Vec<String> {
        self.chat_events()
            .into_iter()
            .filter_map(|event| match event {
                ChatEvent::LlmChunk(chunk) => Some(chunk),
                _ => None,
            })
            .collect()
    }
}

impl ReporterPort for RecordingReporter {
    fn report(&self, report: AgentReport) {
        self.reports.lock().unwrap().push(report);
    }
}
