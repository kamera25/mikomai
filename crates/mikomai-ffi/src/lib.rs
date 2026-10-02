use mikomai_adapters::device::JsonDeviceRegistry;
use mikomai_adapters::headless::{EchoToolExecutor, JsonTaskRepository, StdoutReporter};
use mikomai_adapters::knowledge::{KnowledgePlanner, KnowledgeStore};
use mikomai_core::application::{ChatService, TaskManager};

#[cfg(test)]
use mikomai_core::agent::registered_arp_decision;
use mikomai_core::domain::{ChangePlanner, OperationGate, OperationPlan};
use mikomai_core::port::{
    PlanDecision, PlannerPort, PortFuture, ReportEvent, ReporterPort, ToolExecutorPort, ToolResult,
};
use mikomai_core::{DispatchMode, TaskSnapshot};
use std::collections::HashMap;
use std::ffi::{c_char, CStr, CString};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, OnceLock};

#[cfg(test)]
mod approval_boundary;

use mikomai_adapters::local_llama::{infer, CANCEL_INFERENCE};

static OPERATION_PLANS: OnceLock<Mutex<HashMap<String, OperationPlan>>> = OnceLock::new();
static GENERIC_EXECUTION_CLAIMS: OnceLock<Mutex<std::collections::HashSet<String>>> =
    OnceLock::new();
static PENDING_AGENT_TASKS: OnceLock<Mutex<HashMap<uuid::Uuid, TaskSnapshot>>> = OnceLock::new();
static RAG_INGESTED_PATHS: OnceLock<Mutex<std::collections::HashSet<PathBuf>>> = OnceLock::new();
static PORTABLE_GRAPH: OnceLock<Mutex<Option<mikomai_adapters::portable_graph::PortableGraph>>> =
    OnceLock::new();
static PORTABLE_RUNTIME: OnceLock<Result<tokio::runtime::Runtime, String>> = OnceLock::new();
static OPERATION_AUDIT: OnceLock<mikomai_adapters::audit::FileAuditLog> = OnceLock::new();
static WATCH_RUNTIME: OnceLock<Mutex<Option<FfiWatchRuntime>>> = OnceLock::new();

type MikomaiWatchNotificationCallback =
    unsafe extern "C" fn(notification_json: *const c_char, context: *mut std::ffi::c_void);

struct FfiWatchRuntime {
    service: Arc<mikomai_adapters::portable_watch::PortableWatchService>,
    scheduler: Option<mikomai_adapters::portable_watch::WatchSchedulerHandle>,
    executor: Arc<FfiWatchPrimitiveExecutor>,
    sink: Arc<FfiWatchNotificationSink>,
}

fn watch_runtime() -> &'static Mutex<Option<FfiWatchRuntime>> {
    WATCH_RUNTIME.get_or_init(|| Mutex::new(None))
}

fn portable_app_data_dir() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("MIKOMAI_DATA_DIR") {
        return Ok(PathBuf::from(path));
    }
    if let Some(path) = std::env::var_os("MIKOMAI_OPERATION_PLANS_PATH") {
        return PathBuf::from(path)
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| "operation plan path has no parent directory".to_string());
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| "home directory is unavailable".to_string())?;
    Ok(home.join("Library/Application Support/MikomaiDesktopMac"))
}

fn operation_audit_log() -> Result<&'static mikomai_adapters::audit::FileAuditLog, String> {
    if let Some(log) = OPERATION_AUDIT.get() {
        return Ok(log);
    }
    let path = portable_app_data_dir()?.join("audit/operations.ndjson");
    let log = mikomai_adapters::audit::FileAuditLog::at(path);
    let _ = OPERATION_AUDIT.set(log);
    OPERATION_AUDIT
        .get()
        .ok_or_else(|| "operation audit log is unavailable".to_string())
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PersistedAgentAudit {
    snapshot: TaskSnapshot,
    events: Vec<serde_json::Value>,
    started_at: chrono::DateTime<chrono::Utc>,
    last_event_at: chrono::DateTime<chrono::Utc>,
}

fn agent_event_directory() -> Result<PathBuf, String> {
    if let Some(root) = std::env::var_os("MIKOMAI_DATA_DIR") {
        return Ok(PathBuf::from(root).join("agent-events"));
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| "home directory is unavailable".to_string())?;
    let native_path = home.join("Library/Application Support/MikomaiDesktopMac/agent-events");
    let old_path = home.join("Library/Application Support/com.mikomai.agent/agent-events");
    if old_path.is_dir() {
        std::fs::create_dir_all(&native_path)
            .map_err(|error| format!("cannot create native agent audit directory: {error}"))?;
        for entry in std::fs::read_dir(&old_path)
            .map_err(|error| format!("cannot list legacy agent audits: {error}"))?
            .flatten()
        {
            let source = entry.path();
            if !source.is_file()
                || source.extension().and_then(|extension| extension.to_str()) != Some("json")
            {
                continue;
            }
            let Some(name) = source.file_name() else {
                continue;
            };
            let destination = native_path.join(name);
            if !destination.exists() {
                std::fs::copy(&source, &destination).map_err(|error| {
                    format!(
                        "cannot import legacy agent audit {}: {error}",
                        source.display()
                    )
                })?;
            }
        }
    }
    Ok(native_path)
}

fn agent_task_path(task_id: uuid::Uuid) -> Result<PathBuf, String> {
    Ok(agent_event_directory()?.join(format!("{task_id}.json")))
}

fn legacy_task_snapshot(value: &serde_json::Value) -> Option<PersistedAgentAudit> {
    let events = value.get("events")?.as_array()?.clone();
    let task_id = events
        .iter()
        .find_map(|event| {
            if event.get("event_type")?.as_str()? == "task_started" {
                event.get("task_id").and_then(serde_json::Value::as_str)
            } else {
                None
            }
        })
        .and_then(|text| uuid::Uuid::parse_str(text).ok())?;
    let goal = events.iter().rev().find_map(|event| {
        (event.get("event_type")?.as_str()? == "goal_set")
            .then(|| {
                event
                    .get("goal")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned)
            })
            .flatten()
    })?;
    let mut snapshot = TaskSnapshot::new(goal);
    snapshot.task.id = task_id;
    for event in &events {
        let observation = if event.get("event_type").and_then(serde_json::Value::as_str)
            == Some("observation")
        {
            Some(event)
        } else if event.get("event_type").and_then(serde_json::Value::as_str) == Some("result") {
            event.get("observation")
        } else {
            None
        };
        if let Some(observation) = observation {
            let content = observation
                .get("raw")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            let source = observation.get("source");
            let target = source
                .and_then(|value| value.get("device"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned);
            let tool = source
                .and_then(|value| value.get("tool_name"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned);
            snapshot
                .evidence
                .push(mikomai_core::Evidence::from_tool(content, target, tool));
        }
    }
    let now = chrono::Utc::now();
    let started_at = events
        .iter()
        .find_map(|event| {
            (event.get("event_type")?.as_str()? == "task_started")
                .then(|| event.get("timestamp").and_then(serde_json::Value::as_str))
                .flatten()
                .and_then(|text| chrono::DateTime::parse_from_rfc3339(text).ok())
                .map(|time| time.with_timezone(&chrono::Utc))
        })
        .unwrap_or(now);
    Some(PersistedAgentAudit {
        snapshot,
        events,
        started_at,
        last_event_at: now,
    })
}

fn read_agent_audit(task_id: uuid::Uuid) -> Result<Option<PersistedAgentAudit>, String> {
    let path = agent_task_path(task_id)?;
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("cannot read agent task audit: {error}")),
    };
    if let Ok(audit) = serde_json::from_slice::<PersistedAgentAudit>(&bytes) {
        return Ok(Some(audit));
    }
    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("agent task audit is malformed: {error}"))?;
    Ok(legacy_task_snapshot(&value))
}

fn persist_agent_audit(snapshot: &TaskSnapshot, event: serde_json::Value) -> Result<(), String> {
    let path = agent_task_path(snapshot.task.id)?;
    let mut audit = read_agent_audit(snapshot.task.id)?.unwrap_or_else(|| PersistedAgentAudit {
        snapshot: snapshot.clone(),
        events: Vec::new(),
        started_at: chrono::Utc::now(),
        last_event_at: chrono::Utc::now(),
    });
    audit.snapshot = snapshot.clone();
    audit.last_event_at = chrono::Utc::now();
    audit.events.push(event);
    let parent = path
        .parent()
        .ok_or_else(|| "agent audit path has no parent".to_string())?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("cannot create agent event directory: {error}"))?;
    let temporary = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(&audit).map_err(|error| error.to_string())?;
    std::fs::write(&temporary, bytes)
        .map_err(|error| format!("cannot write agent audit: {error}"))?;
    std::fs::rename(&temporary, &path).map_err(|error| {
        let _ = std::fs::remove_file(&temporary);
        format!("cannot atomically replace agent audit: {error}")
    })
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentTaskSummary {
    task_id: uuid::Uuid,
    started_at: chrono::DateTime<chrono::Utc>,
    goal: String,
    last_event_at: chrono::DateTime<chrono::Utc>,
    event_count: usize,
    status: String,
}

fn summarize_agent_audit(audit: &PersistedAgentAudit) -> AgentTaskSummary {
    let status = match audit.snapshot.status {
        mikomai_core::TaskStatus::Pending => "pending",
        mikomai_core::TaskStatus::Running => "running",
        mikomai_core::TaskStatus::AwaitingApproval => "awaiting_approval",
        mikomai_core::TaskStatus::AwaitingInput => "awaiting_input",
        mikomai_core::TaskStatus::Completed => "completed",
        mikomai_core::TaskStatus::Failed => "failed",
        mikomai_core::TaskStatus::Unknown => "unknown",
    }
    .to_string();
    AgentTaskSummary {
        task_id: audit.snapshot.task.id,
        started_at: audit.started_at,
        goal: audit.snapshot.task.goal.clone(),
        last_event_at: audit.last_event_at,
        event_count: audit.events.len(),
        status,
    }
}

fn list_agent_audits() -> Result<Vec<PersistedAgentAudit>, String> {
    let directory = agent_event_directory()?;
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("cannot list agent task audit: {error}")),
    };
    let mut audits = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(id) = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .and_then(|stem| uuid::Uuid::parse_str(stem).ok())
        else {
            continue;
        };
        match read_agent_audit(id) {
            Ok(Some(audit)) => audits.push(audit),
            Ok(None) => {}
            Err(error) => eprintln!("skipping unreadable agent audit {id}: {error}"),
        }
    }
    audits.sort_by(|left, right| right.last_event_at.cmp(&left.last_event_at));
    Ok(audits)
}

fn resume_saved_agent_task(task_id: uuid::Uuid) -> Result<TaskSnapshot, String> {
    let mut snapshot = read_agent_audit(task_id)?
        .ok_or_else(|| "agent task audit was not found".to_string())?
        .snapshot;
    snapshot.task.id = uuid::Uuid::new_v4();
    snapshot.status = mikomai_core::TaskStatus::Pending;
    snapshot.evidence.push(mikomai_core::Evidence::from_tool(
        "Investigation resumed from its stored observations; previous operation history remains immutable.",
        None,
        Some("agent_task_resume".into()),
    ));
    Ok(snapshot)
}

fn audit_operation(plan: &OperationPlan, outcome: &str, details: &serde_json::Value) {
    let record = mikomai_core::audit::record(
        plan.tool_id.clone(),
        plan.target.clone(),
        plan.operation_class,
        outcome,
        details,
    );
    if let Err(error) = operation_audit_log().and_then(|log| log.append(&record)) {
        eprintln!("operation audit write failed ({}): {error}", plan.tool_id);
    }
}

fn portable_runtime() -> Result<&'static tokio::runtime::Runtime, String> {
    PORTABLE_RUNTIME
        .get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .map_err(|error| format!("could not initialize portable runtime: {error}"))
        })
        .as_ref()
        .map_err(Clone::clone)
}

fn portable_graph() -> Result<mikomai_adapters::portable_graph::PortableGraph, String> {
    let mut graph = PORTABLE_GRAPH
        .get_or_init(|| Mutex::new(None))
        .lock()
        .map_err(|_| "portable graph lock is poisoned".to_string())?;
    if graph.is_none() {
        let path = resolve_portable_graph_path()?;
        *graph = Some(
            portable_runtime()?
                .block_on(mikomai_adapters::portable_graph::PortableGraph::initialize_at(&path))?,
        );
    }
    graph
        .as_ref()
        .cloned()
        .ok_or_else(|| "portable graph initialization failed".to_string())
}

fn resolve_portable_graph_path() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("MIKOMAI_GRAPH_DB_PATH") {
        return Ok(PathBuf::from(path));
    }
    #[cfg(test)]
    {
        return Ok(std::env::temp_dir().join(format!("mikomai-graph-test-{}", std::process::id())));
    }
    #[cfg(not(test))]
    {
        let home = std::env::var_os("HOME").map(PathBuf::from).ok_or_else(|| {
            "graph database path is unavailable; set MIKOMAI_GRAPH_DB_PATH".to_string()
        })?;
        let legacy = home.join("Library/Application Support/com.mikomai.agent/surrealdb");
        if legacy.is_dir() {
            Ok(legacy)
        } else {
            Ok(home.join("Library/Application Support/MikomaiDesktopMac/surrealdb"))
        }
    }
}

fn pending_agent_tasks() -> &'static Mutex<HashMap<uuid::Uuid, TaskSnapshot>> {
    PENDING_AGENT_TASKS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn resume_pending_agent_task(id: uuid::Uuid, selection: String) -> Result<TaskSnapshot, String> {
    let mut task = pending_agent_tasks()
        .lock()
        .map_err(|_| "pending agent task state is unavailable".to_string())?
        .remove(&id)
        .ok_or_else(|| "agent choice session expired; please ask again".to_string())?;
    task.evidence.push(mikomai_core::Evidence::from_tool(
        format!("__USER_CHOICE__{}", selection),
        None,
        Some("user_choice".into()),
    ));
    Ok(task)
}

fn rag_ingested_paths() -> &'static Mutex<std::collections::HashSet<PathBuf>> {
    RAG_INGESTED_PATHS.get_or_init(|| Mutex::new(std::collections::HashSet::new()))
}

fn operation_plans() -> &'static Mutex<HashMap<String, OperationPlan>> {
    OPERATION_PLANS.get_or_init(|| {
        let restored = operation_plan_path()
            .ok()
            .and_then(|path| std::fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice::<Vec<OperationPlan>>(&bytes).ok())
            .unwrap_or_default()
            .into_iter()
            .map(|plan| (plan.id.to_string(), plan))
            .collect();
        Mutex::new(restored)
    })
}

fn strip_model_credentials(value: &mut serde_json::Value) {
    const SECRET_KEYS: &[&str] = &[
        "password",
        "pass",
        "secret",
        "privatekey",
        "credentials",
        "username",
        "user",
        "passphrase",
        "enablepassword",
        "token",
        "community",
        "authkey",
        "sharedsecret",
    ];
    match value {
        serde_json::Value::Object(object) => {
            object.retain(|key, _| {
                let normalized = key.to_ascii_lowercase().replace(['_', '-', ' '], "");
                !SECRET_KEYS.contains(&normalized.as_str())
            });
            for child in object.values_mut() {
                strip_model_credentials(child);
            }
        }
        serde_json::Value::Array(values) => values.iter_mut().for_each(strip_model_credentials),
        _ => {}
    }
}

fn generic_execution_claims() -> &'static Mutex<std::collections::HashSet<String>> {
    GENERIC_EXECUTION_CLAIMS.get_or_init(|| {
        let restored = operation_plan_path()
            .ok()
            .map(|path| load_generic_execution_claims(&path.with_extension("executed.json")))
            .unwrap_or_default();
        Mutex::new(restored)
    })
}

fn load_generic_execution_claims(path: &Path) -> std::collections::HashSet<String> {
    std::fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn persist_generic_execution_claims(
    claims: &std::collections::HashSet<String>,
) -> Result<(), String> {
    persist_generic_execution_claims_at(
        &operation_plan_path()?.with_extension("executed.json"),
        claims,
    )
}

fn persist_generic_execution_claims_at(
    path: &Path,
    claims: &std::collections::HashSet<String>,
) -> Result<(), String> {
    let directory = path
        .parent()
        .ok_or_else(|| "operation claim path is invalid".to_string())?;
    std::fs::create_dir_all(directory)
        .map_err(|error| format!("cannot create operation claim storage: {error}"))?;
    let temporary = path.with_extension("tmp");
    let bytes = serde_json::to_vec(claims)
        .map_err(|error| format!("cannot serialize operation claims: {error}"))?;
    std::fs::write(&temporary, bytes)
        .map_err(|error| format!("cannot persist operation claims: {error}"))?;
    std::fs::rename(&temporary, &path)
        .map_err(|error| format!("cannot replace operation claim storage: {error}"))
}

fn operation_plan_path() -> Result<PathBuf, String> {
    Ok(portable_app_data_dir()?.join("operation-plans.json"))
}

fn persist_operation_plans(plans: &HashMap<String, OperationPlan>) -> Result<(), String> {
    let path = operation_plan_path()?;
    let directory = path
        .parent()
        .ok_or_else(|| "operation plan storage path is invalid".to_string())?;
    std::fs::create_dir_all(directory)
        .map_err(|e| format!("cannot create operation plan storage: {e}"))?;
    let bytes = serde_json::to_vec_pretty(&plans.values().cloned().collect::<Vec<_>>())
        .map_err(|e| format!("cannot serialize operation plans: {e}"))?;
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, bytes)
        .map_err(|e| format!("cannot persist operation plans: {e}"))?;
    std::fs::rename(&temporary, &path)
        .map_err(|e| format!("cannot replace operation plan storage: {e}"))
}

fn validate_native_config_command(command: &str) -> Result<(), String> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return Err("Config command cannot be empty".into());
    }
    let disallowed = [
        ';', '|', '&', '$', '(', ')', '`', '>', '<', '\\', '\n', '\r', '"', '\'',
    ];
    for character in trimmed.chars() {
        if disallowed.contains(&character) {
            return Err(format!(
                "Config command contains forbidden character: '{character}'"
            ));
        }
        if !character.is_alphanumeric()
            && ![' ', '-', '_', '.', '/', ':', '?', '*', '[', ']', ','].contains(&character)
        {
            return Err(format!(
                "Config command contains unsafe character: '{character}'"
            ));
        }
    }
    Ok(())
}

/// Creates an immutable, hash-bound device change plan for the native desktop.
#[no_mangle]
pub unsafe extern "C" fn mikomai_operation_plan_create(
    target: *const c_char,
    target_snapshot_json: *const c_char,
    commands_json: *const c_char,
    rationale: *const c_char,
) -> MikomaiResult {
    if target.is_null()
        || target_snapshot_json.is_null()
        || commands_json.is_null()
        || rationale.is_null()
    {
        return error_result("operation plan fields must not be null".into());
    }
    let caught = std::panic::catch_unwind(|| {
        let target = CStr::from_ptr(target)
            .to_str()
            .map_err(|e| e.to_string())?
            .to_owned();
        if target.trim().is_empty() {
            return Err("a registered target device is required".into());
        }
        let target_snapshot: serde_json::Value = serde_json::from_str(
            CStr::from_ptr(target_snapshot_json)
                .to_str()
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| format!("invalid target snapshot: {e}"))?;
        let commands: Vec<String> = serde_json::from_str(
            CStr::from_ptr(commands_json)
                .to_str()
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| format!("invalid command list: {e}"))?;
        if commands.is_empty() || commands.iter().any(|line| line.trim().is_empty()) {
            return Err("at least one non-empty configuration command is required".into());
        }
        for command in &commands {
            validate_native_config_command(command)?;
        }
        let rationale = CStr::from_ptr(rationale)
            .to_str()
            .map_err(|e| e.to_string())?
            .to_owned();
        let plan = ChangePlanner::create(
            "network_config".into(),
            Some(target.clone()),
            serde_json::json!({"deviceName": target, "deviceSnapshot": target_snapshot, "commands": commands}),
            rationale,
        )?;
        let json = serde_json::to_string(&plan).map_err(|e| e.to_string())?;
        let mut plans = operation_plans()
            .lock()
            .map_err(|_| "operation plan state is unavailable".to_string())?;
        plans.insert(plan.id.to_string(), plan.clone());
        persist_operation_plans(&plans)?;
        audit_operation(
            &plan,
            "planned",
            &serde_json::json!({"rationale":&plan.rationale,"args":&plan.args}),
        );
        Ok(json)
    });
    match caught {
        Ok(Ok(v)) => result(0, v),
        Ok(Err(e)) => error_result(e),
        Err(_) => error_result("operation plan creation failed unexpectedly".into()),
    }
}

/// Creates an immutable plan for a non-config operation while keeping the
/// credential-bearing device snapshot outside model-controlled arguments.
#[no_mangle]
pub unsafe extern "C" fn mikomai_operation_plan_create_generic(
    target: *const c_char,
    tool_id: *const c_char,
    target_snapshot_json: *const c_char,
    args_json: *const c_char,
    rationale: *const c_char,
) -> MikomaiResult {
    if [target, tool_id, target_snapshot_json, args_json, rationale]
        .iter()
        .any(|value| value.is_null())
    {
        return error_result("operation plan fields must not be null".into());
    }
    let caught = std::panic::catch_unwind(|| {
        let text = |value: *const c_char| -> Result<String, String> {
            Ok(CStr::from_ptr(value)
                .to_str()
                .map_err(|e| e.to_string())?
                .to_owned())
        };
        let target = text(target)?;
        let tool_id = text(tool_id)?;
        let rationale = text(rationale)?;
        if target.trim().is_empty() || rationale.trim().is_empty() {
            return Err("a registered target and rationale are required".into());
        }
        if !matches!(
            tool_id.as_str(),
            "network_send_console_message"
                | "network_config"
                | "network_ftp_download"
                | "network_ftp_upload"
                | "network_tftp_download"
                | "network_tftp_upload"
        ) {
            return Err(format!(
                "generic approval plan does not allow tool `{tool_id}`"
            ));
        }
        let snapshot: serde_json::Value = serde_json::from_str(&text(target_snapshot_json)?)
            .map_err(|e| format!("invalid target snapshot: {e}"))?;
        let args: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str::<serde_json::Value>(&text(args_json)?)
                .map_err(|e| format!("invalid operation arguments: {e}"))?
                .as_object()
                .cloned()
                .ok_or_else(|| "operation arguments must be an object".to_string())?;
        if args.len() > 64 {
            return Err("operation arguments exceed the limit".into());
        }
        let mut args_value = serde_json::Value::Object(args);
        strip_model_credentials(&mut args_value);
        let mut args = args_value
            .as_object()
            .cloned()
            .ok_or_else(|| "operation arguments must be an object".to_string())?;
        args.insert(
            "deviceName".into(),
            serde_json::Value::String(target.clone()),
        );
        args.insert("deviceSnapshot".into(), snapshot);
        let args = serde_json::Value::Object(args);
        let plan = ChangePlanner::create(tool_id, Some(target), args, rationale)?;
        let json = serde_json::to_string(&plan).map_err(|e| e.to_string())?;
        let mut plans = operation_plans()
            .lock()
            .map_err(|_| "operation plan state is unavailable".to_string())?;
        plans.insert(plan.id.to_string(), plan.clone());
        persist_operation_plans(&plans)?;
        audit_operation(
            &plan,
            "planned",
            &serde_json::json!({"rationale":&plan.rationale,"args":&plan.args}),
        );
        Ok(json)
    });
    match caught {
        Ok(Ok(value)) => result(0, value),
        Ok(Err(error)) => error_result(error),
        Err(_) => error_result("generic operation plan creation failed unexpectedly".into()),
    }
}

/// Approves only the stored plan identified by its exact id and hash.
#[no_mangle]
pub unsafe extern "C" fn mikomai_operation_plan_approve(
    id: *const c_char,
    hash: *const c_char,
) -> MikomaiResult {
    if id.is_null() || hash.is_null() {
        return error_result("operation plan id and hash must not be null".into());
    }
    let caught = std::panic::catch_unwind(|| {
        let id = CStr::from_ptr(id)
            .to_str()
            .map_err(|e| e.to_string())?
            .to_owned();
        let hash = CStr::from_ptr(hash)
            .to_str()
            .map_err(|e| e.to_string())?
            .to_owned();
        let mut plans = operation_plans()
            .lock()
            .map_err(|_| "operation plan state is unavailable".to_string())?;
        let previous = plans
            .get(&id)
            .cloned()
            .ok_or_else(|| "operation plan was not found".to_string())?;
        let (output, audit_plan) = {
            let plan = plans
                .get_mut(&id)
                .ok_or_else(|| "operation plan was not found".to_string())?;
            OperationGate::approve(plan, &hash)?;
            (
                serde_json::to_string(plan).map_err(|e| e.to_string())?,
                plan.clone(),
            )
        };
        if let Err(error) = persist_operation_plans(&plans) {
            plans.insert(id, previous);
            return Err(error);
        }
        audit_operation(
            &audit_plan,
            "approved",
            &serde_json::json!({"plan_hash":audit_plan.plan_hash}),
        );
        Ok(output)
    });
    match caught {
        Ok(Ok(v)) => result(0, v),
        Ok(Err(e)) => error_result(e),
        Err(_) => error_result("operation approval failed unexpectedly".into()),
    }
}

/// Reads a stored plan by id without accepting caller-provided plan contents.
#[no_mangle]
pub unsafe extern "C" fn mikomai_operation_plan_get(id: *const c_char) -> MikomaiResult {
    if id.is_null() {
        return error_result("operation plan id must not be null".into());
    }
    let caught = std::panic::catch_unwind(|| {
        let id = CStr::from_ptr(id)
            .to_str()
            .map_err(|e| e.to_string())?
            .to_owned();
        let plans = operation_plans()
            .lock()
            .map_err(|_| "operation plan state is unavailable".to_string())?;
        let plan = plans
            .get(&id)
            .ok_or_else(|| "operation plan was not found".to_string())?;
        serde_json::to_string(plan).map_err(|e| e.to_string())
    });
    match caught {
        Ok(Ok(v)) => result(0, v),
        Ok(Err(e)) => error_result(e),
        Err(_) => error_result("operation plan lookup failed unexpectedly".into()),
    }
}

#[no_mangle]
pub extern "C" fn mikomai_operation_audit_list() -> MikomaiResult {
    match operation_audit_log()
        .and_then(|log| serde_json::to_string(&log.list(500)?).map_err(|error| error.to_string()))
    {
        Ok(value) => result(0, value),
        Err(error) => error_result(error),
    }
}

#[no_mangle]
pub extern "C" fn mikomai_agent_task_list() -> MikomaiResult {
    match list_agent_audits().and_then(|audits| {
        let summaries = audits.iter().map(summarize_agent_audit).collect::<Vec<_>>();
        serde_json::to_string(&summaries).map_err(|error| error.to_string())
    }) {
        Ok(value) => result(0, value),
        Err(error) => error_result(error),
    }
}

#[no_mangle]
pub unsafe extern "C" fn mikomai_agent_task_history(id: *const c_char) -> MikomaiResult {
    if id.is_null() {
        return error_result("agent task id must not be null".into());
    }
    let caught = std::panic::catch_unwind(|| {
        let id = CStr::from_ptr(id)
            .to_str()
            .map_err(|error| error.to_string())?;
        let id = uuid::Uuid::parse_str(id).map_err(|error| error.to_string())?;
        let audit =
            read_agent_audit(id)?.ok_or_else(|| "agent task audit was not found".to_string())?;
        serde_json::to_string(&serde_json::json!({
            "summary": summarize_agent_audit(&audit),
            "snapshot": audit.snapshot,
            "events": audit.events,
        }))
        .map_err(|error| error.to_string())
    });
    match caught {
        Ok(Ok(value)) => result(0, value),
        Ok(Err(error)) => error_result(error),
        Err(_) => error_result("agent task audit lookup failed unexpectedly".into()),
    }
}

/// Claims an approved plan once before the native runner begins dry-run.
#[no_mangle]
pub unsafe extern "C" fn mikomai_operation_plan_begin(
    id: *const c_char,
    hash: *const c_char,
) -> MikomaiResult {
    if id.is_null() || hash.is_null() {
        return error_result("operation plan id and hash must not be null".into());
    }
    let caught = std::panic::catch_unwind(|| {
        let id = CStr::from_ptr(id)
            .to_str()
            .map_err(|e| e.to_string())?
            .to_owned();
        let hash = CStr::from_ptr(hash)
            .to_str()
            .map_err(|e| e.to_string())?
            .to_owned();
        let mut plans = operation_plans()
            .lock()
            .map_err(|_| "operation plan state is unavailable".to_string())?;
        let previous = plans
            .get(&id)
            .cloned()
            .ok_or_else(|| "operation plan was not found".to_string())?;
        let (output, audit_plan) = {
            let plan = plans
                .get_mut(&id)
                .ok_or_else(|| "operation plan was not found".to_string())?;
            OperationGate::begin_execution(plan, &hash)?;
            (
                serde_json::to_string(plan).map_err(|e| e.to_string())?,
                plan.clone(),
            )
        };
        if let Err(error) = persist_operation_plans(&plans) {
            plans.insert(id, previous);
            return Err(error);
        }
        audit_operation(
            &audit_plan,
            "started",
            &serde_json::json!({"plan_hash":audit_plan.plan_hash}),
        );
        Ok(output)
    });
    match caught {
        Ok(Ok(v)) => result(0, v),
        Ok(Err(e)) => error_result(e),
        Err(_) => error_result("operation execution claim failed unexpectedly".into()),
    }
}

/// Records completion only for a plan previously claimed for execution.
#[no_mangle]
pub unsafe extern "C" fn mikomai_operation_plan_finish(
    id: *const c_char,
    succeeded: i32,
) -> MikomaiResult {
    if id.is_null() {
        return error_result("operation plan id must not be null".into());
    }
    let caught = std::panic::catch_unwind(|| {
        let id = CStr::from_ptr(id)
            .to_str()
            .map_err(|e| e.to_string())?
            .to_owned();
        let mut plans = operation_plans()
            .lock()
            .map_err(|_| "operation plan state is unavailable".to_string())?;
        let previous = plans
            .get(&id)
            .cloned()
            .ok_or_else(|| "operation plan was not found".to_string())?;
        let (output, audit_plan) = {
            let plan = plans
                .get_mut(&id)
                .ok_or_else(|| "operation plan was not found".to_string())?;
            OperationGate::finish_execution(plan, succeeded != 0)?;
            (
                serde_json::to_string(plan).map_err(|e| e.to_string())?,
                plan.clone(),
            )
        };
        if let Err(error) = persist_operation_plans(&plans) {
            plans.insert(id, previous);
            return Err(error);
        }
        audit_operation(
            &audit_plan,
            if succeeded != 0 { "success" } else { "failed" },
            &serde_json::json!({"plan_hash":audit_plan.plan_hash}),
        );
        Ok(output)
    });
    match caught {
        Ok(Ok(v)) => result(0, v),
        Ok(Err(e)) => error_result(e),
        Err(_) => error_result("operation completion update failed unexpectedly".into()),
    }
}

/// Executes only a persisted plan that Swift has already hash-approved and
/// claimed. Keychain secrets arrive for this single call and are never saved.
#[no_mangle]
pub unsafe extern "C" fn mikomai_operation_execute_approved(
    id: *const c_char,
    hash: *const c_char,
    credentials_json: *const c_char,
) -> MikomaiResult {
    if id.is_null() || hash.is_null() || credentials_json.is_null() {
        return error_result("operation id, hash, and credential payload are required".into());
    }
    let caught = std::panic::catch_unwind(|| {
        let id = CStr::from_ptr(id)
            .to_str()
            .map_err(|e| e.to_string())?
            .to_owned();
        let hash = CStr::from_ptr(hash)
            .to_str()
            .map_err(|e| e.to_string())?
            .to_owned();
        let credentials: serde_json::Value = serde_json::from_str(
            CStr::from_ptr(credentials_json)
                .to_str()
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| format!("invalid ephemeral credential payload: {e}"))?;
        let plan = {
            let plans = operation_plans()
                .lock()
                .map_err(|_| "operation plan state is unavailable".to_string())?;
            let plan = plans
                .get(&id)
                .cloned()
                .ok_or_else(|| "operation plan was not found".to_string())?;
            if plan.tool_id == "network_config" {
                return Err(
                    "network_config must use the native dry-run/config/post-verify execution flow"
                        .into(),
                );
            }
            if plan.status != mikomai_core::OperationStatus::Executing {
                return Err("approved operation has not been claimed for execution".into());
            }
            OperationGate::authorize(&plan, Some(&hash))?;
            let mut claims = generic_execution_claims()
                .lock()
                .map_err(|_| "operation execution state is unavailable".to_string())?;
            if !claims.insert(id.clone()) {
                return Err("approved operation has already been executed or claimed".into());
            }
            if let Err(error) = persist_generic_execution_claims(&claims) {
                claims.remove(&id);
                return Err(error);
            }
            plan
        };
        let snapshot = plan
            .args
            .get("deviceSnapshot")
            .ok_or_else(|| "operation plan has no registered device snapshot".to_string())?;
        let args = &plan.args;
        let output = if plan.tool_id == "network_send_console_message" {
            let port_path = args
                .get("port")
                .or_else(|| args.get("port_path"))
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| "approved plan has no serial port".to_string())?;
            let message = args
                .get("message")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| "approved plan has no console message".to_string())?;
            let request = mikomai_adapters::transfer::SerialConsoleRequest {
                port_path: port_path.to_string(),
                message: message.to_string(),
                baud_rate: args
                    .get("baud_rate")
                    .or_else(|| args.get("baudRate"))
                    .and_then(serde_json::Value::as_u64)
                    .and_then(|value| u32::try_from(value).ok()),
                timeout_ms: args
                    .get("timeout_ms")
                    .or_else(|| args.get("timeoutMs"))
                    .and_then(serde_json::Value::as_u64),
            };
            let result = mikomai_adapters::transfer::send_serial_console(request)?;
            serde_json::to_string(&result).map_err(|e| e.to_string())?
        } else {
            use mikomai_adapters::transfer::{
                TransferDirection, TransferProtocol, TransferRequest,
            };
            let protocol = if plan.tool_id.starts_with("network_ftp_") {
                TransferProtocol::Ftp
            } else {
                TransferProtocol::Tftp
            };
            let direction = if plan.tool_id.ends_with("_upload") {
                TransferDirection::Upload
            } else {
                TransferDirection::Download
            };
            let host = snapshot
                .get("host")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| {
                    "registered host is missing from the operation snapshot".to_string()
                })?
                .to_string();
            let remote_path = args
                .get("remote_path")
                .or_else(|| args.get("remote_file"))
                .or_else(|| args.get("remotePath"))
                .or_else(|| args.get("filename"))
                .or_else(|| args.get("file_name"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(|| {
                    if direction == TransferDirection::Upload {
                        "upload.txt".to_string()
                    } else {
                        "download.bin".to_string()
                    }
                });
            let local_path = args
                .get("local_path")
                .or_else(|| args.get("local_file"))
                .or_else(|| args.get("localPath"))
                .or_else(|| args.get("path"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(|| {
                    let base = std::env::var_os("HOME")
                        .map(std::path::PathBuf::from)
                        .unwrap_or_else(std::env::temp_dir);
                    base.join("Library/Application Support/MikomaiDesktopMac/artifacts")
                        .join(format!(
                            "{}-{}",
                            id,
                            std::path::Path::new(&remote_path)
                                .file_name()
                                .and_then(|name| name.to_str())
                                .unwrap_or("download.bin")
                        ))
                        .to_string_lossy()
                        .into_owned()
                });
            let content = args
                .get("content")
                .and_then(serde_json::Value::as_str)
                .map(|text| text.as_bytes().to_vec());
            let credentials = mikomai_adapters::portable_device::DeviceCredentials {
                username: snapshot
                    .get("username")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("anonymous")
                    .to_string(),
                password: credentials
                    .get("password")
                    .and_then(serde_json::Value::as_str)
                    .map(|password| password.to_owned()),
                enable_password: None,
                private_key: None,
                passphrase: None,
            };
            let port = args
                .get("port")
                .and_then(serde_json::Value::as_u64)
                .and_then(|value| u16::try_from(value).ok())
                .unwrap_or(if protocol == TransferProtocol::Ftp {
                    21
                } else {
                    69
                });
            let request = TransferRequest {
                host,
                port,
                protocol,
                direction,
                username: (protocol == TransferProtocol::Ftp).then(|| credentials.username.clone()),
                password: (protocol == TransferProtocol::Ftp)
                    .then(|| credentials.password.clone())
                    .flatten(),
                remote_path,
                local_path,
                content,
                mode: args
                    .get("mode")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned),
                timeout_secs: args
                    .get("timeout_secs")
                    .or_else(|| args.get("timeoutSecs"))
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(if protocol == TransferProtocol::Ftp {
                        15
                    } else {
                        3
                    }),
            };
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("could not initialize file-transfer runtime: {e}"))?;
            let result =
                runtime.block_on(mikomai_adapters::transfer::execute_file_transfer(request))?;
            serde_json::to_string(&result).map_err(|e| e.to_string())?
        };
        Ok(output)
    });
    match caught {
        Ok(Ok(output)) => result(0, output),
        Ok(Err(error)) => error_result(error),
        Err(_) => error_result("approved operation failed unexpectedly".into()),
    }
}

#[no_mangle]
pub unsafe extern "C" fn mikomai_set_inference_params(
    temperature: f32,
    repetition_penalty: f32,
    n_ctx: u32,
    max_new_tokens: u32,
) -> MikomaiResult {
    let caught = std::panic::catch_unwind(|| {
        mikomai_adapters::local_llama::set_params(
            temperature,
            repetition_penalty,
            n_ctx,
            max_new_tokens,
        )
    });
    match caught {
        Ok(Ok(val)) => result(0, val),
        Ok(Err(err)) => error_result(err),
        Err(_) => error_result("failed to set inference params".into()),
    }
}

fn expand_tilde(path: &Path) -> PathBuf {
    if let Ok(stripped) = path.strip_prefix("~") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(stripped);
        }
    }
    path.to_path_buf()
}

#[no_mangle]
pub unsafe extern "C" fn mikomai_model_load(path: *const c_char) -> MikomaiResult {
    if path.is_null() {
        return error_result("model path must not be null".into());
    }
    let caught = std::panic::catch_unwind(|| {
        let raw_str = CStr::from_ptr(path).to_str().map_err(|e| e.to_string())?;
        let path = expand_tilde(Path::new(raw_str));
        mikomai_adapters::local_llama::load(&path)
    });
    match caught {
        Ok(Ok(value)) => result(0, value),
        Ok(Err(error)) => error_result(error),
        Err(_) => error_result("model load failed unexpectedly".into()),
    }
}

#[no_mangle]
pub unsafe extern "C" fn mikomai_model_status() -> MikomaiResult {
    match mikomai_adapters::local_llama::status() {
        Ok(path) => result(0, path),
        Err(error) => error_result(error),
    }
}

#[no_mangle]
pub unsafe extern "C" fn mikomai_model_cancel() -> MikomaiResult {
    CANCEL_INFERENCE.store(true, Ordering::Relaxed);
    result(0, "生成を停止しています".into())
}

/// Reads only non-secret device metadata from a selected Tauri connections.json file.
#[no_mangle]
pub unsafe extern "C" fn mikomai_device_registry_read(path: *const c_char) -> MikomaiResult {
    if path.is_null() {
        return error_result("device registry path must not be null".into());
    }
    let caught = std::panic::catch_unwind(|| {
        let path = PathBuf::from(CStr::from_ptr(path).to_str().map_err(|e| e.to_string())?);
        let metadata = std::fs::metadata(&path)
            .map_err(|e| format!("failed to read device registry metadata: {e}"))?;
        if metadata.len() > 2 * 1024 * 1024 {
            return Err("機器情報ファイルは2 MiB以下にしてください。".to_string());
        }
        let devices = JsonDeviceRegistry::at(path).list()?;
        serde_json::to_string(&devices)
            .map_err(|e| format!("failed to encode device metadata: {e}"))
    });
    match caught {
        Ok(Ok(value)) => result(0, value),
        Ok(Err(error)) => error_result(error),
        Err(_) => error_result("device registry import failed unexpectedly".into()),
    }
}

#[no_mangle]
pub unsafe extern "C" fn mikomai_model_chat(prompt: *const c_char) -> MikomaiResult {
    if prompt.is_null() {
        return error_result("chat prompt must not be null".into());
    }
    let caught = std::panic::catch_unwind(|| {
        let prompt = CStr::from_ptr(prompt).to_str().map_err(|e| e.to_string())?;
        CANCEL_INFERENCE.store(false, Ordering::Relaxed);
        infer(prompt)
    });
    match caught {
        Ok(Ok(answer)) => result(0, answer),
        Ok(Err(error)) => error_result(error),
        Err(_) => error_result("model inference failed unexpectedly".into()),
    }
}

#[no_mangle]
pub unsafe extern "C" fn mikomai_assistant_chat(
    message: *const c_char,
    history: *const c_char,
    documents_dir: *const c_char,
    knowledge_dir: *const c_char,
) -> MikomaiResult {
    mikomai_assistant_chat_with_attachments(
        message,
        history,
        documents_dir,
        knowledge_dir,
        std::ptr::null(),
    )
}

pub type MikomaiStreamCallback =
    unsafe extern "C" fn(chunk: *const c_char, is_done: i32, context: *mut std::ffi::c_void);
pub type MikomaiToolCallback = unsafe extern "C" fn(
    tool_id: *const c_char,
    target_json: *const c_char,
    args_json: *const c_char,
    output: *mut c_char,
    output_capacity: usize,
    context: *mut std::ffi::c_void,
) -> i32;
pub type MikomaiPlanCallback = unsafe extern "C" fn(
    target: *const c_char,
    tool_id: *const c_char,
    args_json: *const c_char,
    rationale: *const c_char,
    output: *mut c_char,
    output_capacity: usize,
    context: *mut std::ffi::c_void,
) -> i32;

/// Routes a request using the same portable dispatch policy as the desktop
/// planner. `devices_json` contains public device metadata only.
#[no_mangle]
pub unsafe extern "C" fn mikomai_dispatch_mode(
    message: *const c_char,
    devices_json: *const c_char,
) -> MikomaiResult {
    if message.is_null() || devices_json.is_null() {
        return error_result("message and device metadata must not be null".into());
    }
    let caught = std::panic::catch_unwind(|| {
        let message = CStr::from_ptr(message)
            .to_str()
            .map_err(|e| e.to_string())?;
        let devices: Vec<mikomai_adapters::portable_device::RegisteredDevice> =
            serde_json::from_str(
                CStr::from_ptr(devices_json)
                    .to_str()
                    .map_err(|e| e.to_string())?,
            )
            .map_err(|e| format!("invalid device metadata: {e}"))?;
        let names = devices
            .into_iter()
            .flat_map(|device| [device.hostname, device.ip.unwrap_or_default()])
            .collect::<Vec<_>>();
        let mode = mikomai_core::dispatch::select_dispatch_mode_for_devices(message, &names);
        Ok(match mode {
            DispatchMode::Agent => "agent",
            DispatchMode::Worker => "worker",
        }
        .to_string())
    });
    match caught {
        Ok(Ok(value)) => result(0, value),
        Ok(Err(error)) => error_result(error),
        Err(_) => error_result("dispatch classification failed unexpectedly".into()),
    }
}

struct FfiAgentPlanner {
    inventory: Vec<mikomai_adapters::portable_device::RegisteredDevice>,
    devices: Vec<String>,
    tools: Vec<String>,
    history: String,
    attachments: String,
    reference_material: String,
    plan_callback: Option<MikomaiPlanCallback>,
    callback_context: usize,
}

impl PlannerPort for FfiAgentPlanner {
    fn plan<'a>(&'a self, task: &'a TaskSnapshot) -> PortFuture<'a, PlanDecision> {
        self.plan_with_cancellation(task, CANCEL_INFERENCE.load(Ordering::Relaxed))
    }
}
impl FfiAgentPlanner {
    fn plan_with_cancellation<'a>(
        &'a self,
        task: &'a TaskSnapshot,
        cancelled: bool,
    ) -> PortFuture<'a, PlanDecision> {
        Box::pin(async move {
            let inference = mikomai_adapters::inference::FnInference(infer);
            let worker = mikomai_adapters::inference::FnInference(|question: &str| {
                let context = mikomai_core::response::ResponseContext {
                    question,
                    history: &self.history,
                    references: &self.reference_material,
                    attachments: &self.attachments,
                };
                futures_lite::future::block_on(context.answer(&inference))
            });
            let approval = SwiftPlanTransport {
                callback: self.plan_callback,
                context: self.callback_context,
            };
            let planner = mikomai_core::agent::AgentPlanner {
                inventory: &self.inventory,
                devices: &self.devices,
                tools: &self.tools,
                history: &self.history,
                attachments: &self.attachments,
                reference_material: &self.reference_material,
                inference: &inference,
                worker: &worker,
                approval: &approval,
            };
            planner.plan_with_cancellation(task, cancelled).await
        })
    }
}
struct SwiftPlanTransport {
    callback: Option<MikomaiPlanCallback>,
    context: usize,
}
impl mikomai_core::port::OperationProposalPort for SwiftPlanTransport {
    fn propose<'a>(
        &'a self,
        target: &'a str,
        tool_id: &'a str,
        args: &'a serde_json::Value,
        rationale: &'a str,
    ) -> PortFuture<'a, OperationPlan> {
        Box::pin(async move {
            let callback = self
                .callback
                .ok_or_else(|| "native operation approval callback is unavailable".to_string())?;
            let target_c = CString::new(target).map_err(|error| error.to_string())?;
            let tool_c = CString::new(tool_id).map_err(|error| error.to_string())?;
            let args_c =
                CString::new(serde_json::to_string(&args).map_err(|error| error.to_string())?)
                    .map_err(|error| error.to_string())?;
            let rationale_c = CString::new(rationale).map_err(|error| error.to_string())?;
            let mut output = vec![0_i8; 256 * 1024];
            let status = unsafe {
                callback(
                    target_c.as_ptr(),
                    tool_c.as_ptr(),
                    args_c.as_ptr(),
                    rationale_c.as_ptr(),
                    output.as_mut_ptr(),
                    output.len(),
                    self.context as *mut std::ffi::c_void,
                )
            };
            let end = output
                .iter()
                .position(|byte| *byte == 0)
                .ok_or("native plan response is not NUL-terminated")?;
            let bytes = output[..end]
                .iter()
                .map(|byte| *byte as u8)
                .collect::<Vec<_>>();
            let plan_json = String::from_utf8(bytes)
                .map_err(|e| format!("native plan response is not UTF-8: {e}"))?;
            if status != 0 {
                return Err(if plan_json.is_empty() {
                    "Swift could not create the approved operation plan".into()
                } else {
                    plan_json
                });
            }
            let plan: OperationPlan = serde_json::from_str(&plan_json)
                .map_err(|error| format!("native operation plan response was invalid: {error}"))?;
            Ok(plan)
        })
    }
}

struct SwiftCallbackTransport {
    callback: MikomaiToolCallback,
    context: usize,
}

impl mikomai_adapters::portable_device::CredentialedReadOnlyTransport for SwiftCallbackTransport {
    fn execute_read_only(
        &self,
        target: &mikomai_adapters::portable_device::RegisteredDevice,
        _credentials: &mikomai_adapters::portable_device::DeviceCredentials,
        tool: mikomai_adapters::portable_device::ReadOnlyDeviceTool,
        args: &serde_json::Value,
    ) -> Result<String, String> {
        let tool = CString::new(tool.as_str()).unwrap();
        let target =
            CString::new(serde_json::to_string(target).map_err(|e| e.to_string())?).unwrap();
        let args = CString::new(serde_json::to_string(args).map_err(|e| e.to_string())?).unwrap();
        let mut output = vec![0_i8; 256 * 1024];
        let status = unsafe {
            (self.callback)(
                tool.as_ptr(),
                target.as_ptr(),
                args.as_ptr(),
                output.as_mut_ptr(),
                output.len(),
                self.context as *mut std::ffi::c_void,
            )
        };
        let text = unsafe {
            CStr::from_ptr(output.as_ptr())
                .to_string_lossy()
                .into_owned()
        };
        if status == 0 {
            Ok(text)
        } else {
            Err(if text.is_empty() {
                "Swift network tool failed".into()
            } else {
                text
            })
        }
    }
}

struct FfiWatchPrimitiveExecutor {
    callback: MikomaiToolCallback,
    context: usize,
}

impl mikomai_adapters::portable_watch::WatchPrimitiveExecutor for FfiWatchPrimitiveExecutor {
    fn execute<'a>(
        &'a self,
        call: &'a mikomai_adapters::portable_watch::CallStep,
    ) -> mikomai_adapters::portable_watch::WatchFuture<'a, Result<serde_json::Value, String>> {
        Box::pin(async move {
            let tool_id = CString::new("get_state").unwrap();
            let target = serde_json::json!({"hostname": call.args.device});
            let target = CString::new(target.to_string()).map_err(|error| error.to_string())?;
            let args = CString::new(serde_json::json!({"resource":"cpu"}).to_string())
                .map_err(|error| error.to_string())?;
            let mut output = vec![0_i8; 16 * 1024];
            let status = unsafe {
                (self.callback)(
                    tool_id.as_ptr(),
                    target.as_ptr(),
                    args.as_ptr(),
                    output.as_mut_ptr(),
                    output.len(),
                    self.context as *mut std::ffi::c_void,
                )
            };
            let response = unsafe { CStr::from_ptr(output.as_ptr()) }
                .to_string_lossy()
                .into_owned();
            if status != 0 {
                return Err(if response.trim().is_empty() {
                    "CPU watch probe failed in the Swift transport".into()
                } else {
                    response
                });
            }
            let response: serde_json::Value = serde_json::from_str(&response)
                .map_err(|error| format!("CPU watch callback returned invalid JSON: {error}"))?;
            if response.get("success").and_then(serde_json::Value::as_bool) != Some(true) {
                return Err(response
                    .get("output")
                    .and_then(serde_json::Value::as_str)
                    .filter(|output| !output.trim().is_empty())
                    .unwrap_or("CPU watch probe failed")
                    .to_string());
            }
            let output = response
                .get("output")
                .and_then(serde_json::Value::as_str)
                .filter(|output| !output.trim().is_empty())
                .ok_or_else(|| "CPU watch probe returned an empty output".to_string())?;
            let output: serde_json::Value = serde_json::from_str(output)
                .map_err(|error| format!("CPU watch probe returned invalid usage JSON: {error}"))?;
            let usage = output
                .get("usage")
                .and_then(serde_json::Value::as_f64)
                .filter(|usage| usage.is_finite() && (0.0..=100.0).contains(usage))
                .ok_or_else(|| {
                    "CPU watch probe did not return numeric usage in 0..=100".to_string()
                })?;
            Ok(serde_json::json!({"usage":usage}))
        })
    }
}

struct FfiWatchNotificationSink {
    callback: MikomaiWatchNotificationCallback,
    context: usize,
}

impl mikomai_adapters::portable_watch::WatchNotificationSink for FfiWatchNotificationSink {
    fn notify(
        &self,
        notification: &mikomai_adapters::portable_watch::WatchNotification,
    ) -> Result<(), String> {
        let payload =
            CString::new(serde_json::to_string(notification).map_err(|error| error.to_string())?)
                .map_err(|error| error.to_string())?;
        unsafe {
            (self.callback)(payload.as_ptr(), self.context as *mut std::ffi::c_void);
        }
        Ok(())
    }
}

fn active_watch_service(
) -> Result<Arc<mikomai_adapters::portable_watch::PortableWatchService>, String> {
    watch_runtime()
        .lock()
        .map_err(|_| "Watch runtime state is unavailable".to_string())?
        .as_ref()
        .map(|runtime| runtime.service.clone())
        .ok_or_else(|| "Watch scheduler has not been started".to_string())
}

#[no_mangle]
pub unsafe extern "C" fn mikomai_watch_start(
    storage_path: *const c_char,
    tool_callback: Option<MikomaiToolCallback>,
    notification_callback: Option<MikomaiWatchNotificationCallback>,
    context: *mut std::ffi::c_void,
) -> MikomaiResult {
    if storage_path.is_null() || tool_callback.is_none() || notification_callback.is_none() {
        return error_result("Watch storage path and both native callbacks are required".into());
    }
    let caught = std::panic::catch_unwind(|| {
        let path = PathBuf::from(
            CStr::from_ptr(storage_path)
                .to_str()
                .map_err(|error| error.to_string())?,
        );
        let mut state = watch_runtime()
            .lock()
            .map_err(|_| "Watch runtime state is unavailable".to_string())?;
        if state.is_some() {
            return Err("Watch scheduler is already started".into());
        }
        let service = Arc::new(mikomai_adapters::portable_watch::PortableWatchService::at(
            &path,
        )?);
        let executor = Arc::new(FfiWatchPrimitiveExecutor {
            callback: tool_callback.unwrap(),
            context: context as usize,
        });
        let sink = Arc::new(FfiWatchNotificationSink {
            callback: notification_callback.unwrap(),
            context: context as usize,
        });
        let scheduler = portable_runtime()?.block_on(async {
            service.clone().start(
                executor.clone(),
                sink.clone(),
                std::time::Duration::from_secs(1),
            )
        })?;
        *state = Some(FfiWatchRuntime {
            service,
            scheduler: Some(scheduler),
            executor,
            sink,
        });
        Ok("Watch scheduler started".to_string())
    });
    match caught {
        Ok(Ok(message)) => result(0, message),
        Ok(Err(error)) => error_result(error),
        Err(_) => error_result("Watch scheduler start failed unexpectedly".into()),
    }
}

#[no_mangle]
pub extern "C" fn mikomai_watch_stop() -> MikomaiResult {
    let caught = std::panic::catch_unwind(|| {
        let state = watch_runtime()
            .lock()
            .map_err(|_| "Watch runtime state is unavailable".to_string())?
            .take();
        if let Some(mut state) = state {
            if let Some(scheduler) = state.scheduler.take() {
                portable_runtime()?.block_on(scheduler.stop())?;
            }
        }
        Ok("Watch scheduler stopped".to_string())
    });
    match caught {
        Ok(Ok(message)) => result(0, message),
        Ok(Err(error)) => error_result(error),
        Err(_) => error_result("Watch scheduler stop failed unexpectedly".into()),
    }
}

#[no_mangle]
pub extern "C" fn mikomai_watch_list() -> MikomaiResult {
    match active_watch_service().and_then(|service| {
        serde_json::to_string(&service.list()?).map_err(|error| error.to_string())
    }) {
        Ok(value) => result(0, value),
        Err(error) => error_result(error),
    }
}

#[no_mangle]
pub unsafe extern "C" fn mikomai_watch_create(request_json: *const c_char) -> MikomaiResult {
    if request_json.is_null() {
        return error_result("Watch request must not be null".into());
    }
    let caught = std::panic::catch_unwind(|| {
        let request: mikomai_adapters::portable_watch::CreateWatchRequest = serde_json::from_str(
            CStr::from_ptr(request_json)
                .to_str()
                .map_err(|error| error.to_string())?,
        )
        .map_err(|error| format!("invalid Watch request: {error}"))?;
        let watch = active_watch_service()?.create(request)?;
        serde_json::to_string(&watch).map_err(|error| error.to_string())
    });
    match caught {
        Ok(Ok(value)) => result(0, value),
        Ok(Err(error)) => error_result(error),
        Err(_) => error_result("Watch creation failed unexpectedly".into()),
    }
}

#[no_mangle]
pub unsafe extern "C" fn mikomai_watch_update(
    id: *const c_char,
    request_json: *const c_char,
) -> MikomaiResult {
    if id.is_null() || request_json.is_null() {
        return error_result("Watch id and request are required".into());
    }
    let caught = std::panic::catch_unwind(|| {
        let id = CStr::from_ptr(id)
            .to_str()
            .map_err(|error| error.to_string())?;
        let id = uuid::Uuid::parse_str(id).map_err(|error| error.to_string())?;
        let request: mikomai_adapters::portable_watch::UpdateWatchRequest = serde_json::from_str(
            CStr::from_ptr(request_json)
                .to_str()
                .map_err(|error| error.to_string())?,
        )
        .map_err(|error| format!("invalid Watch request: {error}"))?;
        let watch = active_watch_service()?.update(id, request)?;
        serde_json::to_string(&watch).map_err(|error| error.to_string())
    });
    match caught {
        Ok(Ok(value)) => result(0, value),
        Ok(Err(error)) => error_result(error),
        Err(_) => error_result("Watch update failed unexpectedly".into()),
    }
}

fn set_watch_status(id: *const c_char, enabled: bool) -> MikomaiResult {
    if id.is_null() {
        return error_result("Watch id must not be null".into());
    }
    let caught = std::panic::catch_unwind(|| {
        let id = unsafe { CStr::from_ptr(id) }
            .to_str()
            .map_err(|error| error.to_string())?;
        let id = uuid::Uuid::parse_str(id).map_err(|error| error.to_string())?;
        let service = active_watch_service()?;
        let watch = if enabled {
            service.enable(id)?
        } else {
            service.disable(id)?
        };
        serde_json::to_string(&watch).map_err(|error| error.to_string())
    });
    match caught {
        Ok(Ok(value)) => result(0, value),
        Ok(Err(error)) => error_result(error),
        Err(_) => error_result("Watch status update failed unexpectedly".into()),
    }
}

#[no_mangle]
pub unsafe extern "C" fn mikomai_watch_enable(id: *const c_char) -> MikomaiResult {
    set_watch_status(id, true)
}

#[no_mangle]
pub unsafe extern "C" fn mikomai_watch_disable(id: *const c_char) -> MikomaiResult {
    set_watch_status(id, false)
}

#[no_mangle]
pub unsafe extern "C" fn mikomai_watch_delete(id: *const c_char) -> MikomaiResult {
    if id.is_null() {
        return error_result("Watch id must not be null".into());
    }
    let caught = std::panic::catch_unwind(|| {
        let id = CStr::from_ptr(id)
            .to_str()
            .map_err(|error| error.to_string())?;
        let id = uuid::Uuid::parse_str(id).map_err(|error| error.to_string())?;
        active_watch_service()?.delete(id)?;
        Ok("Watch deleted".to_string())
    });
    match caught {
        Ok(Ok(value)) => result(0, value),
        Ok(Err(error)) => error_result(error),
        Err(_) => error_result("Watch deletion failed unexpectedly".into()),
    }
}

#[no_mangle]
pub unsafe extern "C" fn mikomai_watch_run_now(id: *const c_char) -> MikomaiResult {
    if id.is_null() {
        return error_result("Watch id must not be null".into());
    }
    let caught = std::panic::catch_unwind(|| {
        let id = CStr::from_ptr(id)
            .to_str()
            .map_err(|error| error.to_string())?;
        let id = uuid::Uuid::parse_str(id).map_err(|error| error.to_string())?;
        let (service, executor, sink) = {
            let state = watch_runtime()
                .lock()
                .map_err(|_| "Watch runtime state is unavailable".to_string())?;
            let state = state
                .as_ref()
                .ok_or_else(|| "Watch scheduler has not been started".to_string())?;
            (
                state.service.clone(),
                state.executor.clone(),
                state.sink.clone(),
            )
        };
        let record =
            portable_runtime()?.block_on(service.run_now(id, executor.as_ref(), sink.as_ref()))?;
        serde_json::to_string(&record).map_err(|error| error.to_string())
    });
    match caught {
        Ok(Ok(value)) => result(0, value),
        Ok(Err(error)) => error_result(error),
        Err(_) => error_result("Watch execution failed unexpectedly".into()),
    }
}

struct FfiAgentExecutor {
    registry: mikomai_adapters::portable_device::ReadOnlyToolRegistry,
    transport: SwiftCallbackTransport,
    graph: mikomai_adapters::portable_graph::PortableGraph,
    rag: mikomai_adapters::portable_rag::PortableRag,
    rag_sources: Vec<PathBuf>,
}

impl ToolExecutorPort for FfiAgentExecutor {
    fn execute<'a>(
        &'a self,
        _task_id: uuid::Uuid,
        tool: &'a str,
        target: Option<&'a str>,
        args: &'a serde_json::Value,
    ) -> PortFuture<'a, ToolResult> {
        Box::pin(async move {
            if CANCEL_INFERENCE.load(Ordering::Relaxed) {
                return Err("生成を停止しました。".into());
            }
            if tool == "get_state"
                && target == Some("localhost")
                && args.get("resource").and_then(serde_json::Value::as_str) == Some("arp")
            {
                let local = mikomai_adapters::portable_device::RegisteredDevice {
                    id: None,
                    hostname: "localhost".into(),
                    ip: None,
                    device_type: Some("local".into()),
                };
                let credentials = mikomai_adapters::portable_device::DeviceCredentials {
                    username: String::new(),
                    password: None,
                    enable_password: None,
                    private_key: None,
                    passphrase: None,
                };
                let raw = mikomai_adapters::portable_device::CredentialedReadOnlyTransport::execute_read_only(&self.transport, &local, &credentials, mikomai_adapters::portable_device::ReadOnlyDeviceTool::GetState, args)?;
                let tool_result = serde_json::from_str::<ToolResult>(&raw).unwrap_or(ToolResult {
                    success: true,
                    output: raw,
                });
                return Ok(ToolResult {
                    success: tool_result.success,
                    output: mikomai_core::redaction::redact_network_secrets(&tool_result.output),
                });
            }
            if tool == "self_network_nwdiag" {
                let schema = args
                    .get("schema")
                    .or_else(|| args.get("nwdiag"))
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| "nwdiag schema is required".to_string())?;
                mikomai_core::nwdiag::validate_nwdiag_schema(schema)
                    .map_err(|error| error.to_llm_feedback_string())?;
            }
            if matches!(tool, "network_packet_analyze" | "network_packet_prepare") {
                if tool == "network_packet_prepare" {
                    let request: mikomai_core::network::packet::DhcpRequestPreviewInput =
                        serde_json::from_value(args.clone()).map_err(|error| error.to_string())?;
                    return Ok(ToolResult {
                        success: true,
                        output: mikomai_core::network::packet::prepare_dhcp_request_preview(
                            request,
                        )?,
                    });
                }
                let frame = args
                    .get("frame_hex")
                    .or_else(|| args.get("frameHex"))
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| "network_packet_analyze requires frame_hex".to_string())?;
                return Ok(ToolResult {
                    success: true,
                    output: mikomai_core::network::packet::analyze_ethernet_frame_hex(frame)?,
                });
            }
            if tool == "network_packet_safety" {
                let request: mikomai_core::network::packet::PacketSafetyRequest =
                    serde_json::from_value(args.clone()).map_err(|error| error.to_string())?;
                let outcome = mikomai_core::network::packet::run_packet_safety(request);
                let success = !matches!(
                    outcome,
                    mikomai_core::network::packet::PacketSafetyOutcome::Failed { .. }
                );
                return Ok(ToolResult {
                    success,
                    output: serde_json::to_string(&outcome).map_err(|error| error.to_string())?,
                });
            }
            if tool == "require_host_registered" {
                return Ok(mikomai_adapters::portable_graph::require_host_registered());
            }
            if matches!(
                tool,
                "ask_user_choice" | "ask_interface_choice" | "ask_ipaddress_choice"
            ) {
                let question = args
                    .get("question")
                    .or_else(|| args.get("message"))
                    .or_else(|| args.get("prompt"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("選択内容を指定してください。")
                    .trim();
                if question.is_empty() || question.len() > 2_000 {
                    return Err("choice question must contain 1–2,000 characters".into());
                }
                let options = args
                    .get("options")
                    .and_then(serde_json::Value::as_array)
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(serde_json::Value::as_str)
                            .map(str::to_owned)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                if options.len() > 64 || options.iter().any(|item| item.len() > 256) {
                    return Err("choice options exceed the portable limits".into());
                }
                let label = match tool {
                    "ask_interface_choice" => "インターフェース選択",
                    "ask_ipaddress_choice" => "IPアドレス選択",
                    _ => "選択",
                };
                return Ok(ToolResult {
                    success: true,
                    output: format!(
                        "__ASK_HUMAN__{}",
                        serde_json::json!({"title":label,"question":question,"options":options})
                    ),
                });
            }
            if tool == "get_operation_plan" {
                let id = args
                    .get("id")
                    .or_else(|| args.get("plan_id"))
                    .or_else(|| args.get("planId"))
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| "operation plan id is required".to_string())?;
                let plans = operation_plans()
                    .lock()
                    .map_err(|_| "operation plan store lock is poisoned".to_string())?;
                let Some(plan) = plans.get(id) else {
                    return Ok(ToolResult {
                        success: false,
                        output: format!("operation plan `{id}` was not found"),
                    });
                };
                return Ok(ToolResult {
                    success: true,
                    output: serde_json::to_string(plan).map_err(|e| e.to_string())?,
                });
            }
            if tool == "query_network_graph" {
                use mikomai_adapters::portable_graph::GraphQuery;
                let query = GraphQuery {
                    query: args
                        .get("query")
                        .or_else(|| args.get("userMessage"))
                        .or_else(|| args.get("user_message"))
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    device_name: args
                        .get("deviceName")
                        .or_else(|| args.get("device_name"))
                        .or_else(|| args.get("device"))
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string),
                    ip_address: args
                        .get("ipAddress")
                        .or_else(|| args.get("ip_address"))
                        .or_else(|| args.get("ip"))
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string),
                    vlan: args
                        .get("vlan")
                        .and_then(serde_json::Value::as_u64)
                        .and_then(|value| u32::try_from(value).ok()),
                    acl: args
                        .get("acl")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string),
                };
                let output = portable_runtime()?.block_on(self.graph.query_network(query))?;
                return Ok(ToolResult {
                    success: true,
                    output: serde_json::to_string(&output).map_err(|e| e.to_string())?,
                });
            }
            if tool == "get_subgraph" {
                let request = serde_json::from_value(args.clone())
                    .map_err(|e| format!("Invalid get_subgraph arguments: {e}"))?;
                let output = portable_runtime()?.block_on(self.graph.get_subgraph(request))?;
                return Ok(ToolResult {
                    success: true,
                    output: serde_json::to_string(&output).map_err(|e| e.to_string())?,
                });
            }
            if matches!(
                tool,
                "find_ip_by_mac" | "find_mac_by_ip" | "find_interface_by_mac"
            ) {
                use mikomai_adapters::portable_graph::EndpointLookup;
                let lookup = match tool {
                    "find_ip_by_mac" => EndpointLookup::IpByMac,
                    "find_mac_by_ip" => EndpointLookup::MacByIp,
                    _ => EndpointLookup::InterfaceByMac,
                };
                let key = if tool == "find_mac_by_ip" {
                    "ip"
                } else {
                    "mac"
                };
                let value = args
                    .get(key)
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| format!("{key} is required"))?;
                let device = args
                    .get("device")
                    .or_else(|| args.get("device_name"))
                    .or_else(|| args.get("deviceName"))
                    .and_then(serde_json::Value::as_str);
                let output = portable_runtime()?
                    .block_on(self.graph.find_endpoint(lookup, value, device))?;
                return Ok(ToolResult {
                    success: true,
                    output: serde_json::to_string(&output).map_err(|e| e.to_string())?,
                });
            }
            if matches!(tool, "query_nw_db" | "network_query_nw_db" | "query_rag") {
                let query = args
                    .get("query")
                    .or_else(|| args.get("userMessage"))
                    .and_then(serde_json::Value::as_str)
                    .filter(|query| !query.trim().is_empty())
                    .ok_or_else(|| "knowledge query is required".to_string())?;
                let brand = args
                    .get("brand")
                    .or_else(|| args.get("vendor"))
                    .or_else(|| args.get("device_vendor"))
                    .and_then(serde_json::Value::as_str)
                    .or_else(|| {
                        target
                            .and_then(|target| {
                                self.registry.devices().iter().find(|device| {
                                    target == device.hostname
                                        || device.id.as_deref() == Some(target)
                                        || device.ip.as_deref() == Some(target)
                                })
                            })
                            .and_then(|device| device.device_type.as_deref())
                    });
                for source in &self.rag_sources {
                    if !source.is_dir() {
                        continue;
                    }
                    let should_ingest = rag_ingested_paths()
                        .lock()
                        .map_err(|_| "RAG index state is unavailable".to_string())?
                        .insert(source.clone());
                    if should_ingest {
                        if let Err(error) = self.rag.ingest_path(source).await {
                            rag_ingested_paths()
                                .lock()
                                .ok()
                                .map(|mut paths| paths.remove(source));
                            return Err(error);
                        }
                    }
                }
                let hits = self.rag.search(query, brand).await?;
                return Ok(ToolResult {
                    success: hits.success,
                    output: hits.output,
                });
            }
            let target_id = target;
            let target = match target_id {
                Some(target) => target,
                None if mikomai_adapters::portable_device::is_local_tool_id(tool) => "localhost",
                None => return Err("device observation requires a registered target".into()),
            };
            let credentials = mikomai_adapters::portable_device::DeviceCredentials {
                username: String::new(),
                password: None,
                enable_password: None,
                private_key: None,
                passphrase: None,
            };
            let result =
                self.registry
                    .execute(&self.transport, tool, target, args, &credentials)?;
            let decoded =
                serde_json::from_str::<ToolResult>(&result.output).unwrap_or(ToolResult {
                    success: true,
                    output: result.output,
                });
            let decoded = ToolResult {
                success: decoded.success,
                output: mikomai_core::redaction::redact_network_secrets(&decoded.output),
            };
            if decoded.success {
                if let Some(device) = target_id.and_then(|target| {
                    self.registry.devices().iter().find(|device| {
                        target == device.hostname
                            || device.id.as_deref() == Some(target)
                            || device.ip.as_deref() == Some(target)
                    })
                }) {
                    if let Some((kind, normalized, canonical)) =
                        graph_observation(tool, args, &decoded.output)
                    {
                        self.graph
                            .ingest(mikomai_adapters::portable_graph::GraphIngestInput {
                                source_id: format!("swift-agent:{}", uuid::Uuid::new_v4()),
                                collected_at: chrono::Utc::now(),
                                device_name: device.hostname.clone(),
                                kind,
                                raw: decoded.output.clone(),
                                normalized,
                                canonical,
                                evidence: None,
                                normalizer_version: "portable-agent-v1".into(),
                            })
                            .await?;
                    }
                }
            }
            Ok(decoded)
        })
    }
}

fn graph_observation(
    tool: &str,
    args: &serde_json::Value,
    output: &str,
) -> Option<(
    mikomai_adapters::portable_graph::GraphDataKind,
    Option<serde_json::Value>,
    Option<serde_json::Value>,
)> {
    use mikomai_adapters::portable_graph::GraphDataKind;
    let resource = args
        .get("resource")
        .or_else(|| args.get("resourceType"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let command = args
        .get("command")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let name = match tool {
        "fetch_config" => "config",
        "fetch_routing" => "routing",
        "fetch_arp" => "arp",
        "get_state" => resource.as_str(),
        "network_show" => {
            if command.contains("arp") {
                "arp"
            } else if command.contains("route") {
                "routing"
            } else if command.contains("interface") {
                "interfaces"
            } else {
                return None;
            }
        }
        _ => return None,
    };
    match name {
        "arp" => {
            let table = parse_arp_observation(output);
            Some((GraphDataKind::Arp, Some(table.clone()), Some(table)))
        }
        "routing" | "routes" => {
            let routes = parse_routes_observation(output);
            Some((GraphDataKind::Routing, Some(routes.clone()), Some(routes)))
        }
        "interfaces" => {
            let interfaces = parse_interfaces_observation(output);
            Some((
                GraphDataKind::Interfaces,
                Some(interfaces.clone()),
                Some(interfaces),
            ))
        }
        "config" => Some((GraphDataKind::Config, None, None)),
        "lldp" => Some((GraphDataKind::Lldp, None, None)),
        "mac_table" => Some((GraphDataKind::MacTable, None, None)),
        "bgp" => Some((GraphDataKind::Bgp, None, None)),
        "ospf" => Some((GraphDataKind::Ospf, None, None)),
        _ => None,
    }
}

fn parse_arp_observation(raw: &str) -> serde_json::Value {
    let entries = raw
        .lines()
        .filter_map(|line| {
            let fields = line.split_whitespace().collect::<Vec<_>>();
            let ip = fields.iter().find_map(|field| {
                field
                    .trim_matches(|ch: char| matches!(ch, '(' | ')' | ','))
                    .parse::<std::net::IpAddr>()
                    .ok()
                    .map(|ip| ip.to_string())
            })?;
            let mac = fields
                .iter()
                .find(|field| {
                    let normalized = mikomai_core::network::canonicalization::normalize_mac(field);
                    normalized.len() == 17 && normalized.contains(':')
                })
                .map(|field| mikomai_core::network::canonicalization::normalize_mac(field));
            let interface = fields
                .iter()
                .rev()
                .find(|field| {
                    field.chars().any(|ch| ch.is_ascii_alphabetic())
                        && !field.eq_ignore_ascii_case("dynamic")
                        && !field.eq_ignore_ascii_case("static")
                        && !field.eq_ignore_ascii_case("incomplete")
                })
                .map(|field| (*field).to_string());
            Some(serde_json::json!({"ip_address":ip,"mac_address":mac,"interface":interface}))
        })
        .collect::<Vec<_>>();
    serde_json::json!({"arp_table":entries})
}

fn parse_interfaces_observation(raw: &str) -> serde_json::Value {
    let mut interfaces = Vec::<serde_json::Value>::new();
    let mut current: Option<serde_json::Value> = None;
    for line in raw.lines() {
        let trimmed = line.trim();
        if let Some(name) = trimmed.strip_prefix("interface ") {
            if let Some(item) = current.take() {
                interfaces.push(item);
            }
            current = Some(serde_json::json!({"name":name.trim()}));
            continue;
        }
        if let Some((name, _)) = trimmed.split_once(" is ") {
            if name.chars().any(|c| c.is_ascii_alphabetic()) && name.len() < 96 {
                if let Some(item) = current.take() {
                    interfaces.push(item);
                }
                current = Some(serde_json::json!({"name":name.trim()}));
            }
        }
        let address = trimmed
            .strip_prefix("Internet address is ")
            .or_else(|| trimmed.strip_prefix("ip address "));
        if let Some(address) = address {
            let address = address.split_whitespace().next().unwrap_or_default();
            let (ip, prefix_len) = if let Some((ip, prefix)) = address.split_once('/') {
                (ip.to_string(), prefix.parse::<u8>().ok())
            } else {
                (address.to_string(), None)
            };
            if ip.parse::<std::net::IpAddr>().is_ok() {
                if let Some(item) = current.as_mut().and_then(serde_json::Value::as_object_mut) {
                    item.insert("ipv4_addresses".into(), serde_json::json!([address]));
                    if let Some(prefix) = prefix_len {
                        item.insert("prefix_len".into(), serde_json::json!(prefix));
                    }
                }
            }
        }
    }
    if let Some(item) = current {
        interfaces.push(item);
    }
    let ip_addresses = interfaces
        .iter()
        .flat_map(|interface| {
            let name = interface
                .get("name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            interface
                .get("ipv4_addresses")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(move |address| {
                    let address = address.as_str()?;
                    let subnet = address.to_string();
                    Some(serde_json::json!({"address":address,"interface":name,"subnet":subnet}))
                })
        })
        .collect::<Vec<_>>();
    serde_json::json!({"interfaces":interfaces,"ip_addresses":ip_addresses})
}

fn parse_routes_observation(raw: &str) -> serde_json::Value {
    let routes = raw.lines().filter_map(|line| {
        let words = line.split_whitespace().collect::<Vec<_>>();
        let destination = words.iter().find(|word| {
            let token = word.trim_matches(|ch: char| matches!(ch, ',' | '(' | ')' | '[' | ']'));
            token == "default" || token.parse::<std::net::IpAddr>().is_ok() || token.split_once('/').is_some_and(|(ip, prefix)| ip.parse::<std::net::IpAddr>().is_ok() && prefix.parse::<u8>().is_ok())
        })?;
        let gateway = words.iter().position(|word| *word == "via").and_then(|idx| words.get(idx + 1)).copied().unwrap_or_default();
        let iface = words.iter().position(|word| *word == "dev" || *word == "is").and_then(|idx| words.get(idx + 1)).copied().unwrap_or_default();
        Some(serde_json::json!({"destination":destination,"gateway":gateway,"interface":iface,"raw":line.trim()}))
    }).collect::<Vec<_>>();
    serde_json::json!({"routes":routes})
}

struct FfiAgentReporter {
    callback: Option<MikomaiStreamCallback>,
    context: usize,
    snapshots: Arc<Mutex<HashMap<uuid::Uuid, TaskSnapshot>>>,
}

impl ReporterPort for FfiAgentReporter {
    fn report(&self, event: ReportEvent) {
        let mut snapshot_for_audit = None;
        if let Ok(mut snapshots) = self.snapshots.lock() {
            match &event {
                ReportEvent::TaskStarted { task_id } => {
                    if let Some(task) = snapshots.get_mut(task_id) {
                        task.status = mikomai_core::domain::TaskStatus::Running;
                    }
                }
                ReportEvent::Evidence { task_id, evidence } => {
                    if let Some(task) = snapshots.get_mut(task_id) {
                        task.evidence.push(evidence.clone());
                    }
                }
                ReportEvent::ApprovalRequired { task_id, .. } => {
                    if let Some(task) = snapshots.get_mut(task_id) {
                        task.status = mikomai_core::domain::TaskStatus::AwaitingApproval;
                    }
                }
                ReportEvent::Completed { task_id, .. } => {
                    if let Some(task) = snapshots.get_mut(task_id) {
                        task.status = mikomai_core::domain::TaskStatus::Completed;
                    }
                }
                ReportEvent::Status { .. } => {}
            }
            let task_id = match &event {
                ReportEvent::TaskStarted { task_id }
                | ReportEvent::Evidence { task_id, .. }
                | ReportEvent::ApprovalRequired { task_id, .. }
                | ReportEvent::Status { task_id, .. }
                | ReportEvent::Completed { task_id, .. } => task_id,
            };
            snapshot_for_audit = snapshots.get(task_id).cloned();
        }
        if let Some(snapshot) = snapshot_for_audit {
            let timestamp = chrono::Utc::now();
            let record = match &event {
                ReportEvent::TaskStarted { task_id } => serde_json::json!({
                    "event_type":"task_started", "task_id":task_id, "goal":snapshot.task.goal,
                    "timestamp":timestamp
                }),
                ReportEvent::Evidence { task_id, evidence } => serde_json::json!({
                    "event_type":"observation", "task_id":task_id, "evidence":evidence,
                    "timestamp":timestamp
                }),
                ReportEvent::ApprovalRequired {
                    task_id,
                    plan,
                    message,
                } => serde_json::json!({
                    "event_type":"approval_required", "task_id":task_id,
                    "plan":plan, "message":message, "timestamp":timestamp
                }),
                ReportEvent::Status { task_id, status } => serde_json::json!({
                    "event_type":"state_updated", "task_id":task_id, "status":status,
                    "timestamp":timestamp
                }),
                ReportEvent::Completed { task_id, answer } => serde_json::json!({
                    "event_type":"finished", "task_id":task_id, "answer":answer,
                    "timestamp":timestamp
                }),
            };
            let safe_record = mikomai_core::audit::redact(&record);
            if let Err(error) = persist_agent_audit(&snapshot, safe_record) {
                eprintln!("agent audit persistence failed: {error}");
            }
        }
        let Some(callback) = self.callback else {
            return;
        };
        let text = match event {
            ReportEvent::TaskStarted { .. } => {
                format!(
                    "__MIKOMAI_AGENT_PROGRESS__{}",
                    serde_json::json!({"phase":"開始", "nextAction":"調査計画を作成", "detail":"目的と対象を確認しています"})
                )
            }
            ReportEvent::Evidence { evidence, .. } => format!(
                "__MIKOMAI_AGENT_PROGRESS__{}",
                serde_json::json!({"phase":"結果整理", "nextAction":"追加調査または回答を判断", "detail":mikomai_core::audit::redact(&serde_json::json!({"output":evidence.content}))["output"].as_str().unwrap_or("").chars().take(12000).collect::<String>()})
            ),
            ReportEvent::ApprovalRequired { plan, message, .. } => format!(
                "__MIKOMAI_APPROVAL_PLAN__{}\n{}",
                serde_json::to_string(&plan).unwrap_or_default(),
                message
            ),
            ReportEvent::Status { status, .. } if status.starts_with("dispatch:") => String::new(),
            ReportEvent::Status { status, .. } => format!("__MIKOMAI_AGENT_PROGRESS__{status}"),
            ReportEvent::Completed { answer, .. } => answer,
        };
        if let Ok(text) = CString::new(text.replace('\0', "")) {
            unsafe {
                callback(text.as_ptr(), 0, self.context as *mut std::ffi::c_void);
            }
        }
    }
}

/// Runs a portable multi-step agent loop. Device execution is delegated to the
/// host callback, which resolves credentials from its own secure store.
#[no_mangle]
pub unsafe extern "C" fn mikomai_agent_chat_streaming(
    message: *const c_char,
    history: *const c_char,
    documents_dir: *const c_char,
    knowledge_dir: *const c_char,
    attachments: *const c_char,
    devices_json: *const c_char,
    callback: Option<MikomaiStreamCallback>,
    tool_callback: Option<MikomaiToolCallback>,
    plan_callback: Option<MikomaiPlanCallback>,
    context: *mut std::ffi::c_void,
) -> MikomaiResult {
    if message.is_null()
        || history.is_null()
        || documents_dir.is_null()
        || knowledge_dir.is_null()
        || devices_json.is_null()
        || tool_callback.is_none()
    {
        return error_result(
            "message, history, paths, device metadata and a read-only tool callback are required"
                .into(),
        );
    }
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let incoming = CStr::from_ptr(message)
            .to_str()
            .map_err(|e| e.to_string())?
            .to_owned();
        CANCEL_INFERENCE.store(false, Ordering::Relaxed);
        // Deterministic replies need neither RAG nor an agent task/status event.
        let empty_attachments =
            attachments.is_null() || CStr::from_ptr(attachments).to_bytes().is_empty();
        if let Some(reply) = empty_attachments
            .then(|| mikomai_core::dispatch::legacy_shortcut(&incoming))
            .flatten()
            .and_then(|shortcut| shortcut.reply)
        {
            if let Some(cb) = callback {
                let text = CString::new(reply.clone()).map_err(|e| e.to_string())?;
                cb(text.as_ptr(), 0, context);
                let done = CString::new("").unwrap();
                cb(done.as_ptr(), 1, context);
            }
            return Ok(reply);
        }
        let (resume_id, saved_resume_id, goal, selection) =
            if let Some(rest) = incoming.strip_prefix("__MIKOMAI_RESUME__") {
                let (id, selection) = rest
                    .split_once('\n')
                    .ok_or_else(|| "agent resume payload is malformed".to_string())?;
                let id = uuid::Uuid::parse_str(id.trim())
                    .map_err(|error| format!("invalid agent task id: {error}"))?;
                (
                    Some(id),
                    None,
                    String::new(),
                    Some(selection.trim().to_string()),
                )
            } else if let Some(id) = incoming.strip_prefix("__MIKOMAI_RESUME_SAVED__") {
                let id = uuid::Uuid::parse_str(id.trim())
                    .map_err(|error| format!("invalid saved agent task id: {error}"))?;
                (None, Some(id), String::new(), None)
            } else {
                (None, None, incoming, None)
            };
        if goal.trim().is_empty() && selection.as_deref().unwrap_or_default().is_empty() {
            return Err("chat message is required".into());
        }
        let history = CStr::from_ptr(history)
            .to_str()
            .map_err(|e| e.to_string())?
            .to_owned();
        let documents = expand_tilde(&PathBuf::from(
            CStr::from_ptr(documents_dir)
                .to_str()
                .map_err(|e| e.to_string())?,
        ));
        let knowledge = expand_tilde(&PathBuf::from(
            CStr::from_ptr(knowledge_dir)
                .to_str()
                .map_err(|e| e.to_string())?,
        ));
        let attachments = if attachments.is_null() {
            ""
        } else {
            CStr::from_ptr(attachments)
                .to_str()
                .map_err(|e| e.to_string())?
        };
        let attachments = prepare_attachments(&goal, attachments)?;
        let reference_material = if documents.is_dir() {
            chat_with_paths(&goal, documents.clone(), knowledge.clone()).unwrap_or_default()
        } else {
            String::new()
        };
        let device_text = CStr::from_ptr(devices_json)
            .to_str()
            .map_err(|e| e.to_string())?;
        let registry =
            mikomai_adapters::portable_device::ReadOnlyToolRegistry::from_json(device_text)?;
        let devices = registry
            .devices()
            .iter()
            .flat_map(|d| [d.hostname.clone(), d.ip.clone().unwrap_or_default()])
            .collect::<Vec<_>>();
        let tools = registry
            .tools()
            .into_iter()
            .map(|tool| tool.id)
            .chain([
                "query_network_graph".to_string(),
                "get_subgraph".to_string(),
                "find_ip_by_mac".to_string(),
                "find_mac_by_ip".to_string(),
                "find_interface_by_mac".to_string(),
                "require_host_registered".to_string(),
                "get_operation_plan".to_string(),
                "ask_user_choice".to_string(),
                "ask_interface_choice".to_string(),
                "ask_ipaddress_choice".to_string(),
                "network_config".to_string(),
                "network_send_console_message".to_string(),
                "network_ftp_download".to_string(),
                "network_ftp_upload".to_string(),
                "network_tftp_download".to_string(),
                "network_tftp_upload".to_string(),
            ])
            .collect::<Vec<_>>();
        let planner = FfiAgentPlanner {
            inventory: registry.devices().to_vec(),
            devices,
            tools,
            history,
            attachments: attachments.to_string(),
            reference_material,
            plan_callback,
            callback_context: context as usize,
        };
        let graph = portable_graph()?;
        let rag = mikomai_adapters::portable_rag::PortableRag::new(
            graph.clone(),
            Arc::new(mikomai_adapters::e5_embedder::FastEmbedE5::new()),
        );
        let executor = FfiAgentExecutor {
            registry,
            transport: SwiftCallbackTransport {
                callback: tool_callback.unwrap(),
                context: context as usize,
            },
            graph,
            rag,
            rag_sources: vec![documents.clone(), knowledge.clone()],
        };
        let task_snapshots = Arc::new(Mutex::new(HashMap::new()));
        let reporter = FfiAgentReporter {
            callback,
            context: context as usize,
            snapshots: task_snapshots.clone(),
        };
        let repository = JsonTaskRepository::default();
        let manager = TaskManager::new(repository.clone());
        let mut task = if let Some(id) = resume_id {
            resume_pending_agent_task(id, selection.unwrap_or_default())?
        } else if let Some(id) = saved_resume_id {
            resume_saved_agent_task(id)?
        } else {
            manager.start(goal).map_err(|error| error.to_string())?
        };
        mikomai_core::vision::retain_attachment_context(&mut task, &attachments);
        let task_id = task.task.id;
        task_snapshots
            .lock()
            .map_err(|_| "agent task snapshot state is unavailable".to_string())?
            .insert(task_id, task.clone());
        let service = ChatService::new(&planner, &executor, &reporter).with_max_steps(10);
        let mut response = match portable_runtime()?.block_on(manager.run_chat(&service, task)) {
            Ok(response) => response,
            Err(error) => {
                let failed = task_snapshots.lock().ok().and_then(|mut snapshots| {
                    snapshots.get_mut(&task_id).map(|snapshot| {
                        snapshot.status = mikomai_core::TaskStatus::Failed;
                        snapshot.clone()
                    })
                });
                if let Some(snapshot) = failed {
                    let _ = persist_agent_audit(
                        &snapshot,
                        serde_json::json!({"event_type":"finished","task_id":task_id,"error":error,"timestamp":chrono::Utc::now()}),
                    );
                }
                return Err(error);
            }
        };
        // Release the read guard before updating the same mutex below.
        let snapshot = {
            task_snapshots
                .lock()
                .map_err(|_| "agent task snapshot state is unavailable".to_string())?
                .get(&task_id)
                .cloned()
        };
        if let Some(mut snapshot) = snapshot {
            if response.starts_with("### ❓") {
                snapshot.status = mikomai_core::domain::TaskStatus::AwaitingInput;
            }
            if let Ok(mut snapshots) = task_snapshots.lock() {
                snapshots.insert(task_id, snapshot.clone());
            }
            let _ = persist_agent_audit(
                &snapshot,
                serde_json::json!({"event_type":"state_updated","task_id":task_id,"status":snapshot.status,"timestamp":chrono::Utc::now()}),
            );
            if snapshot.status == mikomai_core::domain::TaskStatus::AwaitingInput {
                let mut pending = pending_agent_tasks()
                    .lock()
                    .map_err(|_| "pending agent task state is unavailable".to_string())?;
                if pending.len() >= 128 {
                    if let Some(oldest) = pending.keys().next().copied() {
                        pending.remove(&oldest);
                    }
                }
                pending.insert(task_id, snapshot.clone());
                let question = snapshot
                    .evidence
                    .last()
                    .and_then(|item| item.content.strip_prefix("__ASK_HUMAN__"))
                    .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok())
                    .unwrap_or(serde_json::Value::Null);
                let display = if response.starts_with("### ❓") {
                    response
                } else {
                    format!("### ❓ 確認要求\n{response}")
                };
                let payload =
                    serde_json::json!({"task_id": task_id, "text": display, "question": question});
                response = format!("__MIKOMAI_CHOICE__{}", payload);
            }
        }
        if let Some(cb) = callback {
            let _ = CString::new("").map(|done| cb(done.as_ptr(), 1, context));
        }
        Ok(response)
    }));
    match caught {
        Ok(Ok(answer)) => result(0, answer),
        Ok(Err(error)) => error_result(error),
        Err(_) => error_result("agent chat failed unexpectedly".into()),
    }
}

/// Runs chat with legacy UTF-8 text or versioned PNG/JPEG image attachments.
/// Core validates images and routes analysis through the Vision adapter.
#[no_mangle]
pub unsafe extern "C" fn mikomai_assistant_chat_with_attachments(
    message: *const c_char,
    history: *const c_char,
    documents_dir: *const c_char,
    knowledge_dir: *const c_char,
    attachments: *const c_char,
) -> MikomaiResult {
    mikomai_assistant_chat_streaming(
        message,
        history,
        documents_dir,
        knowledge_dir,
        attachments,
        None,
        std::ptr::null_mut(),
    )
}

#[no_mangle]
pub unsafe extern "C" fn mikomai_assistant_chat_streaming(
    message: *const c_char,
    history: *const c_char,
    documents_dir: *const c_char,
    knowledge_dir: *const c_char,
    attachments: *const c_char,
    callback: Option<MikomaiStreamCallback>,
    context: *mut std::ffi::c_void,
) -> MikomaiResult {
    if message.is_null() || history.is_null() || documents_dir.is_null() || knowledge_dir.is_null()
    {
        return error_result("message, history and directories must not be null".into());
    }
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        CANCEL_INFERENCE.store(false, Ordering::Relaxed);
        let question = CStr::from_ptr(message)
            .to_str()
            .map_err(|e| e.to_string())?;
        let history = CStr::from_ptr(history)
            .to_str()
            .map_err(|e| e.to_string())?;
        let docs = expand_tilde(&PathBuf::from(
            CStr::from_ptr(documents_dir)
                .to_str()
                .map_err(|e| e.to_string())?,
        ));
        let index = expand_tilde(&PathBuf::from(
            CStr::from_ptr(knowledge_dir)
                .to_str()
                .map_err(|e| e.to_string())?,
        ));
        let attachments = if attachments.is_null() {
            ""
        } else {
            CStr::from_ptr(attachments)
                .to_str()
                .map_err(|e| format!("attachment text is not valid UTF-8: {e}"))?
        };
        let attachments = prepare_attachments(question, attachments)?;
        let evidence = if docs.exists() && docs.is_dir() {
            chat_with_paths(question, docs, index).unwrap_or_else(|err| {
                eprintln!("RAG lookup failed: {err}");
                String::new()
            })
        } else {
            String::new()
        };
        let response = mikomai_core::response::ResponseContext {
            question,
            history,
            references: &evidence,
            attachments: &attachments,
        };
        response.answer_streaming(
            &mikomai_adapters::local_llama::LocalInference,
            &mut |chunk, is_done| {
                if let Some(cb) = callback {
                    if let Ok(c_chunk) = CString::new(chunk.replace('\0', "")) {
                        cb(c_chunk.as_ptr(), if is_done { 1 } else { 0 }, context);
                    }
                }
            },
        )
    }));
    match caught {
        Ok(Ok(answer)) => result(0, answer),
        Ok(Err(error)) => error_result(error),
        Err(_) => error_result("assistant chat streaming failed unexpectedly".into()),
    }
}

/// Tests TCP connectivity to a host and port with timeout in milliseconds.
#[no_mangle]
pub unsafe extern "C" fn mikomai_test_tcp_connection(
    host: *const c_char,
    port: u16,
    timeout_ms: u32,
) -> MikomaiResult {
    if host.is_null() {
        return error_result("host must not be null".into());
    }
    let caught = std::panic::catch_unwind(|| {
        let host_str = CStr::from_ptr(host).to_str().map_err(|e| e.to_string())?;
        test_tcp_connection_core(host_str, port, timeout_ms)
    });
    match caught {
        Ok(Ok(report)) => result(0, report),
        Ok(Err(error)) => error_result(error),
        Err(_) => error_result("connection test failed unexpectedly".into()),
    }
}

fn test_tcp_connection_core(host: &str, port: u16, timeout_ms: u32) -> Result<String, String> {
    use std::net::{TcpStream, ToSocketAddrs};
    use std::time::{Duration, Instant};

    let trimmed = host.trim();
    if trimmed.is_empty() {
        return Err("ホスト名またはIPアドレスが指定されていません。".into());
    }
    let addr_str = if trimmed.contains(':') && !trimmed.starts_with('[') {
        format!("[{}]:{}", trimmed, port)
    } else {
        format!("{}:{}", trimmed, port)
    };
    let timeout = Duration::from_millis(if timeout_ms == 0 {
        2000
    } else {
        timeout_ms as u64
    });
    let start = Instant::now();

    let addrs = addr_str
        .to_socket_addrs()
        .map_err(|e| format!("ホスト '{}' の名前解決に失敗しました: {e}", trimmed))?;

    let mut last_err = None;
    for addr in addrs {
        match TcpStream::connect_timeout(&addr, timeout) {
            Ok(_stream) => {
                let latency = start.elapsed().as_millis();
                return Ok(format!(
                    "接続成功: {} (ポート {}, {} ms)",
                    addr, port, latency
                ));
            }
            Err(e) => {
                last_err = Some(format!("{addr}: {e}"));
            }
        }
    }
    Err(format!(
        "接続失敗 (ポート {port}): {}",
        last_err.unwrap_or_else(|| "アドレスが見つかりませんでした".into())
    ))
}

#[repr(C)]
pub struct MikomaiResult {
    pub status: i32,
    pub message: *mut c_char,
}

/// Runs the same local knowledge chat flow as `mikomai-cli chat`.
/// `message` must point to a valid, NUL-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn mikomai_chat(message: *const c_char) -> MikomaiResult {
    if message.is_null() {
        return error_result("chat message must not be null".into());
    }

    let caught = std::panic::catch_unwind(|| {
        let goal = CStr::from_ptr(message)
            .to_str()
            .map_err(|error| error.to_string())?;
        chat(goal)
    });

    match caught {
        Ok(Ok(answer)) => result(0, answer),
        Ok(Err(error)) => error_result(error),
        Err(_) => error_result("mikomai chat failed unexpectedly".into()),
    }
}

/// Runs local knowledge chat using explicitly selected document and index directories.
/// The two directory arguments must be valid, NUL-terminated UTF-8 strings.
#[no_mangle]
pub unsafe extern "C" fn mikomai_chat_with_paths(
    message: *const c_char,
    documents_dir: *const c_char,
    knowledge_dir: *const c_char,
) -> MikomaiResult {
    if message.is_null() || documents_dir.is_null() || knowledge_dir.is_null() {
        return error_result("chat message and directories must not be null".into());
    }

    let caught = std::panic::catch_unwind(|| {
        let goal = CStr::from_ptr(message)
            .to_str()
            .map_err(|error| error.to_string())?;
        let documents = CStr::from_ptr(documents_dir)
            .to_str()
            .map_err(|error| error.to_string())?;
        let knowledge = CStr::from_ptr(knowledge_dir)
            .to_str()
            .map_err(|error| error.to_string())?;
        chat_with_paths(goal, PathBuf::from(documents), PathBuf::from(knowledge))
    });

    match caught {
        Ok(Ok(answer)) => result(0, answer),
        Ok(Err(error)) => error_result(error),
        Err(_) => error_result("mikomai chat failed unexpectedly".into()),
    }
}

/// Releases the message returned in `MikomaiResult`.
#[no_mangle]
pub unsafe extern "C" fn mikomai_result_free(result: MikomaiResult) {
    if !result.message.is_null() {
        drop(CString::from_raw(result.message));
    }
}

/// Rust-facing entry points used by the standalone CLI. They intentionally use
/// the same process-local model and chat implementation as the Swift C ABI.
pub fn load_local_model(path: &str) -> Result<(), String> {
    let path = CString::new(path).map_err(|error| error.to_string())?;
    let response = unsafe { mikomai_model_load(path.as_ptr()) };
    consume_result(response).map(|_| ())
}

pub fn local_model_chat(
    message: &str,
    history: &str,
    documents_dir: &str,
    knowledge_dir: &str,
) -> Result<String, String> {
    let message = CString::new(message).map_err(|error| error.to_string())?;
    let history = CString::new(history).map_err(|error| error.to_string())?;
    let documents = CString::new(documents_dir).map_err(|error| error.to_string())?;
    let knowledge = CString::new(knowledge_dir).map_err(|error| error.to_string())?;
    let empty_attachments = CString::new("").unwrap();
    let response = unsafe {
        mikomai_assistant_chat_streaming(
            message.as_ptr(),
            history.as_ptr(),
            documents.as_ptr(),
            knowledge.as_ptr(),
            empty_attachments.as_ptr(),
            None,
            std::ptr::null_mut(),
        )
    };
    consume_result(response)
}

fn consume_result(response: MikomaiResult) -> Result<String, String> {
    if response.message.is_null() {
        return Err("LLM runtime returned an empty result".into());
    }
    let status = response.status;
    let message = unsafe {
        CStr::from_ptr(response.message)
            .to_string_lossy()
            .into_owned()
    };
    unsafe { mikomai_result_free(response) };
    if status == 0 {
        Ok(message)
    } else {
        Err(message)
    }
}

fn chat(goal: &str) -> Result<String, String> {
    let knowledge_root = std::env::var_os("MIKOMAI_KNOWLEDGE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("mikomai-knowledge"));
    let documents = std::env::var_os("MIKOMAI_DOCS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("nw-docs"));
    chat_with_paths(goal, documents, knowledge_root)
}

fn chat_with_paths(
    goal: &str,
    documents: PathBuf,
    knowledge_root: PathBuf,
) -> Result<String, String> {
    if goal.trim().is_empty() {
        return Err("chat message is required".into());
    }

    if !documents.is_dir() {
        return Err(format!(
            "documents directory is missing or is not a directory: {}",
            documents.display()
        ));
    }
    let store = KnowledgeStore::at(knowledge_root.clone());
    store.ingest(documents.clone())?;

    let graph = portable_graph()?;
    let rag = mikomai_adapters::portable_rag::PortableRag::new(
        graph,
        Arc::new(mikomai_adapters::e5_embedder::FastEmbedE5::new()),
    );
    let retrieval = portable_runtime()?.block_on(async {
        for source in [&documents, &knowledge_root] {
            if !source.is_dir() {
                continue;
            }
            let should_ingest = rag_ingested_paths()
                .lock()
                .map_err(|_| "RAG index state is unavailable".to_string())?
                .insert(source.clone());
            if should_ingest {
                if let Err(error) = rag.ingest_path(source).await {
                    rag_ingested_paths()
                        .lock()
                        .ok()
                        .map(|mut paths| paths.remove(source));
                    return Err(error);
                }
            }
        }
        let result = rag.search(goal, infer_rag_brand(goal)).await?;
        let paths = result
            .citations
            .iter()
            .take(3)
            .map(|citation| citation.source_path.clone())
            .collect::<Vec<_>>();
        if paths.is_empty() {
            Ok(None)
        } else {
            rag.expand_selected_documents(&paths).await.map(Some)
        }
    });
    if let Ok(Some(retrieved)) = retrieval {
        if !retrieved.trim().is_empty() {
            return Ok(retrieved);
        }
    }

    // Keep a local lexical fallback when the optional E5 model cannot be
    // downloaded or when no vector candidates meet the legacy evidence bar.
    let manager = TaskManager::new(JsonTaskRepository::default());
    let planner = KnowledgePlanner::new(&store);
    let executor = EchoToolExecutor;
    let reporter = StdoutReporter::default();
    let task = manager.start(goal).map_err(|error| error.to_string())?;
    let service = ChatService::new(&planner, &executor, &reporter);
    portable_runtime()?.block_on(manager.run_chat(&service, task))
}

fn infer_rag_brand(query: &str) -> Option<&'static str> {
    let normalized = query.to_ascii_lowercase();
    if ["f220", "fx201", "fx310", "fitelnet", "furukawa"]
        .iter()
        .any(|model| normalized.contains(model))
    {
        Some("furukawa_fitelnet")
    } else if normalized.contains("cisco") || normalized.contains("ios") {
        Some("cisco_ios")
    } else if normalized.contains("juniper") || normalized.contains("junos") {
        Some("juniper_junos")
    } else if normalized.contains("yamaha") || normalized.contains("rtx") {
        Some("yamaha_rtx")
    } else {
        None
    }
}

fn error_result(message: String) -> MikomaiResult {
    result(1, message)
}

fn result(status: i32, message: String) -> MikomaiResult {
    let message = CString::new(message.replace('\0', "�"))
        .unwrap_or_else(|_| CString::new("mikomai returned invalid text").unwrap());
    MikomaiResult {
        status,
        message: message.into_raw(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        mikomai_assistant_chat, mikomai_assistant_chat_with_attachments, mikomai_chat,
        mikomai_chat_with_paths, mikomai_device_registry_read, mikomai_dispatch_mode,
        mikomai_model_load, mikomai_result_free,
    };
    use std::ffi::{CStr, CString};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};
    static TEST_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    static WATCH_NOTIFICATIONS: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

    unsafe extern "C" fn fake_watch_tool(
        _tool_id: *const std::ffi::c_char,
        _target_json: *const std::ffi::c_char,
        _args_json: *const std::ffi::c_char,
        output: *mut std::ffi::c_char,
        capacity: usize,
        _context: *mut std::ffi::c_void,
    ) -> i32 {
        let target = unsafe { CStr::from_ptr(_target_json) }.to_string_lossy();
        let response = if !target.contains("invalid-router") {
            r#"{"success":true,"output":"{\"usage\":90}"}"#
        } else {
            r#"{"success":true,"output":"{}"}"#
        };
        if output.is_null() || capacity <= response.len() {
            return 1;
        }
        std::ptr::copy_nonoverlapping(response.as_ptr(), output.cast::<u8>(), response.len());
        *output.add(response.len()) = 0;
        0
    }

    unsafe extern "C" fn fake_watch_notification(
        notification_json: *const std::ffi::c_char,
        _context: *mut std::ffi::c_void,
    ) {
        if let Ok(value) = unsafe { CStr::from_ptr(notification_json) }.to_str() {
            WATCH_NOTIFICATIONS.lock().unwrap().push(value.to_owned());
        }
    }

    fn watch_request(name: &str) -> serde_json::Value {
        serde_json::json!({
            "name": name,
            "ir": {
                "version": 1,
                "schedule": {"every":"60s"},
                "steps": [
                    {"id":"cpu","call":"get_state","args":{"device":if name == "missing usage" {"invalid-router"} else {"router-01"},"resource":"cpu"}},
                    {"when":{"left":{"ref":"cpu.usage"},"operator":"gt","right":80.0},"then":[{"call":"notify","args":{"message":"cpu high"}}]}
                ]
            }
        })
    }

    #[test]
    fn ffi_watch_lifecycle_runs_callback_notifies_persists_and_joins() {
        let path =
            std::env::temp_dir().join(format!("mikomai-watch-{}.json", uuid::Uuid::new_v4()));
        WATCH_NOTIFICATIONS.lock().unwrap().clear();
        let path_text = CString::new(path.to_string_lossy().as_bytes()).unwrap();
        let started = unsafe {
            super::mikomai_watch_start(
                path_text.as_ptr(),
                Some(fake_watch_tool),
                Some(fake_watch_notification),
                1usize as *mut std::ffi::c_void,
            )
        };
        assert_eq!(started.status, 0);
        unsafe { super::mikomai_result_free(started) };

        let create = |name: &str| {
            let request =
                CString::new(serde_json::to_string(&watch_request(name)).unwrap()).unwrap();
            let response = unsafe { super::mikomai_watch_create(request.as_ptr()) };
            assert_eq!(response.status, 0);
            let json: serde_json::Value = unsafe {
                serde_json::from_str(CStr::from_ptr(response.message).to_str().unwrap()).unwrap()
            };
            unsafe { super::mikomai_result_free(response) };
            json["id"].as_str().unwrap().to_owned()
        };
        let run = |id: &str| {
            let id = CString::new(id).unwrap();
            let response = unsafe { super::mikomai_watch_run_now(id.as_ptr()) };
            assert_eq!(response.status, 0);
            let json: serde_json::Value = unsafe {
                serde_json::from_str(CStr::from_ptr(response.message).to_str().unwrap()).unwrap()
            };
            unsafe { super::mikomai_result_free(response) };
            json
        };

        let success_id = create("valid CPU");
        let success_run = run(&success_id);
        assert_eq!(success_run["notifications"].as_array().unwrap().len(), 1);
        assert_eq!(WATCH_NOTIFICATIONS.lock().unwrap().len(), 1);
        let listing = super::mikomai_watch_list();
        let listed: serde_json::Value = unsafe {
            serde_json::from_str(CStr::from_ptr(listing.message).to_str().unwrap()).unwrap()
        };
        unsafe { super::mikomai_result_free(listing) };
        let persisted = listed
            .as_array()
            .unwrap()
            .iter()
            .find(|watch| watch["id"] == success_id)
            .unwrap();
        assert_eq!(persisted["history"].as_array().unwrap().len(), 1);
        assert_eq!(
            persisted["history"][0]["notifications"][0]["message"],
            "cpu high"
        );

        let invalid_id = create("missing usage");
        let invalid_run = run(&invalid_id);
        assert!(invalid_run["error"]
            .as_str()
            .unwrap()
            .contains("numeric usage"));
        let stopped = super::mikomai_watch_stop();
        assert_eq!(stopped.status, 0);
        unsafe { super::mikomai_result_free(stopped) };
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn ffi_dispatch_matches_shared_agent_policy_and_device_context() {
        for (message_text, expected) in [
            ("F220のVLAN設定方法を教えて", "worker"),
            ("R1の状態を確認して", "agent"),
            ("router-aの現在状態", "agent"),
        ] {
            let message = CString::new(message_text).unwrap();
            let devices =
                CString::new(r#"[{"id":"r1","hostname":"router-a","ip":"192.0.2.1"}]"#).unwrap();
            let response = unsafe { mikomai_dispatch_mode(message.as_ptr(), devices.as_ptr()) };
            assert_eq!(response.status, 0);
            assert_eq!(
                unsafe { CStr::from_ptr(response.message).to_str().unwrap() },
                expected
            );
            unsafe { mikomai_result_free(response) };
        }
    }

    #[test]
    fn choice_resume_keeps_agent_task_and_continues_to_the_next_tool() {
        let mut pending = mikomai_core::TaskSnapshot::new("DHCPRequestを実行");
        let task_id = pending.task.id;
        pending.status = mikomai_core::domain::TaskStatus::AwaitingInput;
        pending.evidence.push(mikomai_core::Evidence::from_tool(
            r#"__ASK_HUMAN__{"title":"送信確認","question":"DHCP要求を送信しますか？","options":["はい","いいえ"]}"#,
            None,
            Some("ask_user_choice".into()),
        ));
        super::pending_agent_tasks()
            .lock()
            .unwrap()
            .insert(task_id, pending);

        let resumed = super::resume_pending_agent_task(task_id, "はい".into()).unwrap();
        assert_eq!(resumed.task.id, task_id);
        assert_eq!(resumed.task.goal, "DHCPRequestを実行");
        assert_eq!(
            resumed.evidence.last().unwrap().content,
            "__USER_CHOICE__はい"
        );

        let planner = super::FfiAgentPlanner {
            inventory: Vec::new(),
            devices: Vec::new(),
            tools: vec!["network_packet_safety".into()],
            history: String::new(),
            attachments: String::new(),
            reference_material: String::new(),
            plan_callback: None,
            callback_context: 0,
        };
        let decision = super::portable_runtime()
            .unwrap()
            .block_on(mikomai_core::port::PlannerPort::plan(&planner, &resumed))
            .unwrap();
        assert!(
            matches!(decision, mikomai_core::port::PlanDecision::Observe { ref tool, ref args, .. }
            if tool == "network_packet_safety" && args["intent"] == "dhcp_request_probe")
        );
    }

    #[test]
    fn legacy_agent_audit_resume_preserves_observations_in_a_new_task() {
        let _guard = TEST_ENV_LOCK.lock().unwrap();
        let root =
            std::env::temp_dir().join(format!("mikomai-agent-resume-{}", uuid::Uuid::new_v4()));
        let previous = std::env::var_os("MIKOMAI_DATA_DIR");
        std::env::set_var("MIKOMAI_DATA_DIR", &root);
        let old_id = uuid::Uuid::new_v4();
        let legacy = serde_json::json!({"events":[
            {"event_type":"task_started","task_id":old_id.to_string(),"timestamp":"2026-10-01T01:00:00Z"},
            {"event_type":"goal_set","goal":"inspect router CPU"},
            {"event_type":"observation","raw":"CPU usage 91%","source":{"device":"router-01","tool_name":"get_state"}}
        ]});
        let converted = super::legacy_task_snapshot(&legacy).expect("legacy audit should convert");
        assert_eq!(converted.snapshot.evidence.len(), 1);
        assert_eq!(converted.snapshot.evidence[0].content, "CPU usage 91%");
        super::persist_agent_audit(
            &converted.snapshot,
            serde_json::json!({"event_type":"legacy_import"}),
        )
        .unwrap();
        let resumed = super::resume_saved_agent_task(old_id).unwrap();
        assert_ne!(resumed.task.id, old_id);
        assert!(resumed
            .evidence
            .iter()
            .any(|item| item.content == "CPU usage 91%"));
        assert!(resumed.evidence.last().unwrap().content.contains("resumed"));
        if let Some(previous) = previous {
            std::env::set_var("MIKOMAI_DATA_DIR", previous);
        } else {
            std::env::remove_var("MIKOMAI_DATA_DIR");
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn approved_write_claim_survives_runtime_reinitialization() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir()
            .join(format!("mikomai-claims-{nonce}.json"))
            .with_extension("executed.json");
        let claims = std::collections::HashSet::from([format!("consumed-{nonce}")]);
        super::persist_generic_execution_claims_at(&path, &claims).unwrap();
        let restored = super::load_generic_execution_claims(&path);
        assert!(restored.contains(&format!("consumed-{nonce}")));
        let _ = std::fs::remove_file(path.with_extension("tmp"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn assistant_chat_rejects_oversized_attachment_before_inference() {
        let message = CString::new("質問").unwrap();
        let history = CString::new("").unwrap();
        let docs = CString::new("").unwrap();
        let index = CString::new("").unwrap();
        let attachments = CString::new("x".repeat(1024 * 1024 + 1)).unwrap();
        let response = unsafe {
            mikomai_assistant_chat_with_attachments(
                message.as_ptr(),
                history.as_ptr(),
                docs.as_ptr(),
                index.as_ptr(),
                attachments.as_ptr(),
            )
        };
        assert_eq!(response.status, 1);
        let error = unsafe { CStr::from_ptr(response.message).to_string_lossy() };
        assert!(error.contains("128 KiB"));
        unsafe { mikomai_result_free(response) };
    }

    #[test]
    fn assistant_chat_rejects_image_pdf_and_non_text_attachment_markers() {
        let message = CString::new("添付を確認してください").unwrap();
        let history = CString::new("").unwrap();
        let docs = CString::new("").unwrap();
        let index = CString::new("").unwrap();
        for (payload, expected) in [
            (
                "[添付ファイル 1: diagram.png]\nimage bytes",
                "画像添付は未対応",
            ),
            (
                "[添付ファイル 1: notes.pdf]\nraw PDF bytes",
                "PDFからテキストを抽出",
            ),
            (
                "[添付ファイル 1: firmware.bin]\nbinary bytes",
                "UTF-8テキスト形式のみ",
            ),
        ] {
            let attachments = CString::new(payload).unwrap();
            let response = unsafe {
                mikomai_assistant_chat_with_attachments(
                    message.as_ptr(),
                    history.as_ptr(),
                    docs.as_ptr(),
                    index.as_ptr(),
                    attachments.as_ptr(),
                )
            };
            assert_eq!(response.status, 1);
            let error = unsafe { CStr::from_ptr(response.message).to_string_lossy() };
            assert!(
                error.contains(expected),
                "expected {expected:?}, got {error:?}"
            );
            unsafe { mikomai_result_free(response) };
        }
    }

    #[test]
    fn assistant_chat_keeps_supported_text_attachment_on_text_only_boundary() {
        let message = CString::new("添付を確認してください").unwrap();
        let history = CString::new("").unwrap();
        let docs = CString::new("").unwrap();
        let index = CString::new("").unwrap();
        for payload in [
            "[添付ファイル 1: notes.md]\nrouter config".to_owned(),
            format!("[添付ファイル 1: notes.md]\n{}", "x".repeat(128 * 1024)),
            "説明文に data:image/png と data:application/pdf を含みます。".to_owned(),
        ] {
            let attachments = CString::new(payload).unwrap();
            let response = unsafe {
                mikomai_assistant_chat_with_attachments(
                    message.as_ptr(),
                    history.as_ptr(),
                    docs.as_ptr(),
                    index.as_ptr(),
                    attachments.as_ptr(),
                )
            };
            assert_eq!(response.status, 1);
            let error = unsafe { CStr::from_ptr(response.message).to_string_lossy() };
            assert!(
                error.contains("モデルが未ロードです"),
                "supported text payload should reach inference, got: {error}"
            );
            unsafe { mikomai_result_free(response) };
        }

        let oversized = CString::new(format!(
            "[添付ファイル 1: notes.md]\n{}",
            "x".repeat(128 * 1024 + 1)
        ))
        .unwrap();
        let response = unsafe {
            mikomai_assistant_chat_with_attachments(
                message.as_ptr(),
                history.as_ptr(),
                docs.as_ptr(),
                index.as_ptr(),
                oversized.as_ptr(),
            )
        };
        assert_eq!(response.status, 1);
        let error = unsafe { CStr::from_ptr(response.message).to_string_lossy() };
        assert!(error.contains("128 KiB"), "got unexpected error: {error}");
        unsafe { mikomai_result_free(response) };
    }

    unsafe extern "C" fn capture_chat_chunk(
        text: *const std::ffi::c_char,
        done: i32,
        context: *mut std::ffi::c_void,
    ) {
        let events = &mut *(context as *mut Vec<(String, i32)>);
        events.push((CStr::from_ptr(text).to_string_lossy().into_owned(), done));
    }

    #[test]
    fn agent_reporter_streams_structured_phases_separately_from_answer() {
        use mikomai_core::port::{ReportEvent, ReporterPort};
        let mut chunks: Vec<(String, i32)> = Vec::new();
        let reporter = super::FfiAgentReporter {
            callback: Some(capture_chat_chunk),
            context: &mut chunks as *mut _ as usize,
            snapshots: std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
        };
        let task_id = uuid::Uuid::new_v4();
        reporter.report(ReportEvent::TaskStarted { task_id });
        reporter.report(ReportEvent::Status { task_id, status: serde_json::json!({"phase":"実行", "nextAction":"結果を確認", "detail":"get_state · sw1"}).to_string() });
        reporter.report(ReportEvent::Completed {
            task_id,
            answer: "診断完了".into(),
        });
        let prefix = "__MIKOMAI_AGENT_PROGRESS__";
        let start: serde_json::Value =
            serde_json::from_str(chunks[0].0.strip_prefix(prefix).unwrap()).unwrap();
        let executing: serde_json::Value =
            serde_json::from_str(chunks[1].0.strip_prefix(prefix).unwrap()).unwrap();
        assert_eq!(start["phase"], "開始");
        assert_eq!(executing["detail"], "get_state · sw1");
        assert_eq!(chunks[2].0, "診断完了");
    }

    #[test]
    fn agent_greetings_preserve_legacy_reply_without_progress_or_model() {
        for greeting in [
            "こんにちは",
            "おはようございます",
            "こんばんは",
            "hello",
            "hi",
            "自己紹介",
        ] {
            let input = CString::new(greeting).unwrap();
            let empty = CString::new("").unwrap();
            let devices = CString::new("[]").unwrap();
            let mut events: Vec<(String, i32)> = Vec::new();
            let response = unsafe {
                super::mikomai_agent_chat_streaming(
                    input.as_ptr(),
                    empty.as_ptr(),
                    empty.as_ptr(),
                    empty.as_ptr(),
                    empty.as_ptr(),
                    devices.as_ptr(),
                    Some(capture_chat_chunk),
                    Some(fake_watch_tool),
                    None,
                    &mut events as *mut _ as *mut _,
                )
            };
            assert_eq!(response.status, 0);
            let answer = unsafe { CStr::from_ptr(response.message) }
                .to_str()
                .unwrap();
            let expected = mikomai_core::dispatch::legacy_shortcut(greeting)
                .unwrap()
                .reply
                .unwrap();
            assert_eq!(answer, expected);
            assert_eq!(events, vec![(expected, 0), (String::new(), 1)]);
            unsafe { mikomai_result_free(response) };
        }
    }

    #[test]
    fn worker_greeting_uses_the_same_legacy_reply_without_model_or_rag() {
        let input = CString::new("こんにちは").unwrap();
        let empty = CString::new("").unwrap();
        let mut events: Vec<(String, i32)> = Vec::new();
        let response = unsafe {
            super::mikomai_assistant_chat_streaming(
                input.as_ptr(),
                empty.as_ptr(),
                empty.as_ptr(),
                empty.as_ptr(),
                empty.as_ptr(),
                Some(capture_chat_chunk),
                &mut events as *mut _ as *mut _,
            )
        };
        assert_eq!(response.status, 0);
        let expected = mikomai_core::dispatch::legacy_shortcut("こんにちは")
            .unwrap()
            .reply
            .unwrap();
        assert_eq!(
            unsafe { CStr::from_ptr(response.message) }
                .to_str()
                .unwrap(),
            expected
        );
        assert_eq!(events, vec![(expected, 0), (String::new(), 1)]);
        unsafe { mikomai_result_free(response) };
    }

    use mikomai_core::{port::PlanDecision, TaskSnapshot};
    use std::ffi::c_char;

    fn arp_inventory(names: &[&str]) -> Vec<mikomai_adapters::portable_device::RegisteredDevice> {
        names
            .iter()
            .map(|name| mikomai_adapters::portable_device::RegisteredDevice {
                id: None,
                hostname: (*name).into(),
                ip: Some("192.0.2.1".into()),
                device_type: Some("F220".into()),
            })
            .collect()
    }

    #[test]
    fn registered_arp_resolves_name_or_unique_model_and_asks_on_ambiguity() {
        let task = TaskSnapshot::new("F220 のARPを確認して");
        for inventory in [arp_inventory(&["F220"]), arp_inventory(&["branch-router"])] {
            let decision = super::registered_arp_decision(&task, &inventory).unwrap();
            assert!(
                matches!(decision, PlanDecision::Observe { tool, target, args }
                if tool == "get_state" && target == Some(inventory[0].hostname.clone())
                && args["device"] == inventory[0].hostname && args["resource"] == "arp")
            );
        }
        let inventory = arp_inventory(&["branch-a", "branch-b"]);
        assert!(matches!(
            super::registered_arp_decision(&task, &inventory),
            Some(PlanDecision::AskUser { .. })
        ));
        assert!(matches!(
            super::registered_arp_decision(&task, &[]),
            Some(PlanDecision::AskUser { .. })
        ));
        let mut resumed = task.clone();
        resumed.evidence.push(mikomai_core::Evidence::from_tool(
            "__USER_CHOICE__2",
            None,
            None,
        ));
        assert!(
            matches!(super::registered_arp_decision(&resumed, &inventory), Some(PlanDecision::Observe { target, .. }) if target.as_deref() == Some("branch-b"))
        );
        assert!(super::registered_arp_decision(
            &TaskSnapshot::new("F220 のARP確認方法を教えて"),
            &inventory
        )
        .is_none());
    }

    unsafe extern "C" fn fake_arp_read(
        tool: *const c_char,
        target: *const c_char,
        args: *const c_char,
        output: *mut c_char,
        capacity: usize,
        context: *mut std::ffi::c_void,
    ) -> i32 {
        let calls = &mut *(context as *mut Vec<(String, serde_json::Value, serde_json::Value)>);
        calls.push((
            CStr::from_ptr(tool).to_string_lossy().into_owned(),
            serde_json::from_str(CStr::from_ptr(target).to_str().unwrap()).unwrap(),
            serde_json::from_str(CStr::from_ptr(args).to_str().unwrap()).unwrap(),
        ));
        let response = serde_json::json!({"success":true,"output":"192.0.2.10 aa:bb:cc:dd:ee:ff GigaEthernet 1/1"}).to_string();
        if capacity <= response.len() {
            return 1;
        }
        std::ptr::copy_nonoverlapping(response.as_ptr(), output.cast::<u8>(), response.len());
        *output.add(response.len()) = 0;
        0
    }

    #[test]
    fn registered_arp_agent_runs_one_read_and_completes_without_llm() {
        let _guard = TEST_ENV_LOCK.lock().unwrap();
        let root = std::env::temp_dir().join(format!("mikomai-arp-test-{}", uuid::Uuid::new_v4()));
        let previous = std::env::var_os("MIKOMAI_DATA_DIR");
        std::env::set_var("MIKOMAI_DATA_DIR", &root);
        let input = CString::new("F220 のARPを確認して").unwrap();
        let empty = CString::new("").unwrap();
        let devices =
            CString::new(serde_json::to_string(&arp_inventory(&["branch-router"])).unwrap())
                .unwrap();
        let mut calls: Vec<(String, serde_json::Value, serde_json::Value)> = Vec::new();
        let response = unsafe {
            super::mikomai_agent_chat_streaming(
                input.as_ptr(),
                empty.as_ptr(),
                empty.as_ptr(),
                empty.as_ptr(),
                empty.as_ptr(),
                devices.as_ptr(),
                None,
                Some(fake_arp_read),
                None,
                (&mut calls as *mut Vec<_>).cast(),
            )
        };
        let answer = unsafe { CStr::from_ptr(response.message) }
            .to_string_lossy()
            .into_owned();
        let status = response.status;
        unsafe { mikomai_result_free(response) };
        match previous {
            Some(value) => std::env::set_var("MIKOMAI_DATA_DIR", value),
            None => std::env::remove_var("MIKOMAI_DATA_DIR"),
        }
        assert_eq!(status, 0, "{answer}");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "get_state");
        assert_eq!(calls[0].1["hostname"], "branch-router");
        assert_eq!(calls[0].2["resource"], "arp");
        assert!(answer.contains("branch-router のARP確認結果") && answer.contains("192.0.2.10"));
    }

    #[test]
    fn agent_planner_stops_before_planning_another_tool() {
        let planner = super::FfiAgentPlanner {
            inventory: Vec::new(),
            devices: vec!["router-01".into()],
            tools: vec!["get_state".into()],
            history: String::new(),
            attachments: String::new(),
            reference_material: String::new(),
            plan_callback: None,
            callback_context: 0,
        };
        let task = mikomai_core::TaskSnapshot::new("router-01 のARPを確認");
        let decision = super::portable_runtime()
            .unwrap()
            .block_on(planner.plan_with_cancellation(&task, true));
        assert!(
            matches!(decision.unwrap(), mikomai_core::port::PlanDecision::Complete { brief }
            if brief == "生成を停止しました。")
        );
    }

    #[test]
    fn agent_completion_returns_after_snapshot_update_and_can_run_again() {
        let _guard = TEST_ENV_LOCK.lock().unwrap();
        let root =
            std::env::temp_dir().join(format!("mikomai-chat-finish-{}", uuid::Uuid::new_v4()));
        let previous = std::env::var_os("MIKOMAI_DATA_DIR");
        std::env::set_var("MIKOMAI_DATA_DIR", &root);
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            // Missing ARP target deterministically asks the user, exercising
            // post-loop snapshot updates without an LLM or a real device.
            for _ in 0..2 {
                let input = CString::new("ARPでMAC 00:11:22:33:44:55を確認").unwrap();
                let empty = CString::new("").unwrap();
                let devices = CString::new("[]").unwrap();
                let response = unsafe {
                    super::mikomai_agent_chat_streaming(
                        input.as_ptr(),
                        empty.as_ptr(),
                        empty.as_ptr(),
                        empty.as_ptr(),
                        empty.as_ptr(),
                        devices.as_ptr(),
                        None,
                        Some(fake_watch_tool),
                        None,
                        std::ptr::null_mut(),
                    )
                };
                let answer = unsafe { CStr::from_ptr(response.message) }
                    .to_string_lossy()
                    .into_owned();
                let status = response.status;
                unsafe { mikomai_result_free(response) };
                sender.send((status, answer)).unwrap();
            }
        });
        for _ in 0..2 {
            let (status, answer) = receiver
                .recv_timeout(std::time::Duration::from_secs(15))
                .expect("agent completion must release its snapshot lock and return");
            assert_eq!(status, 0, "{answer}");
            assert!(answer.contains("登録機器がありません"), "{answer}");
        }
        if let Some(previous) = previous {
            std::env::set_var("MIKOMAI_DATA_DIR", previous);
        } else {
            std::env::remove_var("MIKOMAI_DATA_DIR");
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn assistant_chat_proceeds_to_model_when_docs_missing() {
        let message = CString::new("F220のVLAN設定方法を教えて").unwrap();
        let history = CString::new("").unwrap();
        let docs = CString::new("/nonexistent/documents/dir").unwrap();
        let index = CString::new("/nonexistent/knowledge/dir").unwrap();
        let response = unsafe {
            mikomai_assistant_chat(
                message.as_ptr(),
                history.as_ptr(),
                docs.as_ptr(),
                index.as_ptr(),
            )
        };
        // Should reach model check and report uninitialized model, NOT fail on documents missing
        assert_eq!(response.status, 1);
        let error = unsafe { CStr::from_ptr(response.message).to_string_lossy() };
        assert!(
            error.contains("モデルが未ロードです"),
            "got unexpected error: {error}"
        );
        unsafe { mikomai_result_free(response) };
    }

    #[test]
    fn device_registry_import_returns_metadata_without_secrets() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after the Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("mikomai-device-registry-{unique}.json"));
        let original = r#"[{"id":"connection-123","hostname":"router-1","ip":"192.0.2.1","port":"2222","type":"SSH","deviceType":"F220","password":"plaintext-secret","enablePassword":"enable-secret","passphrase":"key-secret"}]"#;
        fs::write(&path, original).expect("write device registry fixture");
        let path_arg = CString::new(path.to_string_lossy().as_bytes()).unwrap();
        let response = unsafe { mikomai_device_registry_read(path_arg.as_ptr()) };
        assert_eq!(response.status, 0);
        let json = unsafe { CStr::from_ptr(response.message).to_string_lossy() };
        assert!(json.contains("router-1"));
        assert!(json.contains("connection-123"));
        assert!(json.contains("192.0.2.1"));
        assert!(json.contains("2222"));
        assert!(json.contains("F220"));
        assert!(!json.contains("plaintext-secret"));
        assert!(!json.contains("enable-secret"));
        assert!(!json.contains("key-secret"));
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
        unsafe { mikomai_result_free(response) };
        fs::remove_file(path).ok();
    }

    #[test]
    fn device_registry_import_rejects_oversized_files() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after the Unix epoch")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("mikomai-device-registry-large-{unique}.json"));
        fs::write(&path, vec![b' '; 2 * 1024 * 1024 + 1]).expect("write large fixture");
        let path_arg = CString::new(path.to_string_lossy().as_bytes()).unwrap();
        let response = unsafe { mikomai_device_registry_read(path_arg.as_ptr()) };
        assert_eq!(response.status, 1);
        let error = unsafe { CStr::from_ptr(response.message).to_string_lossy() };
        assert!(error.contains("2 MiB"));
        unsafe { mikomai_result_free(response) };
        fs::remove_file(path).ok();
    }

    #[test]
    fn c_abi_reports_errors_answers_from_local_documents_and_frees_results() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after the Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("mikomai-ffi-test-{unique}"));
        let docs = root.join("docs");
        let index = root.join("index");
        fs::create_dir_all(&docs).expect("create temporary docs directory");
        fs::write(
            docs.join("acceptance.md"),
            "# F220 VLAN acceptance\n\nUnique answer marker: NATIVE-FFI-ANSWER-7319.",
        )
        .expect("write temporary knowledge document");

        let old_docs = std::env::var_os("MIKOMAI_DOCS_DIR");
        let old_index = std::env::var_os("MIKOMAI_KNOWLEDGE_DIR");
        std::env::set_var("MIKOMAI_DOCS_DIR", &docs);
        std::env::set_var("MIKOMAI_KNOWLEDGE_DIR", &index);

        unsafe {
            let null_result = mikomai_chat(std::ptr::null());
            assert_eq!(null_result.status, 1);
            assert!(CStr::from_ptr(null_result.message)
                .to_string_lossy()
                .contains("must not be null"));
            mikomai_result_free(null_result);

            let empty = CString::new("").unwrap();
            let empty_result = mikomai_chat(empty.as_ptr());
            assert_eq!(empty_result.status, 1);
            assert!(CStr::from_ptr(empty_result.message)
                .to_string_lossy()
                .contains("chat message is required"));
            mikomai_result_free(empty_result);

            let question = CString::new("F220 VLAN").unwrap();
            let answer = mikomai_chat(question.as_ptr());
            assert_eq!(
                answer.status,
                0,
                "{}",
                CStr::from_ptr(answer.message).to_string_lossy()
            );
            assert!(CStr::from_ptr(answer.message)
                .to_string_lossy()
                .contains("NATIVE-FFI-ANSWER-7319"));
            mikomai_result_free(answer);

            let empty_docs = root.join("empty-docs");
            let empty_index = root.join("empty-index");
            fs::create_dir_all(&empty_docs).expect("create unrelated docs directory");
            std::env::set_var("MIKOMAI_DOCS_DIR", &empty_docs);
            std::env::set_var("MIKOMAI_KNOWLEDGE_DIR", &empty_index);
            let configured = mikomai_chat_with_paths(
                question.as_ptr(),
                CString::new(docs.to_string_lossy().as_bytes())
                    .unwrap()
                    .as_ptr(),
                CString::new(index.to_string_lossy().as_bytes())
                    .unwrap()
                    .as_ptr(),
            );
            assert_eq!(configured.status, 0);
            assert!(CStr::from_ptr(configured.message)
                .to_string_lossy()
                .contains("NATIVE-FFI-ANSWER-7319"));
            mikomai_result_free(configured);
            std::env::set_var("MIKOMAI_DOCS_DIR", &docs);
            std::env::set_var("MIKOMAI_KNOWLEDGE_DIR", &index);

            let missing_docs =
                CString::new(root.join("missing-docs").to_string_lossy().as_bytes()).unwrap();
            let configured_missing = mikomai_chat_with_paths(
                question.as_ptr(),
                missing_docs.as_ptr(),
                CString::new(index.to_string_lossy().as_bytes())
                    .unwrap()
                    .as_ptr(),
            );
            assert_eq!(configured_missing.status, 1);
            assert!(CStr::from_ptr(configured_missing.message)
                .to_string_lossy()
                .contains("documents directory is missing"));
            mikomai_result_free(configured_missing);
        }

        restore_env("MIKOMAI_DOCS_DIR", old_docs);
        restore_env("MIKOMAI_KNOWLEDGE_DIR", old_index);
        fs::remove_dir_all(root).expect("remove temporary FFI test data");
    }

    #[test]
    fn real_model_smoke_when_local_gguf_is_configured() {
        let Ok(model_path) = std::env::var("MIKOMAI_TEST_GGUF") else {
            return;
        };
        let model_path = CString::new(model_path).unwrap();
        let docs = CString::new(std::env::var("MIKOMAI_TEST_DOCS_DIR").expect(
            "set MIKOMAI_TEST_DOCS_DIR to the source corpus when running a local model smoke test",
        ))
        .unwrap();
        let index = CString::new(std::env::var("MIKOMAI_TEST_INDEX_DIR").unwrap_or_else(|_| {
            std::env::temp_dir()
                .join("mikomai-ffi-real-model-smoke")
                .to_string_lossy()
                .into_owned()
        }))
        .unwrap();
        let prior_turn = "以前の確認では、F220のネットワーク設定を資料に沿って確認しました。ポート番号、VLAN ID、サブインターフェース番号は別の値として扱い、未指定値を実機の事実として補ってはいけません。必要な設定値が不足しているときは、具体的な値をユーザーへ確認します。\n";
        let history = CString::new(
            std::env::var("MIKOMAI_TEST_HISTORY").unwrap_or_else(|_| prior_turn.repeat(12)),
        )
        .unwrap();
        let question = CString::new("F220のVLAN設定方法を教えて").unwrap();
        unsafe {
            let loaded = mikomai_model_load(model_path.as_ptr());
            assert_eq!(
                loaded.status,
                0,
                "{}",
                CStr::from_ptr(loaded.message).to_string_lossy()
            );
            mikomai_result_free(loaded);
            let answer = mikomai_assistant_chat(
                question.as_ptr(),
                history.as_ptr(),
                docs.as_ptr(),
                index.as_ptr(),
            );
            let text = CStr::from_ptr(answer.message)
                .to_string_lossy()
                .into_owned();
            assert_eq!(answer.status, 0, "{text}");
            assert!(!text.trim().is_empty());
            println!("Real model F220 answer:\n{text}");
            mikomai_result_free(answer);
        }
    }

    #[test]
    fn test_tcp_connection_reports_success_and_failure() {
        use super::mikomai_test_tcp_connection;

        // Test with invalid / closed port or unreachable host
        let host = CString::new("127.0.0.1").unwrap();
        // Port 1 is typically closed/unassigned
        let response = unsafe { mikomai_test_tcp_connection(host.as_ptr(), 1, 200) };
        assert_eq!(response.status, 1);
        let error_msg = unsafe { CStr::from_ptr(response.message).to_string_lossy() };
        assert!(error_msg.contains("接続失敗"));
        unsafe { super::mikomai_result_free(response) };

        // Test with empty host
        let empty_host = CString::new("").unwrap();
        let empty_res = unsafe { mikomai_test_tcp_connection(empty_host.as_ptr(), 80, 200) };
        assert_eq!(empty_res.status, 1);
        unsafe { super::mikomai_result_free(empty_res) };

        // Test with null host
        let null_res = unsafe { mikomai_test_tcp_connection(std::ptr::null(), 80, 200) };
        assert_eq!(null_res.status, 1);
        unsafe { super::mikomai_result_free(null_res) };
    }

    #[test]
    fn assistant_chat_streaming_rejects_nulls_and_oversized() {
        use super::mikomai_assistant_chat_streaming;

        let res = unsafe {
            mikomai_assistant_chat_streaming(
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                None,
                std::ptr::null_mut(),
            )
        };
        assert_eq!(res.status, 1);
        unsafe { super::mikomai_result_free(res) };
    }

    #[test]
    fn test_set_inference_params() {
        use super::mikomai_set_inference_params;

        let res = unsafe { mikomai_set_inference_params(0.7, 1.2, 4096, 1024) };
        assert_eq!(res.status, 0);
        unsafe { super::mikomai_result_free(res) };
    }

    #[test]
    fn ffi_operation_plan_binds_hash_approval_and_single_execution_claim() {
        use super::{
            mikomai_operation_plan_approve, mikomai_operation_plan_begin,
            mikomai_operation_plan_create, mikomai_operation_plan_create_generic,
            mikomai_operation_plan_finish, mikomai_operation_plan_get,
        };
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let storage = std::env::temp_dir().join(format!("mikomai-operation-{nonce}.json"));
        let previous = std::env::var_os("MIKOMAI_OPERATION_PLANS_PATH");
        std::env::set_var("MIKOMAI_OPERATION_PLANS_PATH", &storage);
        let target = CString::new("edge-01").unwrap();
        let snapshot = CString::new(r#"{"id":"device-1","host":"192.0.2.10"}"#).unwrap();
        let commands = CString::new(r#"["hostname edge-01"]"#).unwrap();
        let rationale = CString::new("Approved maintenance window").unwrap();
        let created = unsafe {
            mikomai_operation_plan_create(
                target.as_ptr(),
                snapshot.as_ptr(),
                commands.as_ptr(),
                rationale.as_ptr(),
            )
        };
        assert_eq!(created.status, 0);
        let plan_json = unsafe {
            CStr::from_ptr(created.message)
                .to_string_lossy()
                .into_owned()
        };
        let plan: serde_json::Value = serde_json::from_str(&plan_json).unwrap();
        unsafe { super::mikomai_result_free(created) };
        let id = CString::new(plan["id"].as_str().unwrap()).unwrap();
        let hash_text = plan["planHash"].as_str().unwrap();
        let wrong_hash = CString::new("wrong-hash").unwrap();
        let correct_hash = CString::new(hash_text).unwrap();

        let unapproved_claim =
            unsafe { mikomai_operation_plan_begin(id.as_ptr(), correct_hash.as_ptr()) };
        assert_eq!(unapproved_claim.status, 1);
        unsafe { super::mikomai_result_free(unapproved_claim) };
        let wrong_approval =
            unsafe { mikomai_operation_plan_approve(id.as_ptr(), wrong_hash.as_ptr()) };
        assert_eq!(wrong_approval.status, 1);
        unsafe { super::mikomai_result_free(wrong_approval) };
        let fetched = unsafe { mikomai_operation_plan_get(id.as_ptr()) };
        assert_eq!(fetched.status, 0);
        unsafe { super::mikomai_result_free(fetched) };
        let approved =
            unsafe { mikomai_operation_plan_approve(id.as_ptr(), correct_hash.as_ptr()) };
        assert_eq!(approved.status, 0);
        unsafe { super::mikomai_result_free(approved) };
        let first_claim =
            unsafe { mikomai_operation_plan_begin(id.as_ptr(), correct_hash.as_ptr()) };
        assert_eq!(first_claim.status, 0);
        unsafe { super::mikomai_result_free(first_claim) };
        let duplicate_claim =
            unsafe { mikomai_operation_plan_begin(id.as_ptr(), correct_hash.as_ptr()) };
        assert_eq!(duplicate_claim.status, 1);
        unsafe { super::mikomai_result_free(duplicate_claim) };
        let before_execution = CString::new(format!("missing-{nonce}")).unwrap();
        let finish_before_execution =
            unsafe { mikomai_operation_plan_finish(before_execution.as_ptr(), 1) };
        assert_eq!(finish_before_execution.status, 1);
        unsafe { super::mikomai_result_free(finish_before_execution) };
        let completed = unsafe { mikomai_operation_plan_finish(id.as_ptr(), 1) };
        assert_eq!(completed.status, 0);
        unsafe { super::mikomai_result_free(completed) };
        let duplicate_finish = unsafe { mikomai_operation_plan_finish(id.as_ptr(), 1) };
        assert_eq!(duplicate_finish.status, 1);
        unsafe { super::mikomai_result_free(duplicate_finish) };

        let generic_tool = CString::new("network_config").unwrap();
        let generic_args = CString::new(r#"{"commands":["hostname edge-01"]}"#).unwrap();
        let generic_plan = unsafe {
            mikomai_operation_plan_create_generic(
                target.as_ptr(),
                generic_tool.as_ptr(),
                snapshot.as_ptr(),
                generic_args.as_ptr(),
                rationale.as_ptr(),
            )
        };
        assert_eq!(generic_plan.status, 0);
        let generic_json = unsafe {
            CStr::from_ptr(generic_plan.message)
                .to_string_lossy()
                .into_owned()
        };
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&generic_json).unwrap()["toolId"],
            "network_config"
        );
        unsafe { super::mikomai_result_free(generic_plan) };
        assert!(super::validate_native_config_command("hostname edge-01").is_ok());
        assert!(super::validate_native_config_command("hostname edge-01; reload").is_err());
        let _ = std::fs::remove_file(storage);
        restore_env("MIKOMAI_OPERATION_PLANS_PATH", previous);
    }

    fn restore_env(key: &str, value: Option<std::ffi::OsString>) {
        if let Some(value) = value {
            std::env::set_var(key, value);
        } else {
            std::env::remove_var(key);
        }
    }
}

fn prepare_attachments(question: &str, payload: &str) -> Result<String, String> {
    let (text, images) = mikomai_adapters::attachments::decode(payload)?;
    futures_lite::future::block_on(mikomai_core::vision::analyze_attachments(
        question,
        &text,
        images,
        &mikomai_adapters::local_llama::LocalVision,
    ))
}

#[no_mangle]
pub unsafe extern "C" fn mikomai_configure_vision(
    enabled: i32,
    projector_path: *const c_char,
) -> MikomaiResult {
    let caught = std::panic::catch_unwind(|| {
        let path = if projector_path.is_null() {
            None
        } else {
            let value = CStr::from_ptr(projector_path)
                .to_str()
                .map_err(|e| e.to_string())?;
            (!value.is_empty()).then(|| expand_tilde(Path::new(value)))
        };
        mikomai_adapters::local_llama::configure_vision(enabled != 0, path.as_deref())
    });
    match caught {
        Ok(Ok(message)) => result(0, message),
        Ok(Err(error)) => error_result(error),
        Err(_) => error_result("Vision configuration failed unexpectedly".into()),
    }
}

#[cfg(test)]
mod vision_integration_tests {
    use super::*;
    use base64::{engine::general_purpose::STANDARD, Engine};
    #[test]
    #[ignore = "requires MIKOMAI_TEST_VISION_MODEL and MIKOMAI_TEST_VISION_PROJECTOR"]
    fn real_image_reaches_core_through_native_chat_abi() {
        let model = std::env::var("MIKOMAI_TEST_VISION_MODEL").expect("vision model path");
        let projector =
            CString::new(std::env::var("MIKOMAI_TEST_VISION_PROJECTOR").expect("projector path"))
                .unwrap();
        load_local_model(&model).unwrap();
        let config = unsafe { mikomai_configure_vision(1, projector.as_ptr()) };
        assert_eq!(config.status, 0);
        unsafe { mikomai_result_free(config) };
        let config = unsafe { mikomai_set_inference_params(0.0, 1.1, 8192, 128) };
        assert_eq!(config.status, 0);
        unsafe { mikomai_result_free(config) };
        for (image, expected) in [
            (
                include_bytes!("../tests/fixtures/red-square.png").as_slice(),
                "赤",
            ),
            (
                include_bytes!("../tests/fixtures/blue-square.png").as_slice(),
                "青",
            ),
        ] {
            let attachments = CString::new(format!("{}{}", mikomai_adapters::attachments::WIRE_PREFIX,
                serde_json::json!({"text":"", "images":[{"name":"sample.png","mimeType":"image/png","base64":STANDARD.encode(image)}]}))).unwrap();
            let question =
                CString::new("この画像に描かれた四角の色を日本語で答えてください。").unwrap();
            let empty = CString::new("").unwrap();
            let result = unsafe {
                mikomai_assistant_chat_with_attachments(
                    question.as_ptr(),
                    empty.as_ptr(),
                    empty.as_ptr(),
                    empty.as_ptr(),
                    attachments.as_ptr(),
                )
            };
            let answer = consume_result(result).unwrap();
            assert!(
                answer.contains(expected),
                "expected {expected}, got {answer}"
            );
            println!("Vision answer (expected {expected}): {answer}");
        }
    }
}
