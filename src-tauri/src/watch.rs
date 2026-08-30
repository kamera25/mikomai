//! A deliberately small, typed, deterministic execution IR for network watches.
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::HashMap, fs, path::PathBuf, sync::Arc};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::Mutex;
use tokio_cron_scheduler::{Job, JobScheduler};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WatchStatus {
    Enabled,
    Disabled,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PrimitiveName {
    GetState,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WatchResource {
    Cpu,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GetStateArgs {
    pub device: String,
    pub resource: WatchResource,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CallStep {
    pub id: String,
    pub call: PrimitiveName,
    pub args: GetStateArgs,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonOperator {
    Eq,
    Ne,
    Gt,
    Gte,
    Lt,
    Lte,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Reference {
    #[serde(rename = "ref")]
    pub path: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NotificationArgs {
    pub message: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NotificationAction {
    pub call: String,
    pub args: NotificationArgs,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Comparison {
    pub left: Reference,
    pub operator: ComparisonOperator,
    pub right: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WhenStep {
    pub when: Comparison,
    #[serde(rename = "then")]
    pub then_actions: Vec<NotificationAction>,
}
/// This compact untagged shape mirrors the JSON/YAML IR exposed to the Agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ExecutionStep {
    Call(CallStep),
    When(WhenStep),
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EverySchedule {
    pub every: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExecutionIr {
    pub version: u8,
    pub schedule: EverySchedule,
    pub steps: Vec<ExecutionStep>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchDefinition {
    pub id: Uuid,
    pub name: String,
    pub status: WatchStatus,
    pub ir: ExecutionIr,
    pub created_at: DateTime<Utc>,
    pub last_run_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateWatchRequest {
    pub name: String,
    pub ir: ExecutionIr,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateWatchRequest {
    pub name: String,
    pub ir: ExecutionIr,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchNotification {
    pub watch_id: Uuid,
    pub message: String,
    pub emitted_at: DateTime<Utc>,
}
pub struct WatchState {
    scheduler: Arc<Mutex<JobScheduler>>,
    watches: Arc<Mutex<Vec<WatchDefinition>>>,
}

fn watches_path(app: &AppHandle) -> PathBuf {
    let p = app
        .path()
        .app_data_dir()
        .expect("app data directory is available");
    let _ = fs::create_dir_all(&p);
    p.join("watches.json")
}
fn persist(app: &AppHandle, watches: &[WatchDefinition]) -> Result<(), String> {
    fs::write(
        watches_path(app),
        serde_json::to_string_pretty(watches).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}
fn every_to_cron(every: &str) -> Result<String, String> {
    let seconds = every
        .trim()
        .strip_suffix('s')
        .ok_or("schedule.every must use seconds, for example '60s'")?
        .parse::<u64>()
        .map_err(|_| "schedule.every must be a positive number of seconds")?;
    if seconds == 0 {
        return Err("schedule.every must be greater than zero".into());
    }
    if seconds > 59 {
        if seconds % 60 != 0 {
            return Err("schedule.every above 59 seconds must be a whole number of minutes".into());
        }
        return Ok(format!("0 */{} * * * *", seconds / 60));
    }
    Ok(format!("*/{} * * * * *", seconds))
}
impl ExecutionIr {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err("Only Execution IR version 1 is supported".into());
        }
        every_to_cron(&self.schedule.every)?;
        let mut calls = HashMap::new();
        for step in &self.steps {
            match step {
                ExecutionStep::Call(call) => {
                    if call.id.trim().is_empty() || calls.insert(call.id.clone(), ()).is_some() {
                        return Err("Each call step requires a unique id".into());
                    }
                    if call.args.device.trim().is_empty() {
                        return Err("get_state requires a device".into());
                    }
                }
                ExecutionStep::When(condition) => {
                    let (id, field) = condition
                        .when
                        .left
                        .path
                        .split_once('.')
                        .ok_or("ref must have the form <step_id>.<field>")?;
                    if !calls.contains_key(id) || field != "usage" {
                        return Err("ref must point to an earlier CPU step's usage field".into());
                    }
                    if condition.then_actions.is_empty()
                        || condition
                            .then_actions
                            .iter()
                            .any(|a| a.call != "notify" || a.args.message.trim().is_empty())
                    {
                        return Err("when.then supports notify actions with a message".into());
                    }
                }
            }
        }
        if calls.is_empty() {
            Err("IR requires at least one call step".into())
        } else {
            Ok(())
        }
    }
}
async fn invoke_primitive(app: &AppHandle, call: &CallStep) -> Result<Value, String> {
    match (&call.call, &call.args.resource) {
        (PrimitiveName::GetState, WatchResource::Cpu) => Ok(
            json!({"usage": crate::mcp::fetch::get_state::fetch_cpu_usage(app, &call.args.device).await?}),
        ),
    }
}
fn resolve_number(values: &HashMap<String, Value>, reference: &Reference) -> Result<f64, String> {
    let (step, field) = reference
        .path
        .split_once('.')
        .ok_or("ref must have the form <step_id>.<field>")?;
    values
        .get(step)
        .and_then(|v| v.get(field))
        .and_then(Value::as_f64)
        .ok_or_else(|| format!("No numeric value exists for ref '{}'", reference.path))
}
fn matches(op: &ComparisonOperator, left: f64, right: f64) -> bool {
    match op {
        ComparisonOperator::Eq => left == right,
        ComparisonOperator::Ne => left != right,
        ComparisonOperator::Gt => left > right,
        ComparisonOperator::Gte => left >= right,
        ComparisonOperator::Lt => left < right,
        ComparisonOperator::Lte => left <= right,
    }
}
async fn execute_watch(app: &AppHandle, watch: &WatchDefinition) -> Result<(), String> {
    let mut values = HashMap::new();
    for step in &watch.ir.steps {
        match step {
            ExecutionStep::Call(call) => {
                values.insert(call.id.clone(), invoke_primitive(app, call).await?);
            }
            ExecutionStep::When(condition) => {
                if matches(
                    &condition.when.operator,
                    resolve_number(&values, &condition.when.left)?,
                    condition.when.right,
                ) {
                    for action in &condition.then_actions {
                        app.emit(
                            "watch-notification",
                            WatchNotification {
                                watch_id: watch.id,
                                message: action.args.message.clone(),
                                emitted_at: Utc::now(),
                            },
                        )
                        .map_err(|e| e.to_string())?;
                    }
                }
            }
        }
    }
    Ok(())
}
async fn run_and_record(app: AppHandle, id: Uuid) {
    let watch = {
        let state: State<WatchState> = app.state();
        let watch = state
            .watches
            .lock()
            .await
            .iter()
            .find(|w| w.id == id && w.status == WatchStatus::Enabled)
            .cloned();
        watch
    };
    let Some(watch) = watch else {
        return;
    };
    let result = execute_watch(&app, &watch).await;
    let state: State<WatchState> = app.state();
    let mut watches = state.watches.lock().await;
    if let Some(saved) = watches.iter_mut().find(|w| w.id == id) {
        saved.last_run_at = Some(Utc::now());
        saved.last_error = result.err();
    }
    if let Err(error) = persist(&app, &watches) {
        tracing::error!(%error, "Failed to persist watch run result");
    }
    let _ = app.emit("watch-executed", id);
}
async fn install_enabled_jobs(
    app: &AppHandle,
    scheduler: &JobScheduler,
    watches: &[WatchDefinition],
) {
    for watch in watches.iter().filter(|w| w.status == WatchStatus::Enabled) {
        let Ok(cron) = every_to_cron(&watch.ir.schedule.every) else {
            continue;
        };
        let id = watch.id;
        let handle = app.clone();
        if let Ok(job) = Job::new_async(&cron, move |_, _| {
            let app = handle.clone();
            Box::pin(async move {
                run_and_record(app, id).await;
            })
        }) {
            let _ = scheduler.add(job).await;
        }
    }
}
async fn rebuild_scheduler(app: &AppHandle, state: &WatchState) {
    let watches = state.watches.lock().await.clone();
    let scheduler = JobScheduler::new()
        .await
        .expect("watch scheduler can be created");
    install_enabled_jobs(app, &scheduler, &watches).await;
    scheduler
        .start()
        .await
        .expect("watch scheduler can be started");
    *state.scheduler.lock().await = scheduler;
}
pub async fn init_watch_scheduler(app: &AppHandle) -> WatchState {
    let watches: Vec<WatchDefinition> = fs::read_to_string(watches_path(app))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let scheduler = JobScheduler::new()
        .await
        .expect("watch scheduler can be created");
    install_enabled_jobs(app, &scheduler, &watches).await;
    scheduler
        .start()
        .await
        .expect("watch scheduler can be started");
    WatchState {
        scheduler: Arc::new(Mutex::new(scheduler)),
        watches: Arc::new(Mutex::new(watches)),
    }
}

#[tauri::command]
pub async fn create_watch(
    app: AppHandle,
    request: CreateWatchRequest,
    state: State<'_, WatchState>,
) -> Result<WatchDefinition, String> {
    if request.name.trim().is_empty() {
        return Err("Watch name is required".into());
    }
    request.ir.validate()?;
    let watch = WatchDefinition {
        id: Uuid::new_v4(),
        name: request.name,
        status: WatchStatus::Enabled,
        ir: request.ir,
        created_at: Utc::now(),
        last_run_at: None,
        last_error: None,
    };
    {
        let mut watches = state.watches.lock().await;
        watches.push(watch.clone());
        persist(&app, &watches)?;
    }
    rebuild_scheduler(&app, &state).await;
    Ok(watch)
}
#[tauri::command]
pub async fn list_watches(state: State<'_, WatchState>) -> Result<Vec<WatchDefinition>, String> {
    Ok(state.watches.lock().await.clone())
}

#[tauri::command]
pub async fn update_watch(
    app: AppHandle,
    id: Uuid,
    request: UpdateWatchRequest,
    state: State<'_, WatchState>,
) -> Result<WatchDefinition, String> {
    if request.name.trim().is_empty() {
        return Err("Watch name is required".into());
    }
    request.ir.validate()?;
    let updated = {
        let mut watches = state.watches.lock().await;
        let watch = watches
            .iter_mut()
            .find(|watch| watch.id == id)
            .ok_or("Watch was not found")?;
        watch.name = request.name;
        watch.ir = request.ir;
        let updated = watch.clone();
        persist(&app, &watches)?;
        updated
    };
    rebuild_scheduler(&app, &state).await;
    Ok(updated)
}
#[tauri::command]
pub async fn get_watch(id: Uuid, state: State<'_, WatchState>) -> Result<WatchDefinition, String> {
    state
        .watches
        .lock()
        .await
        .iter()
        .find(|w| w.id == id)
        .cloned()
        .ok_or("Watch was not found".into())
}
#[tauri::command]
pub async fn delete_watch(
    app: AppHandle,
    id: Uuid,
    state: State<'_, WatchState>,
) -> Result<(), String> {
    {
        let mut watches = state.watches.lock().await;
        let before = watches.len();
        watches.retain(|w| w.id != id);
        if watches.len() == before {
            return Err("Watch was not found".into());
        }
        persist(&app, &watches)?;
    }
    rebuild_scheduler(&app, &state).await;
    Ok(())
}
async fn set_status(
    app: AppHandle,
    id: Uuid,
    status: WatchStatus,
    state: State<'_, WatchState>,
) -> Result<WatchDefinition, String> {
    let updated = {
        let mut watches = state.watches.lock().await;
        let watch = watches
            .iter_mut()
            .find(|w| w.id == id)
            .ok_or("Watch was not found")?;
        watch.status = status;
        let updated = watch.clone();
        persist(&app, &watches)?;
        updated
    };
    rebuild_scheduler(&app, &state).await;
    Ok(updated)
}
#[tauri::command]
pub async fn enable_watch(
    app: AppHandle,
    id: Uuid,
    state: State<'_, WatchState>,
) -> Result<WatchDefinition, String> {
    set_status(app, id, WatchStatus::Enabled, state).await
}
#[tauri::command]
pub async fn disable_watch(
    app: AppHandle,
    id: Uuid,
    state: State<'_, WatchState>,
) -> Result<WatchDefinition, String> {
    set_status(app, id, WatchStatus::Disabled, state).await
}
#[tauri::command]
pub async fn execute_watch_now(
    app: AppHandle,
    id: Uuid,
    state: State<'_, WatchState>,
) -> Result<(), String> {
    if !state.watches.lock().await.iter().any(|w| w.id == id) {
        return Err("Watch was not found".into());
    }
    run_and_record(app, id).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ir() -> ExecutionIr {
        serde_json::from_value(json!({"version":1,"schedule":{"every":"60s"},"steps":[{"id":"cpu","call":"get_state","args":{"device":"rt01","resource":"cpu"}},{"when":{"left":{"ref":"cpu.usage"},"operator":"gt","right":80},"then":[{"call":"notify","args":{"message":"high cpu"}}]}]})).unwrap()
    }
    #[test]
    fn validates_cpu_watch_ir() {
        assert!(ir().validate().is_ok());
    }
    #[test]
    fn rejects_unknown_reference() {
        let mut watch = ir();
        if let ExecutionStep::When(step) = &mut watch.steps[1] {
            step.when.left.path = "other.usage".into();
        }
        assert!(watch.validate().is_err());
    }
    #[test]
    fn compares_deterministically() {
        assert!(matches(&ComparisonOperator::Gt, 81.0, 80.0));
        assert!(!matches(&ComparisonOperator::Lte, 81.0, 80.0));
    }
}
