use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

fn find_python_binary(base: &Path) -> Option<PathBuf> {
    // Check unix venv/bin/python (do NOT canonicalize, as venv relies on symlink to locate its site-packages)
    let unix_venv = base.join("venv").join("bin").join("python");
    if unix_venv.is_file() {
        return Some(unix_venv);
    }
    // Check windows venv/Scripts/python.exe
    let win_venv = base.join("venv").join("Scripts").join("python.exe");
    if win_venv.is_file() {
        return Some(win_venv);
    }
    None
}

fn find_script(base: &Path) -> Option<PathBuf> {
    let script1 = base.join("src-tauri").join("python").join("config_helper.py");
    if script1.is_file() {
        return Some(script1);
    }
    let script2 = base.join("python").join("config_helper.py");
    if script2.is_file() {
        return Some(script2);
    }
    None
}

fn get_candidate_dirs() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(curr) = std::env::current_dir() {
        let mut c = curr.clone();
        if c.ends_with("src-tauri") {
            c.pop();
        }
        candidates.push(c.clone());
        candidates.push(curr);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            candidates.push(exe_dir.to_path_buf());
            if let Some(parent) = exe_dir.parent() {
                candidates.push(parent.to_path_buf());
                if let Some(grandparent) = parent.parent() {
                    candidates.push(grandparent.to_path_buf());
                }
            }
        }
    }
    candidates
}

pub fn resolve_paths() -> Result<(PathBuf, PathBuf), String> {
    // 1. Check environment variables first
    let python_path = if let Ok(custom_py) = std::env::var("MIKOMAI_PYTHON_PATH") {
        let p = PathBuf::from(custom_py);
        if !p.is_file() {
            return Err(format!("Specified MIKOMAI_PYTHON_PATH is not a valid file: {p:?}"));
        }
        p
    } else {
        let candidates = get_candidate_dirs();
        candidates
            .iter()
            .find_map(|dir| find_python_binary(dir))
            .ok_or_else(|| "Python virtual environment binary not found in candidate paths".to_string())?
    };

    let wrapper_path = if let Ok(custom_script) = std::env::var("MIKOMAI_CONFIG_HELPER_PATH") {
        let p = PathBuf::from(custom_script);
        if !p.is_file() {
            return Err(format!("Specified MIKOMAI_CONFIG_HELPER_PATH is not a valid file: {p:?}"));
        }
        p
    } else {
        let candidates = get_candidate_dirs();
        candidates
            .iter()
            .find_map(|dir| find_script(dir))
            .ok_or_else(|| "config_helper.py script not found in candidate paths".to_string())?
    };

    Ok((python_path, wrapper_path))
}

/// Execute the isolated Python adapter used for configuration validation/conversion.
pub fn run(payload: serde_json::Value) -> Result<String, String> {
    let (python_path, wrapper_path) = resolve_paths()?;

    let payload_str =
        serde_json::to_string(&payload).map_err(|e| format!("Failed to serialize payload: {e}"))?;
    let mut child = Command::new(&python_path)
        .arg(&wrapper_path)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to run config helper process: {e}"))?;
    child
        .stdin
        .as_mut()
        .ok_or("Failed to open stdin")?
        .write_all(payload_str.as_bytes())
        .map_err(|e| format!("Failed to write to stdin: {e}"))?;
    let output = child
        .wait_with_output()
        .map_err(|e| format!("Failed to wait on helper process: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "config_helper failed with stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_paths() {
        let res = resolve_paths();
        if let Ok((py, script)) = res {
            assert!(py.is_file());
            assert!(script.is_file());
        }
    }
}
