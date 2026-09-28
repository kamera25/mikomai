//! Operation command DTOs stay at the IPC boundary; authorization remains in
//! the core OperationGate and the existing operation store.
use mikomai_core::OperationPlan;
use serde::Serialize;

pub use crate::operations::{
    approve_operation_plan, create_network_config_operation_plan, create_operation_plan,
    execute_approved_operation_plan, get_operation_plan,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationDto {
    pub id: String,
    pub tool_id: String,
    pub target: Option<String>,
    pub plan_hash: String,
}

impl From<OperationPlan> for OperationDto {
    fn from(plan: OperationPlan) -> Self {
        Self {
            id: plan.id.to_string(),
            tool_id: plan.tool_id,
            target: plan.target,
            plan_hash: plan.plan_hash,
        }
    }
}
