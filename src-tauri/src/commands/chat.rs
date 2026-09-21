use mikomai_core::{TaskSnapshot, TaskStatus};
use serde::Serialize;

pub use crate::llm::{analyze_tool_output, ask_llm_background, ask_llm_initial, stop_llm};
pub use crate::mcp::executor::{execute_mcp_tool, handle_mcp_message};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskDto {
    pub task_id: String,
    pub goal: String,
    pub status: String,
}

impl From<TaskSnapshot> for TaskDto {
    fn from(snapshot: TaskSnapshot) -> Self {
        Self {
            task_id: snapshot.task.id.to_string(),
            goal: snapshot.task.goal,
            status: status_name(snapshot.status).into(),
        }
    }
}

pub(crate) fn status_name(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Pending => "pending",
        TaskStatus::Running => "running",
        TaskStatus::AwaitingApproval => "awaiting_approval",
        TaskStatus::AwaitingInput => "awaiting_input",
        TaskStatus::Completed => "completed",
        TaskStatus::Failed => "failed",
        TaskStatus::Unknown => "unknown",
    }
}
