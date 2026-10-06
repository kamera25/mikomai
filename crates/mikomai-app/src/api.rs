//! Synchronous submission/query/cancellation with ordered recoverable events.
use crate::scheduling::TaskScheduler;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
};
use tokio::sync::watch;
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Command {
    Chat {
        message: String,
        history: String,
        documents_dir: String,
        knowledge_dir: String,
        attachments: String,
        devices_json: String,
        agent: bool,
    },
    ReadDevice {
        target: String,
        commands: Vec<String>,
        timeout_seconds: u32,
    },
    ExecuteApproved {
        plan_id: String,
        plan_hash: String,
    },
    Contract {
        fixture_json: String,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TaskEvent {
    pub task_id: String,
    pub seq: u64,
    pub version: u32,
    pub kind: String,
    pub payload: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub task_id: String,
    pub state: String,
    pub seq: u64,
    pub result: String,
    pub events: Vec<TaskEvent>,
}
pub trait EventListener: Send + Sync {
    fn on_event(&self, event: TaskEvent);
}
struct Task {
    snapshot: Snapshot,
    cancel: watch::Sender<bool>,
    resume: watch::Sender<u64>,
}
pub struct Engine {
    tasks: Mutex<HashMap<String, Task>>,
    listeners: Mutex<Vec<Arc<dyn EventListener>>>,
    pub(crate) scheduler: TaskScheduler,
}
impl Default for Engine {
    fn default() -> Self {
        Self {
            tasks: Mutex::new(HashMap::new()),
            listeners: Mutex::new(Vec::new()),
            scheduler: TaskScheduler::default(),
        }
    }
}
pub fn engine() -> &'static Arc<Engine> {
    static ENGINE: OnceLock<Arc<Engine>> = OnceLock::new();
    ENGINE.get_or_init(|| Arc::new(Engine::default()))
}
impl Engine {
    /// Journal a CLI controller's events through the same durable TaskEvent store.
    pub fn trace_event(&self, id: &str, kind: &str, payload: String) -> Result<TaskEvent, String> {
        if !self.tasks.lock().map_err(|_| "task state unavailable")?.contains_key(id) {
            let (cancel, _) = watch::channel(false);
            let (resume, _) = watch::channel(0);
            self.tasks.lock().map_err(|_| "task state unavailable")?.insert(id.into(), Task {
                snapshot: Snapshot { task_id: id.into(), state: "running".into(), seq: 0,
                    result: String::new(), events: Vec::new() }, cancel, resume,
            });
        }
        self.event(id, kind, payload)?;
        Ok(self.query(id)?.events.last().ok_or("missing event")?.clone())
    }
    pub fn subscribe(&self, listener: Arc<dyn EventListener>) {
        self.listeners.lock().unwrap().push(listener);
    }
    pub fn query(&self, id: &str) -> Result<Snapshot, String> {
        if let Some(task) = self
            .tasks
            .lock()
            .map_err(|_| "task state unavailable")?
            .get(id)
        {
            return Ok(task.snapshot.clone());
        }
        let value = crate::shared_service()
            .load_document("tasks")?
            .ok_or("task not found")?;
        serde_json::from_value(value["scheduled"][id].clone())
            .map_err(|_| "task not found or malformed".into())
    }
    fn event(&self, id: &str, kind: &str, payload: String) -> Result<(), String> {
        let event = {
            let mut tasks = self.tasks.lock().map_err(|_| "task state unavailable")?;
            let task = tasks.get_mut(id).ok_or("task not found")?;
            let mut snapshot = task.snapshot.clone();
            snapshot.seq += 1;
            if kind == "core_response" {
                let response: serde_json::Value = serde_json::from_str(&payload).map_err(|e| e.to_string())?;
                snapshot.state = if response["status"] == 0 { "completed" } else { "failed" }.into();
                snapshot.result = response["text"].as_str().unwrap_or_default().into();
            }
            if [
                "queued",
                "running",
                "waiting_device",
                "awaiting_user",
                "awaiting_approval",
                "completed",
                "failed",
                "cancelled",
                "unknown",
            ]
            .contains(&kind)
            {
                snapshot.state = kind.into();
            }
            if [
                "completed",
                "failed",
                "unknown",
                "awaiting_user",
                "awaiting_approval",
            ]
            .contains(&kind)
            {
                snapshot.result = payload.clone();
            }
            let event = TaskEvent {
                task_id: id.into(),
                seq: snapshot.seq,
                version: 1,
                kind: kind.into(),
                payload,
            };
            snapshot.events.push(event.clone());
            crate::shared_service().update_internal("tasks", |value| {
                if value.get("scheduled").is_none() {
                    value["scheduled"] = json!({});
                }
                value["scheduled"][id] =
                    serde_json::to_value(&snapshot).map_err(|e| e.to_string())?;
                Ok(())
            })?;
            task.snapshot = snapshot;
            event
        };
        // Release all storage/state locks before callbacks; listeners may query/reenter.
        let listeners = self
            .listeners
            .lock()
            .map_err(|_| "listeners unavailable")?
            .clone();
        for listener in listeners {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                listener.on_event(event.clone())
            }));
        }
        Ok(())
    }
    pub fn submit(self: &Arc<Self>, command: Command) -> Result<String, String> {
        let id = uuid::Uuid::new_v4().to_string();
        let (sender, receiver) = watch::channel(false);
        let (resume, _) = watch::channel(0);
        self.tasks
            .lock()
            .map_err(|_| "task state unavailable")?
            .insert(
                id.clone(),
                Task {
                    snapshot: Snapshot {
                        task_id: id.clone(),
                        state: "queued".into(),
                        seq: 0,
                        result: String::new(),
                        events: Vec::new(),
                    },
                    cancel: sender,
                    resume,
                },
            );
        if let Err(error) = self.event(&id, "queued", String::new()) {
            self.tasks.lock().unwrap().remove(&id);
            return Err(error);
        }
        let engine = self.clone();
        let task_id = id.clone();
        crate::shared_service().runtime()?.spawn(async move {
            let result = engine.run(&task_id, command, receiver.clone()).await;
            let (state, text) = match result {
                Ok((state, text)) => (state, text),
                Err(error) => {
                    if error.starts_with("unknown:") {
                        ("unknown".into(), error)
                    } else if *receiver.borrow() {
                        ("cancelled".into(), String::new())
                    } else {
                        ("failed".into(), error)
                    }
                }
            };
            if let Err(error) = engine.event(&task_id, &state, text) {
                if let Some(task) = engine.tasks.lock().unwrap().get_mut(&task_id) {
                    task.snapshot.state = "failed".into();
                    task.snapshot.result = format!("task result could not be saved: {error}");
                }
            }
        });
        Ok(id)
    }
    pub fn resume(&self, id: &str) -> Result<(), String> {
        let tasks = self.tasks.lock().map_err(|_| "task state unavailable")?;
        let task = tasks.get(id).ok_or("task not found")?;
        if task.snapshot.state != "awaiting_user" {
            return Err("task is not waiting for a lock decision".into());
        }
        task.resume.send_modify(|generation| *generation += 1);
        Ok(())
    }
    pub(crate) async fn wait_device(
        &self,
        id: &str,
        key: &str,
        write: bool,
        serial: bool,
        cancel: &mut watch::Receiver<bool>,
    ) -> Result<crate::scheduling::DeviceLease, String> {
        let mut resume = self
            .tasks
            .lock()
            .map_err(|_| "task state unavailable")?
            .get(id)
            .ok_or("task not found")?
            .resume
            .subscribe();
        let settings = crate::shared_service()
            .load_document("settings")?
            .unwrap_or(json!({}));
        let timeout = settings[if write {
            "deviceWriteLockWaitSeconds"
        } else {
            "deviceReadLockWaitSeconds"
        }]
        .as_u64()
        .filter(|n| *n > 0 && *n <= 3600)
        .unwrap_or(if write { 300 } else { 60 });
        loop {
            self.event(id, "waiting_device", String::new())?;
            let waiting = crate::shared_service()
                .device_locks
                .get_or_init(Default::default)
                .acquire(
                    key,
                    write,
                    serial,
                    false,
                    crate::portable_app_data_dir()?.join("locks"),
                    cancel,
                );
            match tokio::time::timeout(std::time::Duration::from_secs(timeout), waiting).await {
                Ok(result) => {
                    let lease = result?;
                    lock_audit(key, "lock_acquired", lease.waited.as_millis() as u64)?;
                    return Ok(lease);
                }
                Err(_) => {
                    lock_audit(key, "lock_wait_timeout", timeout * 1000)?;
                    self.event(id,"awaiting_user",json!({"reason":"device_lock_timeout","device":key,"wait_seconds":timeout,"message":"機器のロック待ちが続いています。待機を続けるか中止してください。"}).to_string())?;
                    if *cancel.borrow() {
                        return Err("cancelled".into());
                    }
                    tokio::select! {_=resume.changed()=>{},_=cancel.changed()=>return Err("cancelled".into())}
                }
            }
        }
    }
    pub fn cancel(&self, id: &str) -> Result<(), String> {
        self.tasks
            .lock()
            .map_err(|_| "task state unavailable")?
            .get(id)
            .ok_or("task not found")?
            .cancel
            .send(true)
            .map_err(|_| "task has ended".into())
    }
    async fn run(
        self: &Arc<Self>,
        id: &str,
        command: Command,
        mut cancel: watch::Receiver<bool>,
    ) -> Result<(String, String), String> {
        let mut permit = Some(self.scheduler.acquire(&mut cancel).await?);
        self.event(id, "running", String::new())?;
        match command {
            Command::Contract { fixture_json } => {
                let fixture: Value =
                    serde_json::from_str(&fixture_json).map_err(|e| e.to_string())?;
                for state in fixture["states"]
                    .as_array()
                    .ok_or("contract states missing")?
                {
                    let state = state.as_str().ok_or("invalid contract state")?;
                    if matches!(
                        state,
                        "awaiting_user" | "awaiting_approval" | "waiting_device"
                    ) {
                        permit.take();
                        self.event(id, state, String::new())?;
                    } else if state == "running" {
                        if permit.is_none() {
                            permit = Some(self.scheduler.acquire(&mut cancel).await?);
                        }
                        self.event(id, state, String::new())?;
                    } else {
                        return Err("invalid contract transition".into());
                    }
                }
                Ok((
                    "completed".into(),
                    fixture["result"].as_str().unwrap_or("").into(),
                ))
            }
            Command::ReadDevice {
                target,
                commands,
                timeout_seconds,
            } => {
                let connection = crate::native_execution::connection(&target)?;
                permit.take();
                self.event(id, "waiting_device", String::new())?;
                let result = crate::native_execution::read_task(
                    id,
                    &connection,
                    commands,
                    timeout_seconds,
                    &mut cancel,
                    &self.scheduler,
                )
                .await?;
                if result.status == "completed" {
                    Ok(("completed".into(), result.payload.to_string()))
                } else {
                    Ok((result.status, result.payload.to_string()))
                }
            }
            Command::ExecuteApproved { plan_id, plan_hash } => {
                // Lock waiting never occupies a scheduler slot.
                permit.take();
                self.event(id, "waiting_device", String::new())?;
                let task_cancel = cancel.clone();
                let task_id = id.to_string();
                let result = tokio::task::spawn_blocking(move || {
                    let _scope = TaskContext::enter(task_id);
                    crate::native_execution::execute_approved_with_cancel(
                        &plan_id,
                        &plan_hash,
                        task_cancel,
                    )
                })
                .await
                .map_err(|_| "operation worker failed")??;
                Ok(("completed".into(), result))
            }
            Command::Chat {
                message,
                history,
                documents_dir,
                knowledge_dir,
                attachments,
                devices_json,
                agent,
            } => {
                permit.take();
                permit = Some(self.scheduler.acquire(&mut cancel).await?);
                let sink: Arc<dyn crate::owned_bridge::LegacyListener> = Arc::new(ChatSink {
                    engine: self.clone(),
                    id: id.into(),
                });
                let args = vec![
                    message,
                    history,
                    documents_dir,
                    knowledge_dir,
                    attachments,
                    devices_json,
                ];
                let task_cancel = cancel.clone();
                let task_id = id.to_string();
                let slot = Arc::new(Mutex::new(permit.take()));
                let task_slot = slot.clone();
                let mut job = tokio::task::spawn_blocking(move || {
                    let _task_scope = TaskContext::enter(task_id);
                    let _slot_scope = TaskSlot::enter(task_slot);
                    let _scope = crate::scheduling::CancellationScope::enter(task_cancel);
                    crate::owned_bridge::invoke(
                        if agent {
                            "mikomai_agent_chat_streaming"
                        } else {
                            "mikomai_assistant_chat_streaming"
                        },
                        &args,
                        Some(sink),
                    )
                });
                let response = tokio::select! {response=&mut job=>response.map_err(|_|"chat worker failed")?,_=cancel.changed()=>{let _=job.await;return Err("cancelled".into());}}?;
                let state = if response.contains("__ASK_HUMAN__") {
                    "awaiting_user"
                } else if response.contains("__MIKOMAI_APPROVAL_PLAN__")
                    || response.contains("承認")
                {
                    "awaiting_approval"
                } else {
                    "completed"
                };
                slot.lock().unwrap().take();
                Ok((state.into(), response))
            }
        }
    }
}
struct ChatSink {
    engine: Arc<Engine>,
    id: String,
}
impl crate::owned_bridge::LegacyListener for ChatSink {
    fn event(&self, _kind: String, text: String, done: bool) {
        let kind = if text.starts_with("__MIKOMAI_DEBUG__") {
            "debug"
        } else if text.starts_with("__MIKOMAI_APPROVAL_PLAN__") {
            "operation_plan"
        } else {
            "stream"
        };
        if self
            .engine
            .event(&self.id, kind, json!({"text":text,"done":done}).to_string())
            .is_err()
        {
            let _ = self.engine.cancel(&self.id);
        }
    }
}

thread_local! {static TASK_ID:std::cell::RefCell<Option<String>>=const {std::cell::RefCell::new(None)};}
pub(crate) struct TaskContext(Option<String>);
impl TaskContext {
    pub(crate) fn enter(id: String) -> Self {
        Self(TASK_ID.with(|value| value.replace(Some(id))))
    }
}
impl Drop for TaskContext {
    fn drop(&mut self) {
        TASK_ID.with(|value| {
            value.replace(self.0.take());
        });
    }
}
pub(crate) fn current_task_id() -> Option<String> {
    TASK_ID.with(|value| value.borrow().clone())
}
pub(crate) fn lock_audit(key: &str, phase: &str, waited_ms: u64) -> Result<(), String> {
    crate::operation_audit_log()?.append(&mikomai_core::audit::record(
        "device_lock",
        Some(key.into()),
        mikomai_core::domain::OperationClass::ReadOnly,
        phase,
        &json!({"waited_ms":waited_ms}),
    ))
}

type Slot = Arc<Mutex<Option<tokio::sync::OwnedSemaphorePermit>>>;
thread_local! {static TASK_SLOT:std::cell::RefCell<Option<Slot>>=const{std::cell::RefCell::new(None)};}
struct TaskSlot(Option<Slot>);
impl TaskSlot {
    fn enter(slot: Slot) -> Self {
        Self(TASK_SLOT.with(|v| v.replace(Some(slot))))
    }
}
impl Drop for TaskSlot {
    fn drop(&mut self) {
        TASK_SLOT.with(|v| {
            v.replace(self.0.take());
        });
    }
}
pub(crate) fn release_task_slot() {
    TASK_SLOT.with(|v| {
        if let Some(slot) = v.borrow().as_ref() {
            slot.lock().unwrap().take();
        }
    });
}
pub(crate) async fn reclaim_task_slot(
    slot: Option<Slot>,
    cancel: &mut watch::Receiver<bool>,
) -> Result<(), String> {
    if let Some(slot) = slot {
        let permit = engine().scheduler.acquire(cancel).await?;
        *slot.lock().unwrap() = Some(permit);
    }
    Ok(())
}
pub(crate) fn current_task_slot() -> Option<Slot> {
    TASK_SLOT.with(|v| v.borrow().clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test(flavor = "multi_thread")]
    async fn lock_timeout_releases_all_slots_and_requires_explicit_resume_or_cancel() {
        let _environment = crate::tests::TEST_ENV_LOCK.lock().unwrap();
        let service = crate::shared_service();
        let previous = service.load_document("settings").unwrap();
        service
            .save_document("settings", &json!({"deviceReadLockWaitSeconds":1}))
            .unwrap();
        let engine = Arc::new(Engine::default());
        let id = uuid::Uuid::new_v4().to_string();
        let (sender, mut cancel) = watch::channel(false);
        let (resume, _) = watch::channel(0);
        engine.tasks.lock().unwrap().insert(
            id.clone(),
            Task {
                snapshot: Snapshot {
                    task_id: id.clone(),
                    state: "queued".into(),
                    seq: 0,
                    result: String::new(),
                    events: vec![],
                },
                cancel: sender,
                resume,
            },
        );
        let locks = service.device_locks.get_or_init(Default::default);
        let root = crate::portable_app_data_dir().unwrap().join("locks");
        let key = format!("device:{id}");
        let held = locks
            .acquire(&key, true, false, false, root, &mut cancel)
            .await
            .unwrap();
        let task = {
            let engine = engine.clone();
            let id = id.clone();
            let key = key.clone();
            tokio::spawn(async move {
                engine
                    .wait_device(&id, &key, false, false, &mut cancel)
                    .await
            })
        };
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while engine.query(&id).unwrap().state != "awaiting_user" {
            assert!(std::time::Instant::now() < deadline);
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        assert_eq!(engine.scheduler.0.available_permits(), 5);
        drop(held);
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        assert!(
            !task.is_finished(),
            "must not automatically resume after timeout"
        );
        engine.resume(&id).unwrap();
        assert!(task.await.unwrap().is_ok());
        let snapshot = engine.query(&id).unwrap();
        assert!(snapshot.events.iter().any(|e| e.kind == "awaiting_user"));
        assert!(snapshot.events.windows(2).all(|e| e[1].seq == e[0].seq + 1));
        service
            .save_document("settings", &previous.unwrap_or(json!({})))
            .unwrap();
    }
}
