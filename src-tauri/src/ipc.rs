//! IPC DTOs are deliberately separate from core domain objects.
use serde::{Deserialize, Serialize};
use mikomai_core::TaskSnapshot;

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskDto { pub task_id: String, pub goal: String, pub status: String }
impl From<TaskSnapshot> for TaskDto {
    fn from(value: TaskSnapshot) -> Self { Self { task_id: value.task.id.to_string(), goal: value.task.goal, status: format!("{:?}", value.status).to_lowercase() } }
}
impl TaskDto { pub fn status_is_terminal(&self) -> bool { matches!(self.status.as_str(), "completed" | "failed" | "unknown") } }
