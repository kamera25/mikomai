//! Safety boundary for every operation that can touch a network device.
//!
//! The LLM may propose an operation, but this module is the authority that
//! classifies it and decides whether it can run.  Keep this policy independent
//! of prompts and UI code so callers cannot bypass it by choosing another MCP
//! entry point.

use crate::mcp::ToolKind;
use ring::digest::{digest, SHA256};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::Manager;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationClass {
    ReadOnly,
    Change,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Executing,
    Executed,
    Failed,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationPlan {
    pub id: uuid::Uuid,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub tool_id: String,
    pub target: Option<String>,
    pub args: serde_json::Value,
    pub rationale: String,
    /// Binds user approval to the exact immutable plan contents.
    pub plan_hash: String,
    pub operation_class: OperationClass,
    pub approval_status: ApprovalStatus,
}

/// A deterministic boundary between an LLM-proposed change and a change that
/// may be approved or executed. The planner owns normalization and hashing;
/// callers cannot modify a plan in place after it has been presented.
pub struct ChangePlanner;

impl ChangePlanner {
    pub fn create(
        tool_id: String,
        target: Option<String>,
        args: serde_json::Value,
        rationale: String,
    ) -> Result<OperationPlan, String> {
        if classify_tool(&tool_id) != OperationClass::Change {
            return Err("Read-only tools do not require a change plan".to_string());
        }
        if rationale.trim().is_empty() {
            return Err("A change rationale is required".to_string());
        }

        let id = uuid::Uuid::new_v4();
        let created_at = chrono::Utc::now();
        let plan_hash = plan_hash(&id, &tool_id, &target, &args, &rationale, &created_at)?;
        Ok(OperationPlan {
            id,
            created_at,
            tool_id,
            target,
            args,
            rationale,
            plan_hash,
            operation_class: OperationClass::Change,
            approval_status: ApprovalStatus::Pending,
        })
    }
}

fn plan_hash(
    id: &uuid::Uuid,
    tool_id: &str,
    target: &Option<String>,
    args: &serde_json::Value,
    rationale: &str,
    created_at: &chrono::DateTime<chrono::Utc>,
) -> Result<String, String> {
    let canonical = serde_json::to_vec(&serde_json::json!({
        "id": id,
        "toolId": tool_id,
        "target": target,
        "args": args,
        "rationale": rationale,
        "createdAt": created_at,
    }))
    .map_err(|e| format!("Failed to hash change plan: {e}"))?;
    Ok(digest(&SHA256, &canonical)
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

pub struct OperationStore {
    plans: Mutex<HashMap<uuid::Uuid, OperationPlan>>,
    storage_path: Option<PathBuf>,
}

impl OperationStore {
    pub fn new() -> Self {
        Self {
            plans: Mutex::new(HashMap::new()),
            storage_path: None,
        }
    }

    /// Restores plans so a pending approval is not silently lost when the app
    /// restarts.  A corrupt cache is ignored rather than preventing startup.
    pub fn load(app: &tauri::AppHandle) -> Result<Self, String> {
        let directory = app
            .path()
            .app_data_dir()
            .map_err(|error| format!("Failed to resolve operation-plan storage: {error}"))?;
        fs::create_dir_all(&directory)
            .map_err(|error| format!("Failed to create operation-plan storage: {error}"))?;
        let storage_path = directory.join("operation-plans.json");
        let plans = match fs::read_to_string(&storage_path) {
            Ok(contents) => match serde_json::from_str::<Vec<OperationPlan>>(&contents) {
                Ok(plans) => plans,
                Err(error) => {
                    log::warn!("Ignoring corrupt operation-plan storage: {error}");
                    Vec::new()
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(error) => return Err(format!("Failed to read operation plans: {error}")),
        };
        Ok(Self {
            plans: Mutex::new(plans.into_iter().map(|plan| (plan.id, plan)).collect()),
            storage_path: Some(storage_path),
        })
    }

    fn persist(&self, plans: &HashMap<uuid::Uuid, OperationPlan>) -> Result<(), String> {
        let Some(path) = &self.storage_path else {
            return Ok(());
        };
        let serialized = serde_json::to_string_pretty(
            &plans.values().cloned().collect::<Vec<OperationPlan>>(),
        )
        .map_err(|error| format!("Failed to serialize operation plans: {error}"))?;
        let temporary_path = path.with_extension("json.tmp");
        fs::write(&temporary_path, serialized)
            .map_err(|error| format!("Failed to write operation plans: {error}"))?;
        fs::rename(&temporary_path, path)
            .map_err(|error| format!("Failed to save operation plans: {error}"))
    }

    fn insert(&self, plan: OperationPlan) -> Result<OperationPlan, String> {
        let mut plans = self
            .plans
            .lock()
            .map_err(|_| "Operation plan store is unavailable".to_string())?;
        plans.insert(plan.id, plan.clone());
        self.persist(&plans)?;
        Ok(plan)
    }

    pub fn get(&self, id: uuid::Uuid) -> Result<OperationPlan, String> {
        self.plans
            .lock()
            .map_err(|_| "Operation plan store is unavailable".to_string())?
            .get(&id)
            .cloned()
            .ok_or("Operation plan was not found".to_string())
    }

    pub fn approve(&self, id: uuid::Uuid, plan_hash: &str) -> Result<OperationPlan, String> {
        let mut plans = self
            .plans
            .lock()
            .map_err(|_| "Operation plan store is unavailable".to_string())?;
        let plan = plans.get_mut(&id).ok_or("Operation plan was not found")?;
        if plan.approval_status != ApprovalStatus::Pending {
            return Err("Only a pending operation plan can be approved".to_string());
        }
        if plan.plan_hash != plan_hash {
            return Err("Change plan has changed or the approval hash is invalid".to_string());
        }
        plan.approval_status = ApprovalStatus::Approved;
        let updated = plan.clone();
        self.persist(&plans)?;
        Ok(updated)
    }

    pub fn take_approved(&self, id: uuid::Uuid, plan_hash: &str) -> Result<OperationPlan, String> {
        let mut plans = self
            .plans
            .lock()
            .map_err(|_| "Operation plan store is unavailable".to_string())?;
        let plan = plans.get_mut(&id).ok_or("Operation plan was not found")?;
        if plan.approval_status != ApprovalStatus::Approved {
            return Err("Operation plan has not been approved".to_string());
        }
        if plan.plan_hash != plan_hash {
            return Err("Change plan hash does not match the approved plan".to_string());
        }
        plan.approval_status = ApprovalStatus::Executing;
        let updated = plan.clone();
        self.persist(&plans)?;
        Ok(updated)
    }

    pub fn mark_executed(&self, id: uuid::Uuid) -> Result<(), String> {
        let mut plans = self
            .plans
            .lock()
            .map_err(|_| "Operation plan store is unavailable".to_string())?;
        if let Some(plan) = plans.get_mut(&id) {
            plan.approval_status = ApprovalStatus::Executed;
        }
        self.persist(&plans)
    }

    pub fn mark_failed(&self, id: uuid::Uuid) -> Result<(), String> {
        let mut plans = self
            .plans
            .lock()
            .map_err(|_| "Operation plan store is unavailable".to_string())?;
        if let Some(plan) = plans.get_mut(&id) {
            plan.approval_status = ApprovalStatus::Failed;
        }
        self.persist(&plans)
    }
}

impl Default for OperationStore {
    fn default() -> Self {
        Self::new()
    }
}

#[tauri::command]
pub fn create_operation_plan(
    tool_id: String,
    target: Option<String>,
    args: serde_json::Value,
    rationale: String,
    store: tauri::State<'_, OperationStore>,
) -> Result<OperationPlan, String> {
    store.insert(ChangePlanner::create(tool_id, target, args, rationale)?)
}

/// Creates a change plan from a registered device name. Credentials are
/// resolved only at execution time and are never stored in the plan or
/// exposed to the webview.
#[tauri::command]
pub async fn create_network_config_operation_plan(
    app: tauri::AppHandle,
    device_name: String,
    commands: Vec<String>,
    rationale: String,
    store: tauri::State<'_, OperationStore>,
) -> Result<OperationPlan, String> {
    if device_name.trim().is_empty() {
        return Err("A registered target device is required".to_string());
    }
    if commands.iter().all(|command| command.trim().is_empty()) {
        return Err("At least one configuration command is required".to_string());
    }
    // Validate the target while intentionally keeping its connection details
    // out of the persisted plan.
    crate::mcp::fetch::fetch_base::resolve_device_config(&app, &device_name).await?;
    store.insert(ChangePlanner::create(
        "network_config".to_string(),
        Some(device_name.clone()),
        serde_json::json!({ "deviceName": device_name, "commands": commands }),
        rationale,
    )?)
}

/// Read-only access for the AgentLoop and UI. This never authorizes a change.
#[tauri::command]
pub fn get_operation_plan(
    id: uuid::Uuid,
    store: tauri::State<'_, OperationStore>,
) -> Result<OperationPlan, String> {
    store.get(id)
}

#[tauri::command]
pub fn approve_operation_plan(
    id: uuid::Uuid,
    plan_hash: String,
    store: tauri::State<'_, OperationStore>,
) -> Result<OperationPlan, String> {
    store.approve(id, &plan_hash)
}

/// The only execution entry point for changes created by ChangePlanner.
/// It accepts an immutable, hash-bound plan rather than LLM-generated commands.
#[tauri::command]
pub async fn execute_approved_operation_plan(
    app: tauri::AppHandle,
    id: uuid::Uuid,
    plan_hash: String,
    store: tauri::State<'_, OperationStore>,
) -> Result<crate::network::CommandResult, String> {
    let plan = store.take_approved(id, &plan_hash)?;
    if plan.tool_id != "network_config" {
        let _ = store.mark_failed(id);
        return Err(format!("Unsupported change-plan tool: {}", plan.tool_id));
    }
    let device_name = plan
        .args
        .get("deviceName")
        .or_else(|| plan.args.get("device_name"))
        .cloned()
        .or_else(|| plan.target.clone().map(serde_json::Value::String))
        .ok_or_else(|| "Change plan is missing device name".to_string())
        .and_then(|value| {
            serde_json::from_value::<String>(value)
                .map_err(|_| "Change plan has an invalid device name".to_string())
        })
        .map_err(|error| {
            let _ = store.mark_failed(id);
            error
        })?;
    let device = crate::mcp::fetch::fetch_base::resolve_device_config(&app, &device_name)
        .await
        .map_err(|error| {
            let _ = store.mark_failed(id);
            error
        })?;
    let commands: Vec<String> = plan
        .args
        .get("commands")
        .cloned()
        .ok_or_else(|| "Change plan is missing commands".to_string())
        .and_then(|value| {
            serde_json::from_value::<Vec<String>>(value)
                .map_err(|_| "Change plan has invalid commands".to_string())
        })
        .map_err(|error| {
            let _ = store.mark_failed(id);
            error
        })?;

    // Dry-run is mandatory for a ChangePlan. A planner or agent cannot opt
    // out by changing a prompt or omitting a UI flag.
    let dry_run = crate::network::SidecarNetmikoWrapper::new(&app)
        .execute_dry_run(&device, commands.clone())
        .await
        .map_err(|error| {
            let _ = store.mark_failed(id);
            format!("Change plan dry-run failed: {error}")
        })?;
    let dry_run_errors: Vec<_> = dry_run.results.iter().filter(|line| !line.ok).collect();
    if !dry_run.success || !dry_run_errors.is_empty() {
        let _ = store.mark_failed(id);
        return Err(format!(
            "Change plan dry-run rejected execution: {}",
            dry_run_errors
                .iter()
                .map(|line| format!(
                    "{}: {}",
                    line.line,
                    line.error.as_deref().unwrap_or("validation failed")
                ))
                .collect::<Vec<_>>()
                .join("; ")
        ));
    }
    let result = crate::network::network_config(app, device, commands)
        .await
        .map_err(|error| error.to_string())?;
    if result.success {
        store.mark_executed(id)?;
    } else {
        store.mark_failed(id)?;
    }
    Ok(result)
}

impl OperationClass {
    pub fn requires_approval(self) -> bool {
        matches!(self, Self::Change)
    }
}

/// Returns the least-privileged classification for a registered tool.
/// Unknown tool IDs are intentionally treated as changes and cannot execute.
pub fn classify_tool(tool_id: &str) -> OperationClass {
    match tool_id.parse::<ToolKind>() {
        Ok(kind) if kind.is_read_only() => OperationClass::ReadOnly,
        _ => OperationClass::Change,
    }
}

/// Phase 1/2 execution policy.  Device-changing tools are deliberately
/// unavailable until they are routed through the approval and audit workflow.
pub fn allow_unattended_execution(tool_id: &str) -> Result<(), String> {
    if classify_tool(tool_id) == OperationClass::ReadOnly {
        Ok(())
    } else {
        Err(format!(
            "Tool '{tool_id}' changes device or file state and requires an approved operation plan. \
             Direct execution is disabled."
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permits_read_only_diagnostics() {
        assert_eq!(classify_tool("self_network_ping"), OperationClass::ReadOnly);
        assert!(allow_unattended_execution("fetch_config").is_ok());
    }

    #[test]
    fn blocks_change_and_unknown_tools() {
        assert_eq!(classify_tool("network_config"), OperationClass::Change);
        assert!(allow_unattended_execution("network_config").is_err());
        assert!(allow_unattended_execution("not_a_tool").is_err());
    }

    #[test]
    fn approved_plan_can_only_be_taken_once() {
        let store = OperationStore::new();
        let plan = ChangePlanner::create(
            "network_config".into(),
            Some("edge-01".into()),
            serde_json::json!({"commands": ["description managed"]}),
            "Approved maintenance window".into(),
        )
        .unwrap();
        let plan = store.insert(plan).unwrap();
        assert!(store.take_approved(plan.id, &plan.plan_hash).is_err());
        assert!(store.approve(plan.id, "wrong-hash").is_err());
        store.approve(plan.id, &plan.plan_hash).unwrap();
        assert_eq!(
            store
                .take_approved(plan.id, &plan.plan_hash)
                .unwrap()
                .approval_status,
            ApprovalStatus::Executing
        );
        assert!(store.take_approved(plan.id, &plan.plan_hash).is_err());
    }

    #[test]
    fn persists_approval_status_before_execution() {
        let directory = std::env::temp_dir().join(format!("mikomai-operation-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let storage_path = directory.join("operation-plans.json");
        let store = OperationStore {
            plans: Mutex::new(HashMap::new()),
            storage_path: Some(storage_path.clone()),
        };
        let plan = store
            .insert(
                ChangePlanner::create(
                    "network_config".into(),
                    Some("router-1".into()),
                    serde_json::json!({"commands": ["hostname router-1"]}),
                    "Set the device hostname".into(),
                )
                .unwrap(),
            )
            .unwrap();
        store.approve(plan.id, &plan.plan_hash).unwrap();

        let saved: Vec<OperationPlan> = serde_json::from_str(&std::fs::read_to_string(&storage_path).unwrap()).unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].approval_status, ApprovalStatus::Approved);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
