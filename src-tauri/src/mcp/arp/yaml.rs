use crate::snapshot::SnapshotManager;

pub fn save_validated_yaml(device_name: &str, yaml_content: &str) -> Result<String, String> {
    let mut manager = SnapshotManager::new().map_err(|e| format!("Failed to create SnapshotManager: {}", e))?;
    match manager.save_artifact(device_name, "arp.yaml", yaml_content) {
        Ok(path) => {
            let _ = manager.update_current_link(path.parent().unwrap());
            Ok(path.to_string_lossy().to_string())
        }
        Err(e) => Err(format!("Failed to save YAML artifact: {}", e)),
    }
}
