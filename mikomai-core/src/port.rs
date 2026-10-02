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
/// Capabilities exposed by the integration, not everything the model might do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InferenceCapabilities {
    pub text_generation: bool,
    pub structured_output: bool,
    pub tool_calling: bool,
    pub token_limits: TokenLimits,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TokenLimits {
    /// Total prompt and generated response budget. None means unknown.
    pub context_window: Option<u32>,
    /// Configured generation cap, if the transport enforces one.
    pub max_output_tokens: Option<u32>,
}

impl Default for InferenceCapabilities {
    fn default() -> Self {
        Self {
            text_generation: true,
            structured_output: false,
            tool_calling: false,
            token_limits: TokenLimits::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelAvailability {
    Available,
    Unavailable {
        reason: String,
    },
    /// Legacy or callback integrations may not have a preflight check.
    Unknown,
}

pub trait InferencePort: Send + Sync {
    fn complete<'a>(&'a self, prompt: &'a str) -> PortFuture<'a, String>;
    fn capabilities(&self) -> InferenceCapabilities {
        InferenceCapabilities::default()
    }
    /// Runtime status, independent of whether the backend was compiled.
    fn availability(&self) -> ModelAvailability {
        ModelAvailability::Unknown
    }
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
    TaskStarted {
        task_id: Uuid,
    },
    Evidence {
        task_id: Uuid,
        evidence: Evidence,
    },
    ApprovalRequired {
        task_id: Uuid,
        plan: OperationPlan,
        message: String,
    },
    Status {
        task_id: Uuid,
        status: String,
    },
    Completed {
        task_id: Uuid,
        answer: String,
    },
}

/// Creates a reviewable operation proposal; this port never authorizes execution.
pub trait OperationProposalPort: Send + Sync {
    fn propose<'a>(
        &'a self,
        target: &'a str,
        tool: &'a str,
        args: &'a serde_json::Value,
        rationale: &'a str,
    ) -> PortFuture<'a, OperationPlan>;
}

pub trait StreamingInferencePort: Send + Sync {
    fn complete_streaming(
        &self,
        prompt: &str,
        on_chunk: &mut dyn FnMut(&str, bool),
    ) -> Result<String, String>;
}

/// Receives actual image bytes, never a fabricated text substitute for vision.
pub trait VisionPort: Send + Sync {
    fn analyze<'a>(&'a self, request: &'a crate::vision::VisionRequest) -> PortFuture<'a, String>;
}
