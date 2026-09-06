//! Durable repositories (SurrealDB, event logs and settings) are composed here.
pub trait DurableStore: Send + Sync { fn put(&self, key: &str, value: &[u8]) -> Result<(), String>; fn get(&self, key: &str) -> Result<Option<Vec<u8>>, String>; }

use std::{fs, path::{Path, PathBuf}};
pub struct JsonFileStore { root: PathBuf }
impl JsonFileStore {
    pub fn at(root: impl AsRef<Path>) -> Self { Self { root: root.as_ref().to_path_buf() } }
    fn path(&self, key: &str) -> Result<PathBuf, String> {
        if key.is_empty() || key.contains('/') || key.contains('\\') || key.contains("..") { return Err("invalid persistence key".into()); }
        Ok(self.root.join(format!("{key}.json")))
    }
}
impl DurableStore for JsonFileStore {
    fn put(&self, key: &str, value: &[u8]) -> Result<(), String> {
        fs::create_dir_all(&self.root).map_err(|e| e.to_string())?;
        let path = self.path(key)?; let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, value).map_err(|e| e.to_string())?;
        fs::rename(&tmp, &path).map_err(|e| e.to_string())
    }
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, String> {
        match fs::read(self.path(key)?) { Ok(v) => Ok(Some(v)), Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None), Err(e) => Err(e.to_string()) }
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
