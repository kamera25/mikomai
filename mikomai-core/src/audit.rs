//! Portable, redacted operation audit record contract.

use crate::OperationClass;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

const SENSITIVE_KEYS: &[&str] = &[
    "password",
    "pass",
    "secret",
    "enable_password",
    "enablepassword",
    "passphrase",
    "token",
    "username",
    "user",
    "privatekey",
    "credentials",
    "community",
    "authkey",
    "sharedsecret",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuditRecord {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub tool_id: String,
    pub target: Option<String>,
    pub operation_class: OperationClass,
    pub outcome: String,
    pub details: Value,
    #[serde(default)]
    pub integrity_hash: String,
}

pub fn redact(value: &Value) -> Value {
    match value {
        Value::Object(values) => values
            .iter()
            .map(|(key, value)| {
                let normalized = key.to_ascii_lowercase().replace(['_', '-', ' '], "");
                let hidden = SENSITIVE_KEYS
                    .iter()
                    .any(|sensitive| sensitive.replace('_', "") == normalized);
                (
                    key.clone(),
                    if hidden {
                        Value::String("[REDACTED]".into())
                    } else {
                        redact(value)
                    },
                )
            })
            .collect(),
        Value::Array(values) => values.iter().map(redact).collect(),
        other => other.clone(),
    }
}

pub fn record(
    tool_id: impl Into<String>,
    target: Option<String>,
    operation_class: OperationClass,
    outcome: impl Into<String>,
    details: &Value,
) -> AuditRecord {
    let id = Uuid::new_v4();
    let timestamp = Utc::now();
    let tool_id = tool_id.into();
    let outcome = outcome.into();
    let details = redact(details);
    let integrity_hash = compute_record_hash(
        &id,
        &timestamp,
        &tool_id,
        target.as_deref(),
        &operation_class,
        &outcome,
        &details,
    );
    AuditRecord {
        id,
        timestamp,
        tool_id,
        target,
        operation_class,
        outcome,
        details,
        integrity_hash,
    }
}

pub fn compute_record_hash(
    id: &Uuid,
    timestamp: &DateTime<Utc>,
    tool_id: &str,
    target: Option<&str>,
    operation_class: &OperationClass,
    outcome: &str,
    details: &Value,
) -> String {
    let payload = format!(
        "{}:{}:{}:{}:{:?}:{}:{}",
        id,
        timestamp.to_rfc3339(),
        tool_id,
        target.unwrap_or(""),
        operation_class,
        outcome,
        details
    );
    Sha256::digest(payload.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub fn verify(record: &AuditRecord) -> bool {
    record.integrity_hash
        == compute_record_hash(
            &record.id,
            &record.timestamp,
            &record.tool_id,
            record.target.as_deref(),
            &record.operation_class,
            &record.outcome,
            &record.details,
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recursively_redacts_credentials_and_hash_detects_tampering() {
        let details = serde_json::json!({
            "password": "secret",
            "nested": {"enablePassword": "secret-too", "port": 22},
            "commands": ["show version"]
        });
        let mut audit = record(
            "network_show",
            Some("router".into()),
            OperationClass::ReadOnly,
            "success",
            &details,
        );
        assert_eq!(audit.details["password"], "[REDACTED]");
        assert_eq!(audit.details["nested"]["enablePassword"], "[REDACTED]");
        assert!(verify(&audit));
        audit.target = Some("changed-router".into());
        assert!(!verify(&audit));
    }
}
