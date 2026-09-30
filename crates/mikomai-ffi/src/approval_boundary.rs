//! Private approval-state adapter for exercising the shared policy at the FFI boundary.

use mikomai_core::domain::ChangePlanner;
use mikomai_core::{OperationGate, OperationPlan};

pub(crate) struct ApprovalBoundary(OperationPlan);

impl ApprovalBoundary {
    pub(crate) fn new(
        tool_id: &str,
        target: Option<String>,
        args: serde_json::Value,
        rationale: &str,
    ) -> Result<Self, String> {
        Ok(Self(ChangePlanner::create(
            tool_id.to_owned(),
            target,
            args,
            rationale.to_owned(),
        )?))
    }

    pub(crate) fn plan_hash(&self) -> &str {
        &self.0.plan_hash
    }

    pub(crate) fn approve(&mut self, hash: &str) -> Result<(), String> {
        OperationGate::approve(&mut self.0, hash)
    }

    pub(crate) fn begin_execution(&mut self, hash: &str) -> Result<(), String> {
        OperationGate::begin_execution(&mut self.0, hash)
    }

    pub(crate) fn finish_execution(&mut self, succeeded: bool) -> Result<(), String> {
        OperationGate::finish_execution(&mut self.0, succeeded)
    }
}

#[cfg(test)]
mod tests {
    use super::ApprovalBoundary;
    use mikomai_core::OperationStatus;

    #[test]
    fn ffi_boundary_requires_matching_approval_and_claims_execution_once() {
        let mut boundary = ApprovalBoundary::new(
            "network_config",
            Some("edge-01".into()),
            serde_json::json!({"commands": ["hostname edge-01"]}),
            "Approved maintenance window",
        )
        .unwrap();
        let hash = boundary.plan_hash().to_owned();

        assert!(boundary.begin_execution(&hash).is_err());
        assert!(boundary.approve("incorrect").is_err());
        boundary.approve(&hash).unwrap();
        assert!(boundary.begin_execution("incorrect").is_err());
        boundary.begin_execution(&hash).unwrap();
        assert!(boundary.begin_execution(&hash).is_err());
        boundary.finish_execution(true).unwrap();
        assert!(boundary.finish_execution(false).is_err());
        assert_eq!(boundary.0.status, OperationStatus::Executed);
    }
}
