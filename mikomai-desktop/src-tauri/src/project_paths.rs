use std::path::{Path, PathBuf};

/// Locate the Tauri project directory from the workspace root, desktop root,
/// or Tauri project's own working directory.
pub(crate) fn tauri_dir_from(current_dir: &Path) -> PathBuf {
    if current_dir.file_name().is_some_and(|name| name == "src-tauri") {
        return current_dir.to_path_buf();
    }

    for ancestor in current_dir.ancestors() {
        let nested = ancestor.join("mikomai-desktop/src-tauri");
        if nested.is_dir() {
            return nested;
        }
        let adjacent = ancestor.join("src-tauri");
        if adjacent.is_dir() {
            return adjacent;
        }
    }

    current_dir.join("src-tauri")
}

/// Locate the Cargo workspace root for resources such as the shared Python venv.
pub(crate) fn workspace_root_from(current_dir: &Path) -> PathBuf {
    current_dir
        .ancestors()
        .find(|ancestor| ancestor.join("mikomai-core").is_dir())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| current_dir.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_tauri_and_workspace_paths_from_all_project_roots() {
        let tauri_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let desktop_dir = tauri_dir.parent().unwrap();
        let workspace_root = desktop_dir.parent().unwrap();

        assert_eq!(tauri_dir_from(workspace_root), tauri_dir);
        assert_eq!(tauri_dir_from(desktop_dir), tauri_dir);
        assert_eq!(tauri_dir_from(&tauri_dir), tauri_dir);

        assert_eq!(workspace_root_from(workspace_root), workspace_root);
        assert_eq!(workspace_root_from(desktop_dir), workspace_root);
        assert_eq!(workspace_root_from(&tauri_dir), workspace_root);
    }
}
