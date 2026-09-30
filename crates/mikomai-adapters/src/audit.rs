//! Durable append-only operation audit storage.

use mikomai_core::audit::{verify, AuditRecord};
use std::{
    fs::{self, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

#[derive(Clone)]
pub struct FileAuditLog {
    path: PathBuf,
    write_lock: Arc<Mutex<()>>,
}

impl FileAuditLog {
    pub fn at(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            write_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn append(&self, record: &AuditRecord) -> Result<(), String> {
        let _guard = self
            .write_lock
            .lock()
            .map_err(|_| "audit write lock is poisoned")?;
        let parent = self
            .path
            .parent()
            .ok_or("audit path has no parent directory")?;
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create audit directory: {error}"))?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|error| format!("cannot open operation audit log: {error}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = file
                .metadata()
                .map_err(|error| error.to_string())?
                .permissions();
            permissions.set_mode(0o600);
            file.set_permissions(permissions)
                .map_err(|error| format!("cannot protect audit log: {error}"))?;
        }
        let bytes = serde_json::to_vec(record).map_err(|error| error.to_string())?;
        file.write_all(&bytes)
            .and_then(|_| file.write_all(b"\n"))
            .map_err(|error| format!("cannot append operation audit record: {error}"))
    }

    pub fn list(&self, limit: usize) -> Result<Vec<AuditRecord>, String> {
        let file = match OpenOptions::new().read(true).open(&self.path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(format!("cannot read operation audit log: {error}")),
        };
        let mut records = Vec::new();
        for line in BufReader::new(file).lines() {
            let line = line.map_err(|error| format!("cannot read audit record: {error}"))?;
            if let Ok(record) = serde_json::from_str::<AuditRecord>(&line) {
                if verify(&record) {
                    records.push(record);
                }
            }
        }
        records.reverse();
        records.truncate(limit.min(10_000));
        Ok(records)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mikomai_core::{audit, OperationClass};

    #[test]
    fn append_lists_newest_first_and_skips_tampered_records() {
        let root = std::env::temp_dir().join(format!("mikomai-audit-{}", uuid::Uuid::new_v4()));
        let store = FileAuditLog::at(root.join("operations.ndjson"));
        let first = audit::record(
            "network_show",
            None,
            OperationClass::ReadOnly,
            "success",
            &serde_json::json!({"command":"show version"}),
        );
        let second = audit::record(
            "network_config",
            Some("R1".into()),
            OperationClass::Change,
            "approved",
            &serde_json::json!({"commands":["show version"]}),
        );
        store.append(&first).unwrap();
        store.append(&second).unwrap();
        let listed = store.list(20).unwrap();
        assert_eq!(
            listed.iter().map(|record| record.id).collect::<Vec<_>>(),
            vec![second.id, first.id]
        );
        let mut tampered = second;
        tampered.outcome = "tampered".into();
        store.append(&tampered).unwrap();
        assert_eq!(store.list(20).unwrap().len(), 2);
        let _ = fs::remove_dir_all(root);
    }
}
