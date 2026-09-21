//! Small, deterministic adapters used by the headless CLI and integration tests.
use mikomai_core::port::{
    HistoryRepository, PlanDecision, PlannerPort, PortFuture, ReportEvent, ReporterPort,
    ToolExecutorPort, ToolResult,
};
use mikomai_core::{ApplicationError, TaskRepository, TaskSnapshot};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Default)]
pub struct EchoPlanner;
impl PlannerPort for EchoPlanner {
    fn plan<'a>(&'a self, task: &'a TaskSnapshot) -> PortFuture<'a, PlanDecision> {
        Box::pin(async move {
            Ok(PlanDecision::Complete {
                brief: format!("質問を受け付けました: {}", task.task.goal),
            })
        })
    }
}

#[derive(Default)]
pub struct EchoToolExecutor;
impl ToolExecutorPort for EchoToolExecutor {
    fn execute<'a>(
        &'a self,
        _task_id: Uuid,
        tool: &'a str,
        target: Option<&'a str>,
        args: &'a serde_json::Value,
    ) -> PortFuture<'a, ToolResult> {
        Box::pin(async move {
            Ok(ToolResult {
                success: true,
                output: format!("{tool} {} {args}", target.unwrap_or("")),
            })
        })
    }
}

#[derive(Default, Clone)]
pub struct StdoutReporter {
    pub events: Arc<Mutex<Vec<ReportEvent>>>,
}
impl ReporterPort for StdoutReporter {
    fn report(&self, event: ReportEvent) {
        self.events.lock().ok().map(|mut events| events.push(event));
    }
}

#[derive(Default)]
pub struct JsonTaskRepository(pub Mutex<std::collections::HashMap<Uuid, TaskSnapshot>>);
impl TaskRepository for JsonTaskRepository {
    fn save(&self, snapshot: &TaskSnapshot) -> Result<(), ApplicationError> {
        self.0
            .lock()
            .map_err(|e| ApplicationError::storage(e.to_string()))?
            .insert(snapshot.task.id, snapshot.clone());
        Ok(())
    }
    fn load(&self, id: Uuid) -> Result<Option<TaskSnapshot>, ApplicationError> {
        Ok(self
            .0
            .lock()
            .map_err(|e| ApplicationError::storage(e.to_string()))?
            .get(&id)
            .cloned())
    }
}

#[derive(Default, Clone)]
pub struct RecordingHistory {
    pub entries: Arc<Mutex<Vec<(Uuid, mikomai_core::Evidence)>>>,
}
impl HistoryRepository for RecordingHistory {
    fn append(
        &self,
        task_id: Uuid,
        evidence: &mikomai_core::Evidence,
    ) -> Result<(), ApplicationError> {
        self.entries
            .lock()
            .map_err(|error| ApplicationError::storage(error.to_string()))?
            .push((task_id, evidence.clone()));
        Ok(())
    }
}
