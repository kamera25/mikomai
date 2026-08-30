//! Append-only local audit records for operation planning and execution.
//!
//! Audit writes are best effort: failure to write a record is logged, but must
//! never turn a completed device operation into an ambiguous failure.

use crate::operations::OperationClass;
use chrono::{DateTime, Utc};
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
    let record = AuditRecord {
        id: uuid::Uuid::new_v4(),
        timestamp: Utc::now(),
        tool_id: tool_id.to_string(),
        target,
        operation_class,
        outcome: outcome.to_string(),
        details: redact(details),
    };

    let result = (|| -> Result<(), String> {
        let directory = app
            .path()
            .app_data_dir()
            .map_err(|e| e.to_string())?
            .join("audit");
        fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(directory.join("operations.ndjson"))
            .map_err(|e| e.to_string())?;
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
}
