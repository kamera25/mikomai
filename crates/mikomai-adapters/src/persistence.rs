//! Durable repositories (SurrealDB, event logs and settings) are composed here.
pub trait DurableStore: Send + Sync {
    fn put(&self, key: &str, value: &[u8]) -> Result<(), String>;
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, String>;
}

use mikomai_core::{
    ApplicationError, Evidence, HistoryRepository, OperationPlan, OperationRepository,
    TaskRepository, TaskSnapshot,
};
use std::{
    fs,
    path::{Path, PathBuf},
};
use uuid::Uuid;
pub struct JsonFileStore {
    root: PathBuf,
}
impl JsonFileStore {
    pub fn at(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }
    pub(crate) fn root_path(&self) -> &Path {
        &self.root
    }
    fn path(&self, key: &str) -> Result<PathBuf, String> {
        if key.is_empty() || key.contains('/') || key.contains('\\') || key.contains("..") {
            return Err("invalid persistence key".into());
        }
        Ok(self.root.join(format!("{key}.json")))
    }
}
impl DurableStore for JsonFileStore {
    fn put(&self, key: &str, value: &[u8]) -> Result<(), String> {
        fs::create_dir_all(&self.root).map_err(|e| e.to_string())?;
        let path = self.path(key)?;
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, value).map_err(|e| e.to_string())?;
        fs::rename(&tmp, &path).map_err(|e| e.to_string())
    }
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, String> {
        match fs::read(self.path(key)?) {
            Ok(v) => Ok(Some(v)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }
}

/// File-backed task repository used when no database adapter is configured.
/// Writes are atomic and serialization failures are returned to the caller.
pub struct FileTaskRepository {
    store: JsonFileStore,
}
impl FileTaskRepository {
    pub fn at(root: impl AsRef<Path>) -> Self {
        Self {
            store: JsonFileStore::at(root),
        }
    }
}
impl TaskRepository for FileTaskRepository {
    fn save(&self, snapshot: &TaskSnapshot) -> Result<(), ApplicationError> {
        let data =
            serde_json::to_vec(snapshot).map_err(|e| ApplicationError::storage(e.to_string()))?;
        self.store
            .put(&snapshot.task.id.to_string(), &data)
            .map_err(ApplicationError::storage)
    }
    fn load(&self, id: Uuid) -> Result<Option<TaskSnapshot>, ApplicationError> {
        self.store
            .get(&id.to_string())
            .map_err(ApplicationError::storage)?
            .map(|data| {
                serde_json::from_slice(&data).map_err(|e| ApplicationError::storage(e.to_string()))
            })
            .transpose()
    }
}

/// File-backed operation plans. The core remains responsible for approval and
/// hash validation; this adapter only persists the aggregate.
pub struct FileOperationRepository {
    store: JsonFileStore,
}
impl FileOperationRepository {
    pub fn at(root: impl AsRef<Path>) -> Self {
        Self {
            store: JsonFileStore::at(root),
        }
    }
}
impl OperationRepository for FileOperationRepository {
    fn prepare(&self) -> Result<(), ApplicationError> {
        fs::create_dir_all(self.store.root_path())
            .map_err(|error| ApplicationError::storage(error.to_string()))
    }

    fn save(&self, plan: &OperationPlan) -> Result<(), ApplicationError> {
        let data = serde_json::to_vec(plan)
            .map_err(|error| ApplicationError::storage(error.to_string()))?;
        self.store
            .put(&plan.id.to_string(), &data)
            .map_err(ApplicationError::storage)
    }
    fn load(&self, id: Uuid) -> Result<Option<OperationPlan>, ApplicationError> {
        self.store
            .get(&id.to_string())
            .map_err(ApplicationError::storage)?
            .map(|data| {
                serde_json::from_slice(&data)
                    .map_err(|error| ApplicationError::storage(error.to_string()))
            })
            .transpose()
    }
}

/// Append-only evidence log used by headless and desktop compositions that do
/// not yet have a database-backed history adapter.
pub struct FileHistoryRepository {
    root: PathBuf,
}
impl FileHistoryRepository {
    pub fn at(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }
}
impl HistoryRepository for FileHistoryRepository {
    fn prepare(&self, _task_id: Uuid) -> Result<(), ApplicationError> {
        fs::create_dir_all(&self.root)
            .map_err(|error| ApplicationError::storage(error.to_string()))?;
        fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.root.join("evidence.ndjson"))
            .map(|_| ())
            .map_err(|error| ApplicationError::storage(error.to_string()))
    }

    fn append(&self, task_id: Uuid, evidence: &Evidence) -> Result<(), ApplicationError> {
        use std::io::Write;
        fs::create_dir_all(&self.root)
            .map_err(|error| ApplicationError::storage(error.to_string()))?;
        let path = self.root.join("evidence.ndjson");
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|error| ApplicationError::storage(error.to_string()))?;
        let record = serde_json::json!({ "taskId": task_id, "evidence": evidence });
        serde_json::to_writer(&mut file, &record)
            .map_err(|error| ApplicationError::storage(error.to_string()))?;
        file.write_all(b"\n")
            .map_err(|error| ApplicationError::storage(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn file_store_round_trips_and_rejects_traversal() {
        let dir = std::env::temp_dir().join(format!("mikomai-store-{}", std::process::id()));
        let store = JsonFileStore::at(&dir);
        store.put("task", b"{}").unwrap();
        assert_eq!(store.get("task").unwrap(), Some(b"{}".to_vec()));
        assert!(store.put("../escape", b"x").is_err());
        let _ = fs::remove_dir_all(dir);
    }
}
