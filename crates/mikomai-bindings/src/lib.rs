//! The sole foreign-language boundary, generated for Swift and C# from this crate.
use std::sync::Arc;
uniffi::setup_scaffolding!();
#[derive(uniffi::Record)]
pub struct LegacyResult {
    pub status: i32,
    pub text: String,
}
#[uniffi::export(callback_interface)]
pub trait LegacyListener: Send + Sync {
    fn event(&self, kind: String, text: String, done: bool);
}
struct LegacySink(Arc<dyn LegacyListener>);
impl mikomai_app::owned_bridge::LegacyListener for LegacySink {
    fn event(&self, kind: String, text: String, done: bool) {
        self.0.event(kind, text, done);
    }
}
#[uniffi::export]
pub fn legacy_invoke(
    op: String,
    args: Vec<String>,
    listener: Option<Box<dyn LegacyListener>>,
) -> LegacyResult {
    let listener = listener.map(|l| {
        Arc::new(LegacySink(Arc::from(l))) as Arc<dyn mikomai_app::owned_bridge::LegacyListener>
    });
    match mikomai_app::owned_bridge::invoke(&op, &args, listener) {
        Ok(text) => LegacyResult { status: 0, text },
        Err(text) => LegacyResult { status: 1, text },
    }
}
#[derive(Debug, uniffi::Error)]
pub enum AppError {
    Failure { message: String },
}
impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Failure { message } => write!(f, "{message}"),
        }
    }
}
impl std::error::Error for AppError {}
fn failure(message: String) -> AppError {
    AppError::Failure { message }
}
#[derive(uniffi::Enum)]
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
#[derive(uniffi::Record, Clone)]
pub struct TaskEvent {
    pub task_id: String,
    pub seq: u64,
    pub version: u32,
    pub kind: String,
    pub payload: String,
}
impl From<mikomai_app::api::TaskEvent> for TaskEvent {
    fn from(e: mikomai_app::api::TaskEvent) -> Self {
        Self {
            task_id: e.task_id,
            seq: e.seq,
            version: e.version,
            kind: e.kind,
            payload: e.payload,
        }
    }
}
#[derive(uniffi::Record)]
pub struct Snapshot {
    pub task_id: String,
    pub state: String,
    pub seq: u64,
    pub result: String,
    pub events: Vec<TaskEvent>,
}
#[derive(uniffi::Enum)]
pub enum Query {
    Task { task_id: String },
}
#[uniffi::export(callback_interface)]
pub trait EventListener: Send + Sync {
    fn on_event(&self, event: TaskEvent);
}
struct Sink(Arc<dyn EventListener>);
impl mikomai_app::api::EventListener for Sink {
    fn on_event(&self, event: mikomai_app::api::TaskEvent) {
        self.0.on_event(event.into());
    }
}
#[derive(uniffi::Object)]
pub struct MikomaiService {
    engine: Arc<mikomai_app::api::Engine>,
}
#[uniffi::export]
impl MikomaiService {
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            engine: mikomai_app::api::engine().clone(),
        })
    }
    pub fn submit(&self, command: Command) -> Result<String, AppError> {
        use mikomai_app::api::Command as C;
        let command = match command {
            Command::Chat {
                message,
                history,
                documents_dir,
                knowledge_dir,
                attachments,
                devices_json,
                agent,
            } => C::Chat {
                message,
                history,
                documents_dir,
                knowledge_dir,
                attachments,
                devices_json,
                agent,
            },
            Command::ReadDevice {
                target,
                commands,
                timeout_seconds,
            } => C::ReadDevice {
                target,
                commands,
                timeout_seconds,
            },
            Command::ExecuteApproved { plan_id, plan_hash } => {
                C::ExecuteApproved { plan_id, plan_hash }
            }
            Command::Contract { fixture_json } => C::Contract { fixture_json },
        };
        self.engine.submit(command).map_err(failure)
    }
    pub fn query(&self, query: Query) -> Result<Snapshot, AppError> {
        let Query::Task { task_id } = query;
        let s = self.engine.query(&task_id).map_err(failure)?;
        Ok(Snapshot {
            task_id: s.task_id,
            state: s.state,
            seq: s.seq,
            result: s.result,
            events: s.events.into_iter().map(Into::into).collect(),
        })
    }
    pub fn cancel(&self, task_id: String) -> Result<(), AppError> {
        self.engine.cancel(&task_id).map_err(failure)
    }
    pub fn resume(&self, task_id: String) -> Result<(), AppError> {
        self.engine.resume(&task_id).map_err(failure)
    }
    pub fn subscribe(&self, listener: Box<dyn EventListener>) {
        self.engine.subscribe(Arc::new(Sink(Arc::from(listener))));
    }
}
