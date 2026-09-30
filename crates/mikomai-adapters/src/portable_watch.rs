//! Portable background scheduling and CPU watch evaluation.
//!
//! The host owns the Tokio runtime and supplies the device probe and
//! notification adapters. This module persists watch definitions and run
//! history in a `watches.json`-compatible file and never creates a runtime.

use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{sync::watch, task::JoinHandle};
use uuid::Uuid;

const MAX_RUN_HISTORY: usize = 256;
const DEFAULT_TICK: Duration = Duration::from_secs(1);

pub type WatchFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

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

impl ExecutionIr {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err("Only Execution IR version 1 is supported".into());
        }
        self.interval_seconds()?;
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
                    if !condition.when.right.is_finite() {
                        return Err("comparison threshold must be a finite number".into());
                    }
                    if condition.then_actions.is_empty()
                        || condition.then_actions.iter().any(|action| {
                            action.call != "notify" || action.args.message.trim().is_empty()
                        })
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

    pub fn interval_seconds(&self) -> Result<u64, String> {
        let seconds = self
            .schedule
            .every
            .trim()
            .strip_suffix('s')
            .ok_or("schedule.every must use seconds, for example '60s'")?
            .parse::<u64>()
            .map_err(|_| "schedule.every must be a positive number of seconds")?;
        if seconds == 0 {
            return Err("schedule.every must be greater than zero".into());
        }
        if seconds > 59
            && seconds != 86_400
            && !(seconds % 3_600 == 0 && seconds / 3_600 < 24 && 24 % (seconds / 3_600) == 0)
            && !(seconds % 60 == 0 && seconds / 60 <= 59)
        {
            return Err(
                "schedule.every must map to a whole-minute, supported hourly, or daily interval"
                    .into(),
            );
        }
        Ok(seconds)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchNotification {
    pub watch_id: Uuid,
    pub message: String,
    pub emitted_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchRunRecord {
    pub run_id: Uuid,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
    pub notifications: Vec<WatchNotification>,
    pub error: Option<String>,
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
    #[serde(default)]
    pub history: Vec<WatchRunRecord>,
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

pub trait WatchPrimitiveExecutor: Send + Sync {
    fn execute<'a>(&'a self, call: &'a CallStep) -> WatchFuture<'a, Result<Value, String>>;
}

pub trait WatchNotificationSink: Send + Sync {
    fn notify(&self, notification: &WatchNotification) -> Result<(), String>;
}

#[derive(Default)]
struct WatchStore {
    watches: Vec<WatchDefinition>,
    next_due: HashMap<Uuid, DateTime<Utc>>,
    running: HashSet<Uuid>,
}

pub struct PortableWatchService {
    path: PathBuf,
    store: Mutex<WatchStore>,
}

impl PortableWatchService {
    /// Open existing Tauri-compatible watch state, or create an empty store.
    /// The host supplies the path, usually `app-data/watches.json`.
    pub fn at(path: impl Into<PathBuf>) -> Result<Self, String> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "Failed to create watch storage directory {}: {error}",
                    parent.display()
                )
            })?;
        }
        let watches = load_watches(&path);
        Ok(Self {
            path,
            store: Mutex::new(WatchStore {
                watches,
                ..WatchStore::default()
            }),
        })
    }

    pub fn storage_path(&self) -> &Path {
        &self.path
    }

    pub fn list(&self) -> Result<Vec<WatchDefinition>, String> {
        Ok(self.lock()?.watches.clone())
    }

    pub fn get(&self, id: Uuid) -> Result<WatchDefinition, String> {
        self.lock()?
            .watches
            .iter()
            .find(|watch| watch.id == id)
            .cloned()
            .ok_or_else(|| "Watch was not found".into())
    }

    pub fn create(&self, request: CreateWatchRequest) -> Result<WatchDefinition, String> {
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
            history: Vec::new(),
        };
        self.mutate(|store| store.watches.push(watch.clone()))?;
        Ok(watch)
    }

    pub fn update(&self, id: Uuid, request: UpdateWatchRequest) -> Result<WatchDefinition, String> {
        if request.name.trim().is_empty() {
            return Err("Watch name is required".into());
        }
        request.ir.validate()?;
        let mut updated = None;
        self.mutate(|store| {
            if let Some(watch) = store.watches.iter_mut().find(|watch| watch.id == id) {
                watch.name = request.name;
                watch.ir = request.ir;
                updated = Some(watch.clone());
                store.next_due.remove(&id);
            }
        })?;
        updated.ok_or_else(|| "Watch was not found".into())
    }

    pub fn enable(&self, id: Uuid) -> Result<WatchDefinition, String> {
        self.set_status(id, WatchStatus::Enabled)
    }

    pub fn disable(&self, id: Uuid) -> Result<WatchDefinition, String> {
        self.set_status(id, WatchStatus::Disabled)
    }

    pub fn delete(&self, id: Uuid) -> Result<(), String> {
        let mut removed = false;
        self.mutate(|store| {
            let before = store.watches.len();
            store.watches.retain(|watch| watch.id != id);
            removed = store.watches.len() != before;
            store.next_due.remove(&id);
        })?;
        if removed {
            Ok(())
        } else {
            Err("Watch was not found".into())
        }
    }

    /// Run one enabled watch immediately. Disabled watches are an intentional
    /// no-op, preserving the retired command's behavior.
    pub async fn run_now(
        &self,
        id: Uuid,
        executor: &dyn WatchPrimitiveExecutor,
        sink: &dyn WatchNotificationSink,
    ) -> Result<Option<WatchRunRecord>, String> {
        self.run_one(id, executor, sink).await
    }

    /// Evaluate due watches at an injected time. Calling once at an initial
    /// fake time arms each schedule; advancing the time makes it due.
    pub async fn run_due_at(
        &self,
        now: DateTime<Utc>,
        executor: &dyn WatchPrimitiveExecutor,
        sink: &dyn WatchNotificationSink,
    ) -> Result<Vec<WatchRunRecord>, String> {
        let due = {
            let mut store = self.lock()?;
            let enabled = store
                .watches
                .iter()
                .filter(|watch| watch.status == WatchStatus::Enabled)
                .map(|watch| {
                    watch
                        .ir
                        .interval_seconds()
                        .map(|seconds| (watch.id, seconds))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let mut due = Vec::new();
            for (id, seconds) in enabled {
                let next = store
                    .next_due
                    .entry(id)
                    .or_insert_with(|| next_boundary_after(now, seconds));
                if *next <= now {
                    due.push(id);
                    *next = next_boundary_after(now, seconds);
                }
            }
            due
        };

        let mut records = Vec::with_capacity(due.len());
        for id in due {
            if let Some(record) = self.run_one(id, executor, sink).await? {
                records.push(record);
            }
        }
        Ok(records)
    }

    /// Start periodic due checks on the host's current Tokio runtime.
    pub fn start(
        self: Arc<Self>,
        executor: Arc<dyn WatchPrimitiveExecutor>,
        sink: Arc<dyn WatchNotificationSink>,
        poll_interval: Duration,
    ) -> Result<WatchSchedulerHandle, String> {
        if tokio::runtime::Handle::try_current().is_err() {
            return Err("Watch scheduler must be started on the shared Tokio runtime".into());
        }
        if poll_interval.is_zero() {
            return Err("Watch scheduler poll interval must be positive".into());
        }
        let (stop_tx, mut stop_rx) = watch::channel(false);
        let join = tokio::spawn(async move {
            let mut interval = tokio::time::interval(poll_interval.max(DEFAULT_TICK));
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let _ = self.run_due_at(Utc::now(), executor.as_ref(), sink.as_ref()).await;
                    }
                    changed = stop_rx.changed() => {
                        if changed.is_err() || *stop_rx.borrow() {
                            break;
                        }
                    }
                }
            }
        });
        Ok(WatchSchedulerHandle {
            stop: stop_tx,
            join: Some(join),
        })
    }

    fn set_status(&self, id: Uuid, status: WatchStatus) -> Result<WatchDefinition, String> {
        let mut updated = None;
        self.mutate(|store| {
            if let Some(watch) = store.watches.iter_mut().find(|watch| watch.id == id) {
                watch.status = status;
                updated = Some(watch.clone());
                store.next_due.remove(&id);
            }
        })?;
        updated.ok_or_else(|| "Watch was not found".into())
    }

    async fn run_one(
        &self,
        id: Uuid,
        executor: &dyn WatchPrimitiveExecutor,
        sink: &dyn WatchNotificationSink,
    ) -> Result<Option<WatchRunRecord>, String> {
        let watch = {
            let mut store = self.lock()?;
            let Some(watch) = store.watches.iter().find(|watch| watch.id == id).cloned() else {
                return Err("Watch was not found".into());
            };
            if watch.status != WatchStatus::Enabled || !store.running.insert(id) {
                return Ok(None);
            }
            watch
        };

        let started_at = Utc::now();
        let mut notifications = Vec::new();
        let error = execute_watch(&watch, executor, sink, &mut notifications)
            .await
            .err();
        let completed_at = Utc::now();
        let record = WatchRunRecord {
            run_id: Uuid::new_v4(),
            started_at,
            completed_at,
            notifications,
            error,
        };
        let save_result = self.mutate(|store| {
            store.running.remove(&id);
            if let Some(saved) = store.watches.iter_mut().find(|saved| saved.id == id) {
                saved.last_run_at = Some(record.completed_at);
                saved.last_error = record.error.clone();
                saved.history.push(record.clone());
                if saved.history.len() > MAX_RUN_HISTORY {
                    let remove = saved.history.len() - MAX_RUN_HISTORY;
                    saved.history.drain(..remove);
                }
            }
        });
        save_result?;
        Ok(Some(record))
    }

    fn mutate(&self, update: impl FnOnce(&mut WatchStore)) -> Result<(), String> {
        let mut store = self.lock()?;
        let previous = store.watches.clone();
        let previous_due = store.next_due.clone();
        update(&mut store);
        if let Err(error) = save_watches(&self.path, &store.watches) {
            store.watches = previous;
            store.next_due = previous_due;
            return Err(error);
        }
        Ok(())
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, WatchStore>, String> {
        self.store
            .lock()
            .map_err(|_| "Watch service state is unavailable".to_owned())
    }
}

pub struct WatchSchedulerHandle {
    stop: watch::Sender<bool>,
    join: Option<JoinHandle<()>>,
}

impl WatchSchedulerHandle {
    pub async fn stop(mut self) -> Result<(), String> {
        self.stop.send_replace(true);
        if let Some(join) = self.join.take() {
            join.await
                .map_err(|error| format!("Watch scheduler failed to stop cleanly: {error}"))?;
        }
        Ok(())
    }
}

impl Drop for WatchSchedulerHandle {
    fn drop(&mut self) {
        self.stop.send_replace(true);
    }
}

fn load_watches(path: &Path) -> Vec<WatchDefinition> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|contents| serde_json::from_str(&contents).ok())
        .unwrap_or_default()
}

fn save_watches(path: &Path, watches: &[WatchDefinition]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Failed to create watch storage directory {}: {error}",
                parent.display()
            )
        })?;
    }
    let json = serde_json::to_vec_pretty(watches).map_err(|error| error.to_string())?;
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(format!(".{}.tmp", Uuid::new_v4()));
    let temporary = PathBuf::from(temporary);
    std::fs::write(&temporary, json).map_err(|error| {
        format!(
            "Failed to write watch state {}: {error}",
            temporary.display()
        )
    })?;
    std::fs::rename(&temporary, path).map_err(|error| {
        let _ = std::fs::remove_file(&temporary);
        format!("Failed to persist watch state {}: {error}", path.display())
    })
}

fn next_boundary_after(now: DateTime<Utc>, seconds: u64) -> DateTime<Utc> {
    let interval = i64::try_from(seconds).unwrap_or(i64::MAX);
    let timestamp = now.timestamp();
    let next = timestamp
        .div_euclid(interval)
        .saturating_add(1)
        .saturating_mul(interval);
    Utc.timestamp_opt(next, 0)
        .single()
        .unwrap_or(now + chrono::Duration::seconds(interval))
}

async fn execute_watch(
    watch: &WatchDefinition,
    executor: &dyn WatchPrimitiveExecutor,
    sink: &dyn WatchNotificationSink,
    notifications: &mut Vec<WatchNotification>,
) -> Result<(), String> {
    let mut values = HashMap::new();
    for step in &watch.ir.steps {
        match step {
            ExecutionStep::Call(call) => {
                let value = executor.execute(call).await?;
                let usage = value
                    .get("usage")
                    .and_then(Value::as_f64)
                    .filter(|usage| usage.is_finite() && (0.0..=100.0).contains(usage))
                    .ok_or_else(|| {
                        format!(
                            "CPU probe '{}' must return numeric usage in the range 0..=100",
                            call.id
                        )
                    })?;
                values.insert(call.id.clone(), json!({"usage": usage}));
            }
            ExecutionStep::When(condition) => {
                let value = resolve_number(&values, &condition.when.left)?;
                if matches(&condition.when.operator, value, condition.when.right) {
                    for action in &condition.then_actions {
                        let notification = WatchNotification {
                            watch_id: watch.id,
                            message: action.args.message.clone(),
                            emitted_at: Utc::now(),
                        };
                        sink.notify(&notification)?;
                        notifications.push(notification);
                    }
                }
            }
        }
    }
    Ok(())
}

fn resolve_number(values: &HashMap<String, Value>, reference: &Reference) -> Result<f64, String> {
    let (step, field) = reference
        .path
        .split_once('.')
        .ok_or("ref must have the form <step_id>.<field>")?;
    values
        .get(step)
        .and_then(|value| value.get(field))
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite() && (0.0..=100.0).contains(value))
        .ok_or_else(|| {
            format!(
                "No valid CPU usage in range 0..=100 exists for ref '{}'",
                reference.path
            )
        })
}

fn matches(operator: &ComparisonOperator, left: f64, right: f64) -> bool {
    match operator {
        ComparisonOperator::Eq => left == right,
        ComparisonOperator::Ne => left != right,
        ComparisonOperator::Gt => left > right,
        ComparisonOperator::Gte => left >= right,
        ComparisonOperator::Lt => left < right,
        ComparisonOperator::Lte => left <= right,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn request() -> CreateWatchRequest {
        serde_json::from_value(json!({
            "name": "CPU threshold",
            "ir": {
                "version": 1,
                "schedule": { "every": "60s" },
                "steps": [
                    { "id": "cpu", "call": "get_state", "args": { "device": "rt01", "resource": "cpu" } },
                    { "when": { "left": { "ref": "cpu.usage" }, "operator": "gt", "right": 80 },
                      "then": [{ "call": "notify", "args": { "message": "CPU high" } }] }
                ]
            }
        }))
        .unwrap()
    }

    struct FakeProbe(AtomicU64);

    impl WatchPrimitiveExecutor for FakeProbe {
        fn execute<'a>(&'a self, _call: &'a CallStep) -> WatchFuture<'a, Result<Value, String>> {
            Box::pin(async move { Ok(json!({"usage": self.0.load(Ordering::SeqCst) as f64})) })
        }
    }

    #[derive(Default)]
    struct FakeSink(Mutex<Vec<WatchNotification>>);

    impl WatchNotificationSink for FakeSink {
        fn notify(&self, notification: &WatchNotification) -> Result<(), String> {
            self.0
                .lock()
                .map_err(|_| "fake sink lock poisoned".to_owned())?
                .push(notification.clone());
            Ok(())
        }
    }

    fn path() -> PathBuf {
        std::env::temp_dir().join(format!("mikomai-watch-{}.json", Uuid::new_v4()))
    }

    #[test]
    fn validates_legacy_schedule_boundaries_and_watch_ir() {
        let mut watch = request().ir;
        assert!(watch.validate().is_ok());
        for (every, allowed) in [
            ("1s", true),
            ("59s", true),
            ("60s", true),
            ("3540s", true),
            ("3600s", true),
            ("7200s", true),
            ("43200s", true),
            ("86400s", true),
            ("0s", false),
            ("61s", false),
            ("18000s", false),
            ("172800s", false),
        ] {
            watch.schedule.every = every.into();
            assert_eq!(watch.validate().is_ok(), allowed, "schedule {every}");
        }
        watch.schedule.every = "60s".into();
        if let ExecutionStep::When(condition) = &mut watch.steps[1] {
            condition.when.left.path = "future.usage".into();
        }
        assert!(watch.validate().is_err());
        if let ExecutionStep::When(condition) = &mut watch.steps[1] {
            condition.when.left.path = "cpu.usage".into();
            condition.then_actions[0].call = "network_config".into();
        }
        assert!(watch.validate().is_err());
    }

    #[tokio::test]
    async fn fake_clock_polls_only_when_due_notifies_on_change_and_persists_history() {
        let file = path();
        let service = PortableWatchService::at(&file).unwrap();
        let watch = service.create(request()).unwrap();
        let probe = FakeProbe(AtomicU64::new(90));
        let sink = FakeSink::default();
        let start = Utc.with_ymd_and_hms(2026, 9, 30, 0, 0, 1).unwrap();

        assert!(service
            .run_due_at(start, &probe, &sink)
            .await
            .unwrap()
            .is_empty());
        let first = service
            .run_due_at(start + chrono::Duration::seconds(60), &probe, &sink)
            .await
            .unwrap();
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].notifications.len(), 1);
        assert_eq!(first[0].notifications[0].message, "CPU high");

        probe.0.store(50, Ordering::SeqCst);
        let second = service
            .run_due_at(start + chrono::Duration::seconds(120), &probe, &sink)
            .await
            .unwrap();
        assert_eq!(second.len(), 1);
        assert!(second[0].notifications.is_empty());
        assert_eq!(sink.0.lock().unwrap().len(), 1);

        let restored = PortableWatchService::at(&file).unwrap();
        let saved = restored.get(watch.id).unwrap();
        assert!(saved.last_run_at.is_some());
        assert_eq!(saved.history.len(), 2);
        assert_eq!(saved.history[0].notifications.len(), 1);
        assert!(saved.last_error.is_none());
        let _ = std::fs::remove_file(file);
    }

    #[tokio::test]
    async fn probe_error_is_recorded_and_disabled_watch_does_not_run() {
        struct FailingProbe;
        impl WatchPrimitiveExecutor for FailingProbe {
            fn execute<'a>(
                &'a self,
                _call: &'a CallStep,
            ) -> WatchFuture<'a, Result<Value, String>> {
                Box::pin(async { Err("device unavailable".into()) })
            }
        }

        let file = path();
        let service = PortableWatchService::at(&file).unwrap();
        let watch = service.create(request()).unwrap();
        let sink = FakeSink::default();
        let failed = service
            .run_now(watch.id, &FailingProbe, &sink)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(failed.error.as_deref(), Some("device unavailable"));
        assert_eq!(
            service.get(watch.id).unwrap().last_error.as_deref(),
            Some("device unavailable")
        );

        service.disable(watch.id).unwrap();
        assert!(service
            .run_now(watch.id, &FailingProbe, &sink)
            .await
            .unwrap()
            .is_none());
        let _ = std::fs::remove_file(file);
    }

    #[tokio::test]
    async fn missing_and_out_of_range_usage_are_recorded_as_probe_errors() {
        struct InvalidProbe(Value);
        impl WatchPrimitiveExecutor for InvalidProbe {
            fn execute<'a>(
                &'a self,
                _call: &'a CallStep,
            ) -> WatchFuture<'a, Result<Value, String>> {
                let value = self.0.clone();
                Box::pin(async move { Ok(value) })
            }
        }

        for value in [json!({"usage": null}), json!({"usage": 101.0})] {
            let file = path();
            let service = PortableWatchService::at(&file).unwrap();
            let watch = service.create(request()).unwrap();
            let record = service
                .run_now(watch.id, &InvalidProbe(value), &FakeSink::default())
                .await
                .unwrap()
                .unwrap();
            assert!(record
                .error
                .as_deref()
                .is_some_and(|error| error.contains("0..=100")));
            let _ = std::fs::remove_file(file);
        }
    }

    #[tokio::test]
    async fn shared_runtime_scheduler_stops_without_later_polls() {
        use std::sync::atomic::AtomicUsize;
        struct CountingProbe(AtomicUsize);
        impl WatchPrimitiveExecutor for CountingProbe {
            fn execute<'a>(
                &'a self,
                _call: &'a CallStep,
            ) -> WatchFuture<'a, Result<Value, String>> {
                Box::pin(async move {
                    self.0.fetch_add(1, Ordering::SeqCst);
                    Ok(json!({"usage": 5.0}))
                })
            }
        }

        let file = path();
        let service = Arc::new(PortableWatchService::at(&file).unwrap());
        let mut request = request();
        request.ir.schedule.every = "1s".into();
        service.create(request).unwrap();
        let probe = Arc::new(CountingProbe(AtomicUsize::new(0)));
        let sink = Arc::new(FakeSink::default());
        let scheduler = service
            .clone()
            .start(probe.clone(), sink, Duration::from_millis(20))
            .unwrap();
        tokio::time::sleep(Duration::from_millis(2_200)).await;
        assert!(probe.0.load(Ordering::SeqCst) > 0);
        scheduler.stop().await.unwrap();
        let stopped_count = probe.0.load(Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(1_100)).await;
        assert_eq!(probe.0.load(Ordering::SeqCst), stopped_count);
        let _ = std::fs::remove_file(file);
    }

    #[test]
    fn old_watch_file_without_history_still_loads_and_corrupt_file_is_empty() {
        let file = path();
        let ir = serde_json::to_value(request().ir).unwrap();
        let definition = json!({
            "id": Uuid::new_v4(),
            "name": "old watch",
            "status": "enabled",
            "ir": ir,
            "createdAt": Utc::now(),
            "lastRunAt": null,
            "lastError": null
        });
        std::fs::write(&file, serde_json::to_vec(&json!([definition])).unwrap()).unwrap();
        let restored = PortableWatchService::at(&file).unwrap();
        assert!(restored.list().unwrap()[0].history.is_empty());
        std::fs::write(&file, "invalid json").unwrap();
        assert!(PortableWatchService::at(&file)
            .unwrap()
            .list()
            .unwrap()
            .is_empty());
        let _ = std::fs::remove_file(file);
    }
}
