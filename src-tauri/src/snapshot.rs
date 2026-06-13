use std::fs;
use std::path::{Path, PathBuf};
use chrono::{FixedOffset, Utc};

/// `SnapshotManager` manages snapshots and state outputs for network devices.
pub struct SnapshotManager {
    base_dir: PathBuf,
    current_snapshot_dir: Option<PathBuf>,
}

impl SnapshotManager {
    /// Creates a new `SnapshotManager` targeting the standard AppData location.
    ///
    /// The default base directory is:
    /// - **macOS**: `~/Library/Application Support/com.mikomai.agent/storage/`
    /// - **Windows**: `%APPDATA%\mikomai\storage\`
    /// - **Linux**: `~/.local/share/mikomai/storage/`
    pub fn new() -> anyhow::Result<Self> {
        let mut base_dir = dirs::data_dir()
            .ok_or_else(|| anyhow::anyhow!("Failed to retrieve local data directory"))?;
        
        #[cfg(target_os = "macos")]
        base_dir.push("com.mikomai.agent");
        #[cfg(not(target_os = "macos"))]
        base_dir.push("mikomai");

        base_dir.push("storage");

        // Ensure base directory exists
        if !base_dir.exists() {
            fs::create_dir_all(&base_dir)?;
        }

        Ok(Self {
            base_dir,
            current_snapshot_dir: None,
        })
    }

    /// Creates a new `SnapshotManager` targeting a custom base directory (useful for testing/demo).
    pub fn with_base_dir(base_dir: PathBuf) -> Self {
        if !base_dir.exists() {
            let _ = fs::create_dir_all(&base_dir);
        }
        Self {
            base_dir,
            current_snapshot_dir: None,
        }
    }

    /// Creates a new snapshot directory under `storage/` using the JST timestamp: `YYYY-MM-DD_HH-MM-SS_snapshot`.
    /// Resolves collisions by appending a counter (e.g., `_1`, `_2`) if the directory already exists.
    pub fn create_snapshot_dir(&mut self) -> anyhow::Result<PathBuf> {
        // Ensure the base directory exists
        if !self.base_dir.exists() {
            fs::create_dir_all(&self.base_dir)?;
        }

        // Generate JST (UTC+9) timestamp
        let offset = FixedOffset::east_opt(9 * 3600)
            .ok_or_else(|| anyhow::anyhow!("Failed to create JST timezone offset"))?;
        let now_jst = Utc::now().with_timezone(&offset);
        let timestamp = now_jst.format("%Y-%m-%d_%H-%M-%S").to_string();

        let mut dir_name = format!("{}_snapshot", timestamp);
        let mut snapshot_dir = self.base_dir.join(&dir_name);

        // Resolve directory collisions
        let mut counter = 1;
        while snapshot_dir.exists() {
            dir_name = format!("{}_{}_snapshot", timestamp, counter);
            snapshot_dir = self.base_dir.join(&dir_name);
            counter += 1;
        }

        fs::create_dir_all(&snapshot_dir)?;
        self.current_snapshot_dir = Some(snapshot_dir.clone());

        Ok(snapshot_dir)
    }

    /// Saves the given content into the current snapshot directory.
    ///
    /// File naming rule: `{device_name}_{data_type}`.
    /// If `data_type` contains a dot (e.g. `arp.json`), the extension will be kept.
    /// If `data_type` has no dot, `.txt` will be appended.
    pub fn save_artifact(&mut self, device_name: &str, data_type: &str, content: &str) -> anyhow::Result<PathBuf> {
        let snapshot_dir = match &self.current_snapshot_dir {
            Some(dir) => dir.clone(),
            None => self.create_snapshot_dir()?,
        };

        let file_name = if data_type.contains('.') {
            format!("{}_{}", device_name, data_type)
        } else {
            format!("{}_{}.txt", device_name, data_type)
        };

        let file_path = snapshot_dir.join(&file_name);
        fs::write(&file_path, content)?;

        Ok(file_path)
    }

    /// Merges the contents of `snapshot_dir` into the `storage/current/` directory.
    ///
    /// Existing files in `storage/current/` that do not conflict with the new snapshot remain intact.
    pub fn update_current_link(&self, snapshot_dir: &Path) -> anyhow::Result<()> {
        if !snapshot_dir.exists() {
            return Err(anyhow::anyhow!("Snapshot directory does not exist: {:?}", snapshot_dir));
        }

        let current_dir = self.base_dir.join("current");
        if !current_dir.exists() {
            fs::create_dir_all(&current_dir)?;
        }

        if snapshot_dir.is_dir() {
            for entry in fs::read_dir(snapshot_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() {
                    let file_name = entry.file_name();
                    let dest_path = current_dir.join(file_name);
                    fs::copy(&path, &dest_path)?;
                }
            }
        }

        Ok(())
    }

    /// Returns the current active snapshot directory, if any.
    pub fn current_snapshot_dir(&self) -> Option<&PathBuf> {
        self.current_snapshot_dir.as_ref()
    }

    /// Returns the base directory.
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snapshot_creation_and_merge() {
        let temp_dir = std::env::temp_dir().join(format!("snapshot_test_{}", uuid::Uuid::new_v4()));
        let mut manager = SnapshotManager::with_base_dir(temp_dir.clone());

        // 1. Create first snapshot and save configs for Router-A & Router-B
        let snap_dir1 = manager.create_snapshot_dir().expect("Failed to create snap dir 1");
        assert!(snap_dir1.exists());

        let file_a_1 = manager.save_artifact("Router-A", "config", "Router-A config version 1")
            .expect("Failed to save Router-A config");
        let file_b_1 = manager.save_artifact("Router-B", "config", "Router-B config version 1")
            .expect("Failed to save Router-B config");

        assert!(file_a_1.exists());
        assert!(file_b_1.exists());

        manager.update_current_link(&snap_dir1).expect("Failed to update current for snap 1");

        let current_dir = temp_dir.join("current");
        assert!(current_dir.join("Router-A_config.txt").exists());
        assert!(current_dir.join("Router-B_config.txt").exists());

        assert_eq!(
            fs::read_to_string(current_dir.join("Router-A_config.txt")).unwrap(),
            "Router-A config version 1"
        );

        // 2. Create second snapshot and save config for Router-A only
        let snap_dir2 = manager.create_snapshot_dir().expect("Failed to create snap dir 2");
        // Ensure they are unique directories
        assert_ne!(snap_dir1, snap_dir2);

        let file_a_2 = manager.save_artifact("Router-A", "config", "Router-A config version 2")
            .expect("Failed to save Router-A config v2");
        assert!(file_a_2.exists());

        // File Router-B is NOT in snap_dir2
        assert!(!snap_dir2.join("Router-B_config.txt").exists());

        manager.update_current_link(&snap_dir2).expect("Failed to update current for snap 2");

        // Verify that current/ contains the updated Router-A config AND the old Router-B config
        assert_eq!(
            fs::read_to_string(current_dir.join("Router-A_config.txt")).unwrap(),
            "Router-A config version 2"
        );
        assert_eq!(
            fs::read_to_string(current_dir.join("Router-B_config.txt")).unwrap(),
            "Router-B config version 1"
        );

        // Clean up test directories
        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_collision_resolution() {
        let temp_dir = std::env::temp_dir().join(format!("snapshot_test_{}", uuid::Uuid::new_v4()));
        let mut manager = SnapshotManager::with_base_dir(temp_dir.clone());

        // Mock timestamp-like collisions manually by creating a directory
        // JST is UTC+9, get current JST date format
        let offset = FixedOffset::east_opt(9 * 3600).unwrap();
        let now_jst = Utc::now().with_timezone(&offset);
        let timestamp = now_jst.format("%Y-%m-%d_%H-%M-%S").to_string();

        let initial_dir_name = format!("{}_snapshot", timestamp);
        let initial_dir = temp_dir.join(&initial_dir_name);
        fs::create_dir_all(&initial_dir).unwrap();

        // Calling create_snapshot_dir should resolve collision and create a directory ending with _1_snapshot
        let snap_dir = manager.create_snapshot_dir().unwrap();
        assert!(snap_dir.to_string_lossy().contains("_1_snapshot"));
        assert!(snap_dir.exists());

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_data_type_extension() {
        let temp_dir = std::env::temp_dir().join(format!("snapshot_test_{}", uuid::Uuid::new_v4()));
        let mut manager = SnapshotManager::with_base_dir(temp_dir.clone());

        let _snap_dir = manager.create_snapshot_dir().unwrap();

        // Test normal extension appending
        let file_txt = manager.save_artifact("device1", "config", "content").unwrap();
        assert_eq!(file_txt.file_name().unwrap(), "device1_config.txt");

        // Test keeping custom extension
        let file_json = manager.save_artifact("device1", "arp.json", "content").unwrap();
        assert_eq!(file_json.file_name().unwrap(), "device1_arp.json");

        let _ = fs::remove_dir_all(temp_dir);
    }
}
