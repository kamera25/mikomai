//! State owner for the application. The legacy bridge shares one instance until
//! Command/Query/Event APIs and explicit service handles replace the C ABI.
use crate::FfiWatchRuntime;
use mikomai_core::{OperationPlan, TaskSnapshot};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{atomic::AtomicBool, Mutex, OnceLock};

#[derive(Default)]
pub struct MikomaiService {
    pub(crate) operation_plans: OnceLock<Result<Mutex<HashMap<String, OperationPlan>>, String>>,
    pub(crate) generic_execution_claims: OnceLock<Result<Mutex<HashSet<String>>, String>>,
    pub(crate) pending_agent_tasks: OnceLock<Mutex<HashMap<uuid::Uuid, TaskSnapshot>>>,
    pub(crate) rag_ingested_paths: OnceLock<Mutex<HashSet<PathBuf>>>,
    pub(crate) portable_graph:
        OnceLock<Mutex<Option<mikomai_adapters::portable_graph::PortableGraph>>>,
    pub(crate) device_workers: OnceLock<Result<mikomai_adapters::device_worker::WorkerPool,String>>,
    pub(crate) device_locks: OnceLock<crate::scheduling::DeviceLockManager>,
    pub(crate) watch_listener: OnceLock<std::sync::Arc<dyn crate::owned_bridge::LegacyListener>>,
    pub(crate) watch_runtime: OnceLock<Mutex<Option<FfiWatchRuntime>>>,
    pub(crate) apple_selected: AtomicBool,
    runtime: OnceLock<Result<tokio::runtime::Runtime, String>>,
    database_path: Option<PathBuf>,
    pub(crate) document_lock: Mutex<()>,
}

impl MikomaiService {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self {
            database_path: Some(path.into()),
            ..Self::default()
        }
    }

    pub(crate) fn graph(&self) -> Result<mikomai_adapters::portable_graph::PortableGraph, String> {
        let mut graph = self
            .portable_graph
            .get_or_init(|| Mutex::new(None))
            .lock()
            .map_err(|_| "portable graph lock is poisoned".to_string())?;
        if graph.is_none() {
            let path = self
                .database_path
                .clone()
                .map(Ok)
                .unwrap_or_else(crate::resolve_portable_graph_path)?;
            *graph =
                Some(self.run(
                    mikomai_adapters::portable_graph::PortableGraph::initialize_at(&path),
                )??);
        }
        graph
            .as_ref()
            .cloned()
            .ok_or_else(|| "portable graph initialization failed".into())
    }

    pub fn load_document(&self, collection: &str) -> Result<Option<serde_json::Value>, String> {
        // The legacy facade shares one service until explicit handles replace it.
        let graph = self.graph()?;
        self.run(graph.load_app_document(collection))?
    }

    pub fn save_document(&self, collection: &str, value: &serde_json::Value) -> Result<(), String> {
        validate_document(collection, value)?;
        let graph = self.graph()?;
        self.run(graph.save_app_document(collection, value))?
    }

    pub(crate) fn run<F: std::future::Future + Send>(&self, future: F) -> Result<F::Output, String>
    where F::Output: Send {
        let runtime = self.runtime()?;
        if let Ok(handle)=tokio::runtime::Handle::try_current() {
            let run=||std::thread::scope(|scope|scope.spawn(||runtime.block_on(future)).join()).map_err(|_|"application job panicked".to_string());
            // Return this runtime worker to Tokio while synchronous callers wait
            // for database work. Otherwise a one-worker runtime deadlocks itself.
            if handle.runtime_flavor()==tokio::runtime::RuntimeFlavor::MultiThread {tokio::task::block_in_place(run)} else {run()}
        } else { Ok(runtime.block_on(future)) }
    }

    pub(crate) fn save_internal(&self, collection: &str, value: &serde_json::Value) -> Result<(), String> {
        let graph = self.graph()?;
        self.run(graph.save_app_document(collection, value))?
    }

    pub(crate) fn update_internal<T>(&self, collection: &str, update: impl FnOnce(&mut serde_json::Value) -> Result<T, String>) -> Result<T, String> {
        let _guard = self.document_lock.lock().map_err(|_| "store lock poisoned")?;
        let mut value = self.load_document(collection)?.unwrap_or(serde_json::json!({}));
        let result = update(&mut value)?;
        self.save_internal(collection, &value)?;
        Ok(result)
    }

    /// All application jobs, including Watch and approved operations, use this
    /// runtime. Construction is lazy so the synchronous bridge stays lightweight.
    pub(crate) fn runtime(&self) -> Result<&tokio::runtime::Runtime, String> {
        self.runtime
            .get_or_init(|| {
                tokio::runtime::Builder::new_multi_thread()
                    .thread_name("mikomai-app-worker")
                    .enable_all()
                    .build()
                    .map_err(|error| format!("could not initialize application runtime: {error}"))
            })
            .as_ref()
            .map_err(Clone::clone)
    }
}

pub(crate) fn shared_service() -> &'static MikomaiService {
    static SERVICE: OnceLock<MikomaiService> = OnceLock::new();
    SERVICE.get_or_init(MikomaiService::new)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    #[test]
    fn stores_are_isolated_and_credentials_cannot_replace_metadata() {
        let root =
            std::env::temp_dir().join(format!("mikomai-service-store-{}", uuid::Uuid::new_v4()));
        let first = MikomaiService::at(root.join("first"));
        let second = MikomaiService::at(root.join("second"));
        let original = serde_json::json!({"modelPath":"model.gguf", "temperature":0.2});
        first.save_document("settings", &original).unwrap();
        assert_eq!(
            first.load_document("settings").unwrap(),
            Some(original.clone())
        );
        assert_eq!(second.load_document("settings").unwrap(), None);
        assert!(first
            .save_document("settings", &serde_json::json!({"password":"do-not-store"}))
            .is_err());
        assert_eq!(first.load_document("settings").unwrap(), Some(original));
        assert!(first
            .save_document(
                "connections",
                &serde_json::json!([{"id":"bad","host":"host;touch"}])
            )
            .is_err());
    }

    #[test]
    fn storage_initialization_failure_is_returned_to_the_caller() {
        let path =
            std::env::temp_dir().join(format!("mikomai-store-file-{}", uuid::Uuid::new_v4()));
        std::fs::write(&path, "not a directory").unwrap();
        let service = MikomaiService::at(&path);
        assert!(service.load_document("settings").is_err());
        assert!(service
            .save_document("settings", &serde_json::json!({}))
            .is_err());
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn instances_do_not_share_application_state() {
        let first = MikomaiService::new();
        let second = MikomaiService::new();
        first.apple_selected.store(true, Ordering::Relaxed);
        first
            .pending_agent_tasks
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .unwrap()
            .insert(uuid::Uuid::new_v4(), TaskSnapshot::new("first"));
        assert!(!second.apple_selected.load(Ordering::Relaxed));
        assert!(second
            .pending_agent_tasks
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .unwrap()
            .is_empty());
    }

    #[test]
    fn legacy_callers_share_the_same_service_and_runtime() {
        assert!(std::ptr::eq(shared_service(), shared_service()));
        assert!(std::ptr::eq(
            shared_service().runtime().unwrap(),
            crate::portable_runtime().unwrap()
        ));
    }
}

fn validate_document(collection: &str, value: &serde_json::Value) -> Result<(), String> {
    fn contains_secret(value: &serde_json::Value) -> bool {
        match value {
            serde_json::Value::Object(object) => object.iter().any(|(key, value)| {
                let normalized = key.to_ascii_lowercase().replace(['_', '-'], "");
                [
                    "password",
                    "enablepassword",
                    "secret",
                    "passphrase",
                    "privatekey",
                    "token",
                ]
                .contains(&normalized.as_str())
                    || contains_secret(value)
            }),
            serde_json::Value::Array(array) => array.iter().any(contains_secret),
            _ => false,
        }
    }
    if contains_secret(value) {
        return Err("credentials must be stored in the OS credential store".into());
    }
    match collection {
        "connections" => {
            for connection in value.as_array().ok_or("connections must be an array")? {
                if let Some(error) = crate::native_features::validate_connection(connection) {
                    return Err(error);
                }
                connection["id"]
                    .as_str()
                    .and_then(|id| uuid::Uuid::parse_str(id).ok())
                    .ok_or("connection ID must be a UUID")?;
            }
        }
        "sessions" => {
            if !value["sessions"].is_array() {
                return Err("invalid session snapshot".into());
            }
        }
        "settings" => {
            if !value.is_object() {
                return Err("settings must be an object".into());
            }
        }
        _ => return Err("unsupported application collection".into()),
    }
    Ok(())
}
