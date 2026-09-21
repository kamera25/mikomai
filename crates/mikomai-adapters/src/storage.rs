//! Storage ports and file-backed implementation. SurrealDB-specific code can
//! implement the same trait without leaking a database type into the core.
use crate::persistence::{DurableStore, JsonFileStore};
use std::path::Path;

pub trait GraphStorePort: Send + Sync {
    fn initialize(&self) -> Result<(), String>;
    fn put_document(&self, key: &str, document: &[u8]) -> Result<(), String>;
    fn get_document(&self, key: &str) -> Result<Option<Vec<u8>>, String>;
}

pub struct FileGraphStore {
    store: JsonFileStore,
}
pub struct SurrealAdapter<S> {
    pub store: S,
}
impl<S: DurableStore> GraphStorePort for SurrealAdapter<S> {
    fn initialize(&self) -> Result<(), String> {
        Ok(())
    }
    fn put_document(&self, key: &str, document: &[u8]) -> Result<(), String> {
        self.store.put(key, document)
    }
    fn get_document(&self, key: &str) -> Result<Option<Vec<u8>>, String> {
        self.store.get(key)
    }
}
impl FileGraphStore {
    pub fn at(root: impl AsRef<Path>) -> Self {
        Self {
            store: JsonFileStore::at(root),
        }
    }
}
impl GraphStorePort for FileGraphStore {
    fn initialize(&self) -> Result<(), String> {
        std::fs::create_dir_all(self.root()).map_err(|error| error.to_string())
    }
    fn put_document(&self, key: &str, document: &[u8]) -> Result<(), String> {
        self.store.put(key, document)
    }
    fn get_document(&self, key: &str) -> Result<Option<Vec<u8>>, String> {
        self.store.get(key)
    }
}
impl FileGraphStore {
    fn root(&self) -> &Path {
        self.store.root_path()
    }
}
