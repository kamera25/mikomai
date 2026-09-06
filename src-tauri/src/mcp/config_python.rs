use std::io::Write;
use std::process::Command;

/// Execute the isolated Python adapter used for configuration validation/conversion.
pub fn run(payload: serde_json::Value) -> Result<String, String> {
    let mut current_dir =
        std::env::current_dir().map_err(|e| format!("Failed to get current directory: {e}"))?;
    if current_dir.ends_with("src-tauri") {
        current_dir.pop();
    }

    let python_path = current_dir.join("venv").join("bin").join("python");
    let wrapper_path = current_dir.join("src-tauri").join("python").join("config_helper.py");
    if !python_path.exists() {
        return Err(format!("Python virtual environment binary not found at {python_path:?}"));
    }
    if !wrapper_path.exists() {
        return Err(format!("config_helper script not found at {wrapper_path:?}"));
    }

    let payload_str = serde_json::to_string(&payload)
        .map_err(|e| format!("Failed to serialize payload: {e}"))?;
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

