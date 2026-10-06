//! State owner for the application. The legacy bridge shares one instance until
//! Command/Query/Event APIs and explicit service handles replace the C ABI.
use crate::FfiWatchRuntime;
use mikomai_core::{OperationPlan, TaskSnapshot};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{atomic::AtomicBool, Mutex, OnceLock};

#[derive(Default)]
pub struct MikomaiService {
    pub(crate) operation_plans: OnceLock<Mutex<HashMap<String, OperationPlan>>>,
    pub(crate) generic_execution_claims: OnceLock<Mutex<HashSet<String>>>,
    pub(crate) pending_agent_tasks: OnceLock<Mutex<HashMap<uuid::Uuid, TaskSnapshot>>>,
    pub(crate) rag_ingested_paths: OnceLock<Mutex<HashSet<PathBuf>>>,
    pub(crate) portable_graph:
        OnceLock<Mutex<Option<mikomai_adapters::portable_graph::PortableGraph>>>,
    pub(crate) operation_audit: OnceLock<mikomai_adapters::audit::FileAuditLog>,
    pub(crate) watch_runtime: OnceLock<Mutex<Option<FfiWatchRuntime>>>,
    pub(crate) apple_selected: AtomicBool,
    runtime: OnceLock<Result<tokio::runtime::Runtime, String>>,
}

impl MikomaiService {
    pub fn new() -> Self {
        Self::default()
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
