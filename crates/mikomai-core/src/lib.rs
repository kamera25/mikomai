//! GUI and infrastructure independent application core.
pub mod application;
pub mod desired_change;
pub mod dispatch;
pub mod domain;
pub mod port;
pub use application::{ApplicationError, ApplicationResult, TaskManager};
pub use application::{ChangeService, ChatService, DiagnoseService};
pub use dispatch::{select_dispatch_mode, DispatchMode};
pub use domain::desired::{
    DesiredStatePatch, EntityRef, EntityType, Mutation, PatchError, PropertyChange, StateEntity,
    StateGraph,
};
pub use domain::{
    ActionType, Decision, Evidence, Observation, ObservationSource, OperationClass, OperationGate,
    OperationPlan, OperationStatus, Provenance, ProvenanceOrigin, Task, TaskSnapshot, TaskStatus,
};
pub use port::{
    HistoryRepository, InferencePort, OperationRepository, PlanDecision, PlannerPort, PortFuture,
    ReportEvent, ReporterPort, SearchHit, SearchPort, TaskRepository, ToolExecutorPort, ToolResult,
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use uuid::Uuid;
    #[derive(Default)]
    struct Repo(Mutex<Vec<TaskSnapshot>>);
    impl TaskRepository for Repo {
        fn save(&self, value: &TaskSnapshot) -> ApplicationResult<()> {
            self.0
                .lock()
                .map_err(|_| ApplicationError::storage("repo lock poisoned"))?
                .push(value.clone());
            Ok(())
        }
        fn load(&self, id: Uuid) -> ApplicationResult<Option<TaskSnapshot>> {
            Ok(self
                .0
                .lock()
                .map_err(|_| ApplicationError::storage("repo lock poisoned"))?
                .iter()
                .find(|v| v.task.id == id)
                .cloned())
        }
    }
    #[test]
    fn task_manager_persists_without_gui_or_adapter() {
        let manager = application::TaskManager::new(Repo::default());
        let task = manager.start("diagnose vlan").unwrap();
        assert_eq!(
            manager.resume(task.task.id).unwrap().unwrap().task.goal,
            "diagnose vlan"
        );
    }
    #[test]
    fn operation_gate_requires_exact_plan_hash() {
        let mut plan = OperationPlan::new(
            "network_config",
            Some("sw1".into()),
            serde_json::json!({"vlan": 10}),
            "requested",
        )
        .unwrap();
        plan.status = domain::OperationStatus::Approved;
        assert!(OperationGate::authorize(&plan, Some(&plan.plan_hash)).is_ok());
        assert!(OperationGate::authorize(&plan, Some("other")).is_err());
    }

    #[test]
    fn dispatches_explanations_to_worker_and_live_checks_to_agent() {
        assert_eq!(
            select_dispatch_mode("F220のVLAN設定方法を教えて"),
            DispatchMode::Worker
        );
        assert_eq!(
            select_dispatch_mode("F220の状態を確認して"),
            DispatchMode::Agent
        );
    }
}
