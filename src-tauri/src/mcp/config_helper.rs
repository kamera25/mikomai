use std::sync::Mutex;
use std::process::Command;
use std::io::Write;
use serde::{Serialize, Deserialize};
use crate::network::CommandResult;

pub struct ChoiceManager {
    pub tx: Mutex<Option<tokio::sync::oneshot::Sender<String>>>,
}

impl ChoiceManager {
    pub fn new() -> Self {
        Self {
            tx: Mutex::new(None),
        }
    }
}

#[tauri::command]
pub async fn submit_user_choice(
    choice: String,
    state: tauri::State<'_, ChoiceManager>
) -> Result<(), String> {
    let mut lock = state.tx.lock().map_err(|_| "Mutex lock poisoned".to_string())?;
    if let Some(tx) = lock.take() {
        let _ = tx.send(choice);
    }
    Ok(())
}

#[derive(Serialize)]
struct ValidatePayload {
    action: &'static str,
    config: String,
}

#[derive(Deserialize)]
struct ValidateResponse {
    success: bool,
    errors: Vec<String>,
    warnings: Vec<String>,
}

#[derive(Serialize)]
struct ConvertPayload {
    action: &'static str,
    config: String,
    target_vendor: String,
}

#[derive(Deserialize)]
struct ConvertResponse {
    success: bool,
    converted_config: String,
    error: Option<String>,
}

fn run_config_helper(payload: serde_json::Value) -> Result<String, String> {
    let mut current_dir = std::env::current_dir()
        .map_err(|e| format!("Failed to get current directory: {}", e))?;
    if current_dir.ends_with("src-tauri") {
        current_dir.pop();
    }
    
    let python_path = current_dir.join("venv").join("bin").join("python");
    let wrapper_path = current_dir.join("src-tauri").join("python").join("config_helper.py");

    if !python_path.exists() {
        return Err(format!("Python virtual environment binary not found at {:?}", python_path));
    }
    if !wrapper_path.exists() {
        return Err(format!("config_helper script not found at {:?}", wrapper_path));
    }

    let payload_str = serde_json::to_string(&payload)
        .map_err(|e| format!("Failed to serialize payload: {}", e))?;

    let mut child = Command::new(&python_path)
        .arg(&wrapper_path)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to run config helper process: {}", e))?;

    {
        let stdin = child.stdin.as_mut().ok_or("Failed to open stdin")?;
        stdin.write_all(payload_str.as_bytes())
            .map_err(|e| format!("Failed to write to stdin: {}", e))?;
    }

    let output = child.wait_with_output()
        .map_err(|e| format!("Failed to wait on helper process: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(format!("config_helper failed with stderr: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    Ok(stdout)
}

#[tauri::command]
pub async fn validate_cisco_config(config: String) -> Result<CommandResult, String> {
    if config.trim().is_empty() {
        return Err("Configuration text cannot be empty".to_string());
    }

    let payload = serde_json::json!(ValidatePayload {
        action: "validate",
        config: config.clone(),
    });

    let output_json = run_config_helper(payload)?;
    let res: ValidateResponse = serde_json::from_str(&output_json)
        .map_err(|e| format!("Failed to parse validator output: {}", e))?;

    let mut md = String::new();
    md.push_str("### Cisco Config Validation Results\n");
    if res.success {
        md.push_str("- **Status**: ✅ Validation Passed\n");
    } else {
        md.push_str("- **Status**: ❌ Validation Failed\n");
    }

    if !res.errors.is_empty() {
        md.push_str("\n#### Errors:\n");
        for err in &res.errors {
            md.push_str(&format!("- ❌ {}\n", err));
        }
    }

    if !res.warnings.is_empty() {
        md.push_str("\n#### Warnings / Security Advice:\n");
        for warn in &res.warnings {
            md.push_str(&format!("- ⚠️ {}\n", warn));
        }
    }

    Ok(CommandResult {
        success: res.success,
        output: md,
        saved_path: None,
        is_cached: None,
        cache_time: None,
    })
}

#[tauri::command]
pub async fn convert_cisco_config(config: String, target_vendor: String) -> Result<CommandResult, String> {
    if config.trim().is_empty() {
        return Err("Configuration text cannot be empty".to_string());
    }
    let vendor = target_vendor.trim().to_lowercase();
    if vendor != "juniper" && vendor != "arista" {
        return Err(format!("Unsupported target vendor: '{}'. Supported: 'juniper', 'arista'", target_vendor));
    }

    let payload = serde_json::json!(ConvertPayload {
        action: "convert",
        config: config.clone(),
        target_vendor: vendor.clone(),
    });

    let output_json = run_config_helper(payload)?;
    let res: ConvertResponse = serde_json::from_str(&output_json)
        .map_err(|e| format!("Failed to parse converter output: {}", e))?;

    if !res.success {
        let err_msg = res.error.unwrap_or_else(|| "Unknown conversion error".to_string());
        return Ok(CommandResult {
            success: false,
            output: format!("### Conversion Failed\nError: {}", err_msg),
            saved_path: None,
            is_cached: None,
            cache_time: None,
        });
    }

    let md = format!(
        "### Converted Configuration ({})\n\n```{}\n{}\n```",
        vendor,
        vendor,
        res.converted_config
    );

    Ok(CommandResult {
        success: true,
        output: md,
        saved_path: None,
        is_cached: None,
        cache_time: None,
    })
}

#[tauri::command]
pub async fn ask_user_choice(
    app: tauri::AppHandle,
    title: String,
    message: String,
    options: Vec<String>,
) -> Result<String, String> {
    use tauri::Emitter;
    use tauri::Manager;
    
    let choice_manager = app.state::<ChoiceManager>();
    
    let (tx, rx) = tokio::sync::oneshot::channel();
    {
        let mut lock = choice_manager.tx.lock().map_err(|_| "Mutex lock poisoned".to_string())?;
        *lock = Some(tx);
    }

    // Emit event to request user choice
    let payload = serde_json::json!({
        "title": title,
        "message": message,
        "options": options
    });
    
    let _ = app.emit("request-user-choice", payload);

    // Wait for frontend response
    match rx.await {
        Ok(c) => Ok(c),
        Err(_) => Ok("cancelled".to_string()),
    }
}

pub struct InterfaceChoiceManager {
    pub tx: Mutex<Option<tokio::sync::oneshot::Sender<String>>>,
}

impl InterfaceChoiceManager {
    pub fn new() -> Self {
        Self {
            tx: Mutex::new(None),
        }
    }
}

#[tauri::command]
pub async fn submit_interface_choice(
    choice: String,
    state: tauri::State<'_, InterfaceChoiceManager>
) -> Result<(), String> {
    let mut lock = state.tx.lock().map_err(|_| "Mutex lock poisoned".to_string())?;
    if let Some(tx) = lock.take() {
        let _ = tx.send(choice);
    }
    Ok(())
}

#[tauri::command]
pub async fn ask_interface_choice(
    app: tauri::AppHandle,
    vendor: String,
) -> Result<String, String> {
    use tauri::Emitter;
    use tauri::Manager;
    
    let choice_manager = app.state::<InterfaceChoiceManager>();
    
    let (tx, rx) = tokio::sync::oneshot::channel();
    {
        let mut lock = choice_manager.tx.lock().map_err(|_| "Mutex lock poisoned".to_string())?;
        *lock = Some(tx);
    }

    // Emit event to request interface choice
    let payload = serde_json::json!({
        "vendor": vendor,
    });
    
    let _ = app.emit("request-interface-choice", payload);

    // Wait for frontend response
    match rx.await {
        Ok(c) => Ok(c),
        Err(_) => Ok("cancelled".to_string()),
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_validate_cisco_config() {
        let config = "hostname RouterA\ninterface GigabitEthernet0/1\n ip address 192.168.1.1 255.255.255.0\n".to_string();
        let result = validate_cisco_config(config).await;
        assert!(result.is_ok(), "Expected success, got: {:?}", result);
        let res = result.unwrap();
        assert!(res.success);
        assert!(res.output.contains("Validation Passed"));
    }

    #[tokio::test]
    async fn test_convert_cisco_config() {
        let config = "hostname RouterA\ninterface GigabitEthernet0/1\n ip address 192.168.1.1 255.255.255.0\n".to_string();
        let result = convert_cisco_config(config, "juniper".to_string()).await;
        assert!(result.is_ok(), "Expected success, got: {:?}", result);
        let res = result.unwrap();
        assert!(res.success);
        assert!(res.output.contains("set system host-name RouterA"));
    }
}
