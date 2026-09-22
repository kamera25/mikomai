//! Append-only local audit records for operation planning and execution.
//!
//! Audit writes are best effort: failure to write a record is logged, but must
//! never turn a completed device operation into an ambiguous failure.

use crate::operations::OperationClass;
use chrono::{DateTime, Utc};
use ring::digest::{digest, SHA256};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use tauri::Manager;

const SENSITIVE_KEYS: &[&str] = &[
    "password",
    "pass",
    "secret",
    "enable_password",
    "enablepassword",
    "passphrase",
    "token",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditRecord {
    pub id: uuid::Uuid,
    pub timestamp: DateTime<Utc>,
    pub tool_id: String,
    pub target: Option<String>,
    pub operation_class: OperationClass,
    pub outcome: String,
    pub details: serde_json::Value,
    #[serde(default)]
    pub integrity_hash: String,
}

pub fn compute_record_hash(
    id: &uuid::Uuid,
    timestamp: &DateTime<Utc>,
    tool_id: &str,
    target: Option<&str>,
    operation_class: &OperationClass,
    outcome: &str,
    details: &serde_json::Value,
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
    let d = digest(&SHA256, payload.as_bytes());
    hex_encode(d.as_ref())
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

pub fn redact(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(values) => values
            .iter()
            .map(|(key, value)| {
                let hidden = SENSITIVE_KEYS
                    .iter()
                    .any(|sensitive| key.eq_ignore_ascii_case(sensitive));
                (
                    key.clone(),
                    if hidden {
                        serde_json::Value::String("[REDACTED]".into())
                    } else {
                        redact(value)
                    },
                )
            })
            .collect(),
        serde_json::Value::Array(values) => values.iter().map(redact).collect(),
        value => value.clone(),
    }
}

pub fn record(
    app: &tauri::AppHandle,
    tool_id: &str,
    target: Option<String>,
    operation_class: OperationClass,
    outcome: &str,
    details: &serde_json::Value,
) {
    let id = uuid::Uuid::new_v4();
    let timestamp = Utc::now();
    let redacted_details = redact(details);
    let integrity_hash = compute_record_hash(
        &id,
        &timestamp,
        tool_id,
        target.as_deref(),
        &operation_class,
        outcome,
        &redacted_details,
    );

    let record = AuditRecord {
        id,
        timestamp,
        tool_id: tool_id.to_string(),
        target,
        operation_class,
        outcome: outcome.to_string(),
        details: redacted_details,
        integrity_hash,
    };

    let result = (|| -> Result<(), String> {
        let directory = app
            .path()
            .app_data_dir()
            .map_err(|e| e.to_string())?
            .join("audit");
        fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        let log_path = directory.join("operations.ndjson");
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .map_err(|e| e.to_string())?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(metadata) = fs::metadata(&log_path) {
                let mut perms = metadata.permissions();
                perms.set_mode(0o600);
                let _ = fs::set_permissions(&log_path, perms);
            }
        }

        let line = serde_json::to_string(&record).map_err(|e| e.to_string())?;
        writeln!(file, "{line}").map_err(|e| e.to_string())
    })();

    if let Err(error) = result {
        log::error!(
            "Could not write audit record for tool '{}' ({}): {}",
            tool_id,
            outcome,
            error
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_sensitive_values_recursively() {
        let input = serde_json::json!({
            "password": "not-for-logs",
            "nested": { "token": "also-secret", "port": 22 },
            "commands": ["show version"]
        });
        assert_eq!(
            redact(&input),
            serde_json::json!({
                "password": "[REDACTED]",
                "nested": { "token": "[REDACTED]", "port": 22 },
                "commands": ["show version"]
            })
        );
    }

    #[test]
    fn test_audit_record_integrity_hash() {
        let id = uuid::Uuid::new_v4();
        let timestamp = Utc::now();
        let details = serde_json::json!({ "cmd": "show vlan" });
        let hash1 = compute_record_hash(
            &id,
            &timestamp,
            "tool_1",
            Some("192.168.1.1"),
            &OperationClass::ReadOnly,
            "success",
            &details,
        );
        let hash2 = compute_record_hash(
            &id,
            &timestamp,
            "tool_1",
            Some("192.168.1.1"),
            &OperationClass::ReadOnly,
            "success",
            &details,
        );
        assert_eq!(hash1, hash2);
        assert!(!hash1.is_empty());

        // Tampering any field must change the hash
        let hash_tampered = compute_record_hash(
            &id,
            &timestamp,
            "tool_1",
            Some("192.168.1.2"), // tampered target
            &OperationClass::ReadOnly,
            "success",
            &details,
        );
        assert_ne!(hash1, hash_tampered);
    }
}
