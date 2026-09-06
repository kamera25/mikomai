//! Common traffic control for terminal worker results.
//!
//! Planners decide whether the goal is complete; they do not own the
//! user-facing wording. The coordinator turns structured worker outcomes into
//! one of a small number of safe next steps and gives completed work to the
//! Fast Agent for presentation.

use crate::state::events::ActionType;

#[derive(Debug, Clone, PartialEq, Eq)]
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

/// Stateless coordinator: all transition decisions are deterministic and can
/// be tested without an LLM, MCP server, or UI.
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

    /// Converts the existing Planner terminal declaration to the common
    /// contract. `final_answer` is now a factual completion brief, not the
    /// final user-facing response.
    pub fn after_planner_terminal(
        action_type: ActionType,
        planner_brief: Option<String>,
    ) -> CoordinatorNext {
        match action_type {
            ActionType::Finish => Self::after_worker(WorkerOutcome::Completed {
                completion_brief: planner_brief
                    .filter(|brief| !brief.trim().is_empty())
                    .unwrap_or_else(|| "Requested work completed without a detailed brief.".into()),
            }),
            ActionType::AskHuman => Self::after_worker(WorkerOutcome::AwaitingUserInput {
                message: planner_brief.unwrap_or_else(|| "Additional input is required.".into()),
            }),
            _ => CoordinatorNext::ResumePlanner,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completed_work_is_presented_by_the_fast_agent_not_the_planner() {
        assert_eq!(
            Coordinator::after_planner_terminal(ActionType::Finish, Some("Ping succeeded".into())),
            CoordinatorNext::PresentWithFastAgent {
                completion_brief: "Ping succeeded".into()
            }
        );
    }

    #[test]
    fn user_input_and_approval_never_fall_through_to_a_completion() {
        assert!(matches!(
            Coordinator::after_worker(WorkerOutcome::AwaitingUserInput {
                message: "Select an interface".into()
            }),
            CoordinatorNext::AskUser { .. }
        ));
        assert!(matches!(
            Coordinator::after_worker(WorkerOutcome::AwaitingApproval {
                message: "Confirm DHCP probe".into()
            }),
            CoordinatorNext::AwaitApproval { .. }
        ));
    }

    #[test]
    fn worker_handoff_is_explicit_and_typed() {
        assert_eq!(
            Coordinator::after_worker(WorkerOutcome::Handoff {
                worker: WorkerKind::PacketSafety,
                request: "validated DHCP diagnostic".into(),
            }),
            CoordinatorNext::DispatchWorker {
                worker: WorkerKind::PacketSafety,
                request: "validated DHCP diagnostic".into(),
            }
        );
    }
}
