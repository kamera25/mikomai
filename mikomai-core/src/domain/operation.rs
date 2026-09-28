use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationClass {
    ReadOnly,
    Change,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationStatus {
    Pending,
    Approved,
    Executing,
    Executed,
    Failed,
    Rejected,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OperationPlan {
    pub id: Uuid,
    pub tool_id: String,
    pub target: Option<String>,
    pub args: serde_json::Value,
    pub rationale: String,
    pub plan_hash: String,
    pub operation_class: OperationClass,
    pub status: OperationStatus,
}
impl OperationPlan {
    pub fn new(
        tool_id: impl Into<String>,
        target: Option<String>,
        args: serde_json::Value,
        rationale: impl Into<String>,
    ) -> Result<Self, String> {
        let tool_id = tool_id.into();
        let rationale = rationale.into();
        if tool_id.trim().is_empty() || rationale.trim().is_empty() {
            return Err("tool and rationale are required".into());
        }
        if classify_tool(&tool_id) != OperationClass::Change {
            return Err("read-only tools do not require a change plan".into());
        }
        let id = Uuid::new_v4();
        let plan_hash = hash_plan(&id, &tool_id, &target, &args, &rationale);
        Ok(Self {
            id,
            tool_id,
            target,
            args,
            rationale,
            plan_hash,
            operation_class: OperationClass::Change,
            status: OperationStatus::Pending,
        })
    }
}
pub struct ChangePlanner;
impl ChangePlanner {
    pub fn create(
        tool_id: String,
        target: Option<String>,
        args: serde_json::Value,
        rationale: String,
    ) -> Result<OperationPlan, String> {
        OperationPlan::new(tool_id, target, args, rationale)
    }
}
pub struct OperationGate;
impl OperationGate {
    pub fn authorize(plan: &OperationPlan, approved_hash: Option<&str>) -> Result<(), String> {
        if plan.operation_class != OperationClass::Change || plan.tool_id.trim().is_empty() {
            return Err("operation plan is incomplete".into());
        }
        if !matches!(
            plan.status,
            OperationStatus::Approved | OperationStatus::Executing
        ) {
            return Err("operation plan is not approved".into());
        }
        if approved_hash == Some(plan.plan_hash.as_str()) {
            Ok(())
        } else {
            Err("operation plan is not approved".into())
        }
    }
    pub fn approve(plan: &mut OperationPlan, hash: &str) -> Result<(), String> {
        if plan.status != OperationStatus::Pending || plan.plan_hash != hash {
            return Err("invalid operation approval".into());
        }
        plan.status = OperationStatus::Approved;
        Ok(())
    }
}
fn classify_tool(tool: &str) -> OperationClass {
    if ["network_config", "configure", "write_config", "reload"]
        .iter()
        .any(|name| tool.eq_ignore_ascii_case(name))
    {
        OperationClass::Change
    } else {
        OperationClass::ReadOnly
    }
}
fn hash_plan(
    id: &Uuid,
    tool: &str,
    target: &Option<String>,
    args: &serde_json::Value,
    rationale: &str,
) -> String {
    let bytes = serde_json::to_vec(&serde_json::json!({"id": id, "toolId": tool, "target": target, "args": args, "rationale": rationale})).unwrap_or_default();
    format!("{:x}", Sha256::digest(bytes))
}
