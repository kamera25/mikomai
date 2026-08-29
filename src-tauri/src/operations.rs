//! Safety boundary for every operation that can touch a network device.
//!
//! The LLM may propose an operation, but this module is the authority that
//! classifies it and decides whether it can run.  Keep this policy independent
//! of prompts and UI code so callers cannot bypass it by choosing another MCP
//! entry point.

use crate::mcp::ToolKind;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

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
    pub operation_class: OperationClass,
    pub approval_status: ApprovalStatus,
}

pub struct OperationStore {
    plans: Mutex<HashMap<uuid::Uuid, OperationPlan>>,
}

impl OperationStore {
    pub fn new() -> Self {
        Self { plans: Mutex::new(HashMap::new()) }
    }

    fn insert(&self, plan: OperationPlan) -> Result<OperationPlan, String> {
        self.plans.lock().map_err(|_| "Operation plan store is unavailable".to_string())?
            .insert(plan.id, plan.clone());
        Ok(plan)
    }

    pub fn approve(&self, id: uuid::Uuid) -> Result<OperationPlan, String> {
        let mut plans = self.plans.lock().map_err(|_| "Operation plan store is unavailable".to_string())?;
        let plan = plans.get_mut(&id).ok_or("Operation plan was not found")?;
        if plan.approval_status != ApprovalStatus::Pending {
            return Err("Only a pending operation plan can be approved".to_string());
        }
        plan.approval_status = ApprovalStatus::Approved;
        Ok(plan.clone())
    }

    pub fn take_approved(&self, id: uuid::Uuid) -> Result<OperationPlan, String> {
        let mut plans = self.plans.lock().map_err(|_| "Operation plan store is unavailable".to_string())?;
        let plan = plans.get_mut(&id).ok_or("Operation plan was not found")?;
        if plan.approval_status != ApprovalStatus::Approved {
            return Err("Operation plan has not been approved".to_string());
        }
        plan.approval_status = ApprovalStatus::Executing;
        Ok(plan.clone())
    }

    pub fn mark_executed(&self, id: uuid::Uuid) {
        if let Ok(mut plans) = self.plans.lock() {
            if let Some(plan) = plans.get_mut(&id) {
                plan.approval_status = ApprovalStatus::Executed;
            }
        }
    }
}

impl Default for OperationStore {
    fn default() -> Self { Self::new() }
}

#[tauri::command]
pub fn create_operation_plan(
    tool_id: String,
    target: Option<String>,
    args: serde_json::Value,
    rationale: String,
    store: tauri::State<'_, OperationStore>,
) -> Result<OperationPlan, String> {
    if classify_tool(&tool_id) != OperationClass::Change {
        return Err("Read-only tools do not require an operation plan".to_string());
    }
    if rationale.trim().is_empty() {
        return Err("An operation rationale is required".to_string());
    }
    store.insert(OperationPlan {
        id: uuid::Uuid::new_v4(),
        created_at: chrono::Utc::now(),
        tool_id,
        target,
        args,
        rationale,
        operation_class: OperationClass::Change,
        approval_status: ApprovalStatus::Pending,
    })
}

#[tauri::command]
pub fn approve_operation_plan(
    id: uuid::Uuid,
    store: tauri::State<'_, OperationStore>,
) -> Result<OperationPlan, String> {
    store.approve(id)
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
        let plan = OperationPlan {
            id: uuid::Uuid::new_v4(), created_at: chrono::Utc::now(), tool_id: "network_config".into(),
            target: Some("edge-01".into()), args: serde_json::json!({"commands": ["description managed"]}),
            rationale: "Approved maintenance window".into(), operation_class: OperationClass::Change,
            approval_status: ApprovalStatus::Pending,
        };
        let plan = store.insert(plan).unwrap();
        assert!(store.take_approved(plan.id).is_err());
        store.approve(plan.id).unwrap();
        assert_eq!(store.take_approved(plan.id).unwrap().approval_status, ApprovalStatus::Executing);
        assert!(store.take_approved(plan.id).is_err());
    }
}
