use super::Evidence;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Task {
    pub id: Uuid,
    pub goal: String,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    Running,
    AwaitingApproval,
    AwaitingInput,
    Completed,
    Failed,
    Unknown,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskSnapshot {
    pub task: Task,
    pub status: TaskStatus,
    pub evidence: Vec<Evidence>,
}
impl TaskSnapshot {
    pub fn new(goal: impl Into<String>) -> Self {
        Self {
            task: Task {
                id: Uuid::new_v4(),
                goal: goal.into(),
            },
            status: TaskStatus::Pending,
            evidence: Vec::new(),
        }
    }
    pub fn with_status(mut self, status: TaskStatus) -> Self {
        self.status = status;
        self
    }
}
