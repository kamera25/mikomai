use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerKind {
    Builder,
    Rag,
    PacketSafety,
    FastAgent,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkerOutcome {
    Completed { completion_brief: String },
    AwaitingUserInput { message: String },
    AwaitingApproval { message: String },
    Handoff { worker: WorkerKind, request: String },
    Failed { public_message: String },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoordinatorNext {
    PresentWithFastAgent { completion_brief: String },
    AskUser { message: String },
    AwaitApproval { message: String },
    DispatchWorker { worker: WorkerKind, request: String },
    ResumePlanner,
    Fail { public_message: String },
}
pub struct Coordinator;
impl Coordinator {
    pub fn after_worker(outcome: WorkerOutcome) -> CoordinatorNext {
        match outcome {
            WorkerOutcome::Completed { completion_brief } => {
                CoordinatorNext::PresentWithFastAgent { completion_brief }
            }
            WorkerOutcome::AwaitingUserInput { message } => CoordinatorNext::AskUser { message },
            WorkerOutcome::AwaitingApproval { message } => {
                CoordinatorNext::AwaitApproval { message }
            }
            WorkerOutcome::Handoff { worker, request } => {
                CoordinatorNext::DispatchWorker { worker, request }
            }
            WorkerOutcome::Failed { public_message } => CoordinatorNext::Fail { public_message },
        }
    }
}
