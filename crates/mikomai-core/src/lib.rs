//! GUI and infrastructure independent application core.
use serde::{Deserialize, Serialize};
use uuid::Uuid;
pub mod application;
pub mod domain;
pub mod port;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Task { pub id: Uuid, pub goal: String }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Evidence { pub source: String, pub content: String }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskStatus { Pending, Running, AwaitingApproval, Completed, Failed, Unknown }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OperationPlan { pub id: Uuid, pub tool: String, pub target: String, pub arguments: serde_json::Value, pub plan_hash: String }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskSnapshot { pub task: Task, pub status: TaskStatus, pub evidence: Vec<Evidence> }

/// Ports used by application services. Implementations belong to adapters.
pub trait Planner: Send + Sync { fn plan(&self, task: &TaskSnapshot) -> Result<serde_json::Value, String>; }
pub trait ToolExecutor: Send + Sync { fn execute(&self, plan: &OperationPlan) -> Result<String, String>; }
pub trait TaskRepository: Send + Sync { fn save(&self, snapshot: &TaskSnapshot) -> Result<(), String>; fn load(&self, id: Uuid) -> Result<Option<TaskSnapshot>, String>; }

pub struct ApplicationService<R> { pub repository: R }
impl<R: TaskRepository> ApplicationService<R> {
    pub fn start(&self, goal: impl Into<String>) -> Result<TaskSnapshot, String> {
        let snapshot = TaskSnapshot { task: Task { id: Uuid::new_v4(), goal: goal.into() }, status: TaskStatus::Pending, evidence: Vec::new() };
        self.repository.save(&snapshot)?;
        Ok(snapshot)
    }
    pub fn resume(&self, id: Uuid) -> Result<Option<TaskSnapshot>, String> { self.repository.load(id) }
}

/// Pure policy gate for side effects. Transport adapters must call this before
/// sending a change to a device.
pub struct OperationGate;
impl OperationGate {
    pub fn authorize(plan: &OperationPlan, approved_hash: Option<&str>) -> Result<(), String> {
        if plan.tool.trim().is_empty() || plan.target.trim().is_empty() { return Err("operation plan is incomplete".into()); }
        match approved_hash { Some(hash) if hash == plan.plan_hash => Ok(()), _ => Err("operation plan is not approved".into()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    struct Repo(Mutex<Vec<TaskSnapshot>>);
    impl TaskRepository for Repo {
        fn save(&self, value: &TaskSnapshot) -> Result<(), String> { self.0.lock().unwrap().push(value.clone()); Ok(()) }
        fn load(&self, id: Uuid) -> Result<Option<TaskSnapshot>, String> { Ok(self.0.lock().unwrap().iter().find(|v| v.task.id == id).cloned()) }
    }
    #[test]
    fn application_service_persists_without_gui_or_adapter() {
        let service = ApplicationService { repository: Repo(Mutex::new(Vec::new())) };
        let task = service.start("diagnose vlan").unwrap();
        assert_eq!(service.resume(task.task.id).unwrap().unwrap().task.goal, "diagnose vlan");
    }
    #[test]
    fn operation_gate_requires_exact_plan_hash() {
        let plan = OperationPlan { id: Uuid::new_v4(), tool: "set_vlan".into(), target: "sw1".into(), arguments: serde_json::json!({"vlan": 10}), plan_hash: "h".into() };
        assert!(OperationGate::authorize(&plan, Some("h")).is_ok());
        assert!(OperationGate::authorize(&plan, Some("other")).is_err());
    }
}
