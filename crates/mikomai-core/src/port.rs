//! Outbound ports. Implementations belong to adapters and application entry points.
use crate::{Evidence, OperationPlan, TaskSnapshot};
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;
use uuid::Uuid;
pub type PortFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, String>> + Send + 'a>>;
pub trait PlannerPort: Send + Sync {
    fn plan<'a>(&'a self, task: &'a TaskSnapshot) -> PortFuture<'a, PlanDecision>;
}
pub trait ToolExecutorPort: Send + Sync {
    fn execute<'a>(
        &'a self,
        task_id: Uuid,
        tool: &'a str,
        target: Option<&'a str>,
        args: &'a serde_json::Value,
    ) -> PortFuture<'a, ToolResult>;
}
pub trait ReporterPort: Send + Sync {
    fn report(&self, event: ReportEvent);
}
pub trait InferencePort: Send + Sync {
    fn complete<'a>(&'a self, prompt: &'a str) -> PortFuture<'a, String>;
    fn cancel(&self) {}
}
pub trait SearchPort: Send + Sync {
    fn search<'a>(&'a self, query: &'a str, limit: usize) -> PortFuture<'a, Vec<SearchHit>>;
}
pub trait TaskRepository: Send + Sync {
    fn save(&self, snapshot: &TaskSnapshot) -> Result<(), crate::ApplicationError>;
    fn load(&self, id: Uuid) -> Result<Option<TaskSnapshot>, crate::ApplicationError>;
}
pub trait OperationRepository: Send + Sync {
    fn prepare(&self) -> Result<(), crate::ApplicationError> {
        Ok(())
    }
    fn save(&self, plan: &OperationPlan) -> Result<(), crate::ApplicationError>;
    fn load(&self, id: Uuid) -> Result<Option<OperationPlan>, crate::ApplicationError>;
}
pub trait HistoryRepository: Send + Sync {
    /// Preflight the durable write so a change is not executed when its audit
    /// record cannot be persisted.
    fn prepare(&self, _task_id: Uuid) -> Result<(), crate::ApplicationError> {
        Ok(())
    }
    fn append(&self, task_id: Uuid, evidence: &Evidence) -> Result<(), crate::ApplicationError>;
}
#[derive(Debug, Clone)]
pub enum PlanDecision {
    Complete {
        brief: String,
    },
    Observe {
        tool: String,
        target: Option<String>,
        args: serde_json::Value,
    },
    AskUser {
        message: String,
    },
    AwaitApproval {
        plan: OperationPlan,
        message: String,
    },
    Fail {
        message: String,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub success: bool,
    pub output: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub title: String,
    pub content: String,
    pub score: Option<f32>,
}
#[derive(Debug, Clone)]
pub enum ReportEvent {
    TaskStarted { task_id: Uuid },
    Evidence { task_id: Uuid, evidence: Evidence },
    Status { task_id: Uuid, status: String },
    Completed { task_id: Uuid, answer: String },
}
