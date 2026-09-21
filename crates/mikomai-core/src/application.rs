//! Application services orchestrate domain policy through outbound ports.
use crate::dispatch::{select_dispatch_mode, DispatchMode};
use crate::domain::{
    Evidence, HarnessState, HarnessStateMachine, OperationGate, OperationPlan, OperationStatus,
    TaskSnapshot, TaskStatus,
};
use crate::port::{
    HistoryRepository, OperationRepository, PlanDecision, PlannerPort, ReportEvent, ReporterPort,
    ToolExecutorPort, ToolResult,
};
use crate::TaskRepository;
use std::fmt;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationError {
    pub message: String,
}
impl ApplicationError {
    pub fn storage(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}
impl fmt::Display for ApplicationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for ApplicationError {}
pub type ApplicationResult<T> = Result<T, ApplicationError>;

pub struct TaskManager<R> {
    repository: R,
}
impl<R: TaskRepository> TaskManager<R> {
    pub fn new(repository: R) -> Self {
        Self { repository }
    }
    pub fn start(&self, goal: impl Into<String>) -> ApplicationResult<TaskSnapshot> {
        let snapshot = TaskSnapshot::new(goal);
        self.repository.save(&snapshot)?;
        Ok(snapshot)
    }
    pub fn resume(&self, id: Uuid) -> ApplicationResult<Option<TaskSnapshot>> {
        self.repository.load(id)
    }
    pub fn update(&self, snapshot: &TaskSnapshot) -> ApplicationResult<()> {
        self.repository.save(snapshot)
    }

    /// Runs a chat service while keeping the task snapshot as the durable
    /// source of execution state for every entry point.
    pub async fn run_chat<'a, P, E, Reporter>(
        &self,
        service: &ChatService<'a, P, E, Reporter>,
        mut task: TaskSnapshot,
    ) -> Result<String, String>
    where
        P: PlannerPort,
        E: ToolExecutorPort,
        Reporter: ReporterPort,
    {
        task.status = TaskStatus::Running;
        self.update(&task).map_err(|error| error.to_string())?;
        match service.answer(task.clone()).await {
            Ok(answer) => {
                task.status = if answer.starts_with("### ❓") {
                    TaskStatus::AwaitingInput
                } else if answer.starts_with("### ✅") {
                    TaskStatus::AwaitingApproval
                } else {
                    TaskStatus::Completed
                };
                self.update(&task).map_err(|error| error.to_string())?;
                Ok(answer)
            }
            Err(error) => {
                task.status = TaskStatus::Failed;
                self.update(&task)
                    .map_err(|save_error| save_error.to_string())?;
                Err(error)
            }
        }
    }
}

pub struct ChatService<'a, P, E, R> {
    planner: &'a P,
    executor: &'a E,
    reporter: &'a R,
    max_steps: usize,
}
impl<'a, P: PlannerPort, E: ToolExecutorPort, R: ReporterPort> ChatService<'a, P, E, R> {
    pub fn new(planner: &'a P, executor: &'a E, reporter: &'a R) -> Self {
        Self {
            planner,
            executor,
            reporter,
            max_steps: 8,
        }
    }
    pub fn with_max_steps(mut self, max_steps: usize) -> Self {
        self.max_steps = max_steps;
        self
    }
    pub async fn answer(&self, mut task: TaskSnapshot) -> Result<String, String> {
        let mode = select_dispatch_mode(&task.task.goal);
        self.reporter.report(ReportEvent::Status {
            task_id: task.task.id,
            status: format!("dispatch:{}", dispatch_name(mode)),
        });
        task.status = TaskStatus::Running;
        self.reporter.report(ReportEvent::TaskStarted {
            task_id: task.task.id,
        });
        let mut machine = HarnessStateMachine::new(self.max_steps);
        machine.transition(HarnessState::Observing)?;
        loop {
            machine.transition(HarnessState::Deciding)?;
            match self.planner.plan(&task).await? {
                PlanDecision::Complete { brief } => {
                    machine.transition(HarnessState::Finished)?;
                    task.status = TaskStatus::Completed;
                    self.reporter.report(ReportEvent::Completed {
                        task_id: task.task.id,
                        answer: brief.clone(),
                    });
                    return Ok(brief);
                }
                PlanDecision::AskUser { message } => {
                    machine.transition(HarnessState::AskingHuman)?;
                    task.status = TaskStatus::AwaitingInput;
                    return Ok(format!("### ❓ 確認要求\n{message}"));
                }
                PlanDecision::AwaitApproval { plan, message } => {
                    machine.transition(HarnessState::AskingHuman)?;
                    task.status = TaskStatus::AwaitingApproval;
                    let _ = plan;
                    return Ok(format!("### ✅ 承認待ち\n{message}"));
                }
                PlanDecision::Fail { message } => {
                    machine.transition(HarnessState::Failed).ok();
                    task.status = TaskStatus::Failed;
                    return Err(message);
                }
                PlanDecision::Observe { tool, target, args } => {
                    machine.transition(HarnessState::Validating)?;
                    let result = self
                        .executor
                        .execute(task.task.id, &tool, target.as_deref(), &args)
                        .await?;
                    let evidence = Evidence::from_tool(result.output.clone(), target, Some(tool));
                    task.evidence.push(evidence.clone());
                    self.reporter.report(ReportEvent::Evidence {
                        task_id: task.task.id,
                        evidence,
                    });
                    machine.transition(HarnessState::Acting)?;
                    machine.transition(HarnessState::Evaluating)?;
                }
            }
        }
    }
}

fn dispatch_name(mode: DispatchMode) -> &'static str {
    match mode {
        DispatchMode::Worker => "worker",
        DispatchMode::Agent => "agent",
    }
}

pub struct DiagnoseService<'a, P, E, R> {
    chat: ChatService<'a, P, E, R>,
}
impl<'a, P: PlannerPort, E: ToolExecutorPort, R: ReporterPort> DiagnoseService<'a, P, E, R> {
    pub fn new(planner: &'a P, executor: &'a E, reporter: &'a R) -> Self {
        Self {
            chat: ChatService::new(planner, executor, reporter),
        }
    }
    pub async fn diagnose(&self, task: TaskSnapshot) -> Result<String, String> {
        self.chat.answer(task).await
    }
}

pub struct ChangeService<'a, E> {
    executor: &'a E,
}
impl<'a, E: ToolExecutorPort> ChangeService<'a, E> {
    pub fn new(executor: &'a E) -> Self {
        Self { executor }
    }
    pub fn create_plan(
        tool: impl Into<String>,
        target: Option<String>,
        args: serde_json::Value,
        rationale: impl Into<String>,
    ) -> Result<OperationPlan, String> {
        OperationPlan::new(tool, target, args, rationale)
    }
    pub fn approve_plan(plan: &mut OperationPlan, plan_hash: &str) -> Result<(), String> {
        OperationGate::approve(plan, plan_hash)
    }

    pub fn create_and_store_plan<O: OperationRepository>(
        repository: &O,
        tool: impl Into<String>,
        target: Option<String>,
        args: serde_json::Value,
        rationale: impl Into<String>,
    ) -> Result<OperationPlan, String> {
        repository.prepare().map_err(|error| error.to_string())?;
        let plan = Self::create_plan(tool, target, args, rationale)?;
        repository.save(&plan).map_err(|error| error.to_string())?;
        Ok(plan)
    }

    pub fn approve_and_store_plan<O: OperationRepository>(
        repository: &O,
        plan: &mut OperationPlan,
        plan_hash: &str,
    ) -> Result<(), String> {
        Self::approve_plan(plan, plan_hash)?;
        repository.save(plan).map_err(|error| error.to_string())
    }
    pub async fn execute(
        &self,
        task_id: Uuid,
        plan: &mut OperationPlan,
        approved_hash: &str,
    ) -> Result<ToolResult, String> {
        OperationGate::authorize(plan, Some(approved_hash))?;
        plan.status = OperationStatus::Executing;
        let result = self
            .executor
            .execute(task_id, &plan.tool_id, plan.target.as_deref(), &plan.args)
            .await;
        plan.status = if result.as_ref().map(|value| value.success).unwrap_or(false) {
            OperationStatus::Executed
        } else {
            OperationStatus::Failed
        };
        result
    }

    pub async fn execute_and_record<H: HistoryRepository>(
        &self,
        task_id: Uuid,
        plan: &mut OperationPlan,
        approved_hash: &str,
        history: &H,
    ) -> Result<ToolResult, String> {
        history
            .prepare(task_id)
            .map_err(|error| error.to_string())?;
        let result = self.execute(task_id, plan, approved_hash).await?;
        let evidence = Evidence::from_tool(
            result.output.clone(),
            plan.target.clone(),
            Some(plan.tool_id.clone()),
        );
        history
            .append(task_id, &evidence)
            .map_err(|error| error.to_string())?;
        Ok(result)
    }

    pub async fn execute_and_record_with_store<O: OperationRepository, H: HistoryRepository>(
        &self,
        task_id: Uuid,
        plan: &mut OperationPlan,
        approved_hash: &str,
        operations: &O,
        history: &H,
    ) -> Result<ToolResult, String> {
        operations.prepare().map_err(|error| error.to_string())?;
        history
            .prepare(task_id)
            .map_err(|error| error.to_string())?;
        let result = self.execute(task_id, plan, approved_hash).await;
        operations.save(plan).map_err(|error| error.to_string())?;
        let result = result?;
        let evidence = Evidence::from_tool(
            result.output.clone(),
            plan.target.clone(),
            Some(plan.tool_id.clone()),
        );
        history
            .append(task_id, &evidence)
            .map_err(|error| error.to_string())?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::port::{PlanDecision, PortFuture};
    use std::sync::{Arc, Mutex};

    struct ScriptedPlanner(Mutex<usize>);
    impl PlannerPort for ScriptedPlanner {
        fn plan<'a>(&'a self, _task: &'a TaskSnapshot) -> PortFuture<'a, PlanDecision> {
            Box::pin(async move {
                let mut step = self.0.lock().unwrap();
                let decision = if *step == 0 {
                    PlanDecision::Observe {
                        tool: "show_state".into(),
                        target: Some("sw1".into()),
                        args: serde_json::json!({"resource": "interfaces"}),
                    }
                } else {
                    PlanDecision::Complete {
                        brief: "診断完了".into(),
                    }
                };
                *step += 1;
                Ok(decision)
            })
        }
    }
    struct FakeExecutor(Arc<Mutex<usize>>);
    impl ToolExecutorPort for FakeExecutor {
        fn execute<'a>(
            &'a self,
            _task_id: Uuid,
            _tool: &'a str,
            _target: Option<&'a str>,
            _args: &'a serde_json::Value,
        ) -> PortFuture<'a, ToolResult> {
            Box::pin(async move {
                *self.0.lock().unwrap() += 1;
                Ok(ToolResult {
                    success: true,
                    output: "ok".into(),
                })
            })
        }
    }
    #[derive(Default)]
    struct Events(Mutex<Vec<ReportEvent>>);
    impl ReporterPort for Events {
        fn report(&self, event: ReportEvent) {
            self.0.lock().unwrap().push(event);
        }
    }
    #[derive(Default)]
    struct History(Mutex<Vec<Evidence>>);
    impl HistoryRepository for History {
        fn append(&self, _task_id: Uuid, evidence: &Evidence) -> ApplicationResult<()> {
            self.0.lock().unwrap().push(evidence.clone());
            Ok(())
        }
    }

    #[test]
    fn chat_service_runs_observation_then_completion_without_tauri() {
        let planner = ScriptedPlanner(Mutex::new(0));
        let executor = FakeExecutor(Arc::new(Mutex::new(0)));
        let reporter = Events::default();
        let task = TaskSnapshot::new("sw1を診断");
        let answer = futures_lite::future::block_on(
            ChatService::new(&planner, &executor, &reporter).answer(task),
        )
        .unwrap();
        assert_eq!(answer, "診断完了");
        assert_eq!(*executor.0.lock().unwrap(), 1);
        assert_eq!(reporter.0.lock().unwrap().len(), 4);
    }

    #[test]
    fn change_service_requires_approval_and_records_evidence() {
        let calls = Arc::new(Mutex::new(0));
        let executor = FakeExecutor(calls.clone());
        let history = History::default();
        let mut plan = ChangeService::<FakeExecutor>::create_plan(
            "network_config",
            Some("sw1".into()),
            serde_json::json!({"command": "vlan 10"}),
            "ユーザーが設定変更を依頼",
        )
        .unwrap();
        let id = plan.plan_hash.clone();
        let service = ChangeService::new(&executor);
        assert!(futures_lite::future::block_on(service.execute_and_record(
            Uuid::new_v4(),
            &mut plan,
            "wrong",
            &history
        ))
        .is_err());
        ChangeService::<FakeExecutor>::approve_plan(&mut plan, &id).unwrap();
        let result = futures_lite::future::block_on(service.execute_and_record(
            Uuid::new_v4(),
            &mut plan,
            &id,
            &history,
        ))
        .unwrap();
        assert!(result.success);
        assert_eq!(*calls.lock().unwrap(), 1);
        assert_eq!(history.0.lock().unwrap().len(), 1);
    }
}
