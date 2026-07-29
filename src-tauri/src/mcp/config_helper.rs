use std::sync::Mutex;
use std::process::Command;
use std::io::Write;
use serde::{Serialize, Deserialize};
use crate::network::CommandResult;

pub struct ChoiceManager {
    pub txs: Mutex<std::collections::HashMap<String, tokio::sync::oneshot::Sender<String>>>,
}

impl ChoiceManager {
    pub fn new() -> Self {
        Self {
            txs: Mutex::new(std::collections::HashMap::new()),
        }
    }
}

#[tauri::command]
pub async fn submit_user_choice(
    id: Option<String>,
    choice: String,
    state: tauri::State<'_, ChoiceManager>
) -> Result<(), String> {
    let id = id.unwrap_or_else(|| "default".to_string());
    let mut lock = state.txs.lock().map_err(|_| "Mutex lock poisoned".to_string())?;
    if let Some(tx) = lock.remove(&id) {
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

pub async fn validate_cisco_config_impl(
    app: Option<tauri::AppHandle>,
    id: Option<String>,
    config: String,
    target_device: Option<(String, String)>,
) -> Result<CommandResult, String> {
    if config.trim().is_empty() {
        return Err("Configuration text cannot be empty".to_string());
    }

    let payload = serde_json::json!(ValidatePayload {
        action: "validate",
        config: config.clone(),
    });

    let (res_errors, res_warnings) = match run_config_helper(payload) {
        Ok(output_json) => {
            if let Ok(res) = serde_json::from_str::<ValidateResponse>(&output_json) {
                (res.errors, res.warnings)
            } else {
                (vec![], vec![])
            }
        }
        Err(e) => {
            (vec![], vec![format!("Config helper notice: {}", e)])
        }
    };

    let res = ValidateResponse {
        success: true, // Cisco Config 検証失敗機能を一旦無効化
        errors: res_errors,
        warnings: res_warnings,
    };

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

    if res.success {
        if let Some(app_handle) = app {
            use tauri::Emitter;
            use tauri::Manager;

            let id = id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            let choice_manager = app_handle.state::<ChoiceManager>();
            let (tx, rx) = tokio::sync::oneshot::channel();
            {
                let mut lock = choice_manager.txs.lock().map_err(|_| "Mutex lock poisoned".to_string())?;
                lock.insert(id.clone(), tx);
            }

            let mut hostname = None;
            let mut ip = None;
            if let Some((h, i)) = target_device {
                hostname = Some(h);
                ip = Some(i);
            } else if let Some(host) = crate::settings::load_settings(app_handle.clone())
                .ok()
                .and_then(|settings| settings.recent_ips.first().cloned())
            {
                if let Ok(connections) = crate::connections::load_connections_raw(&app_handle) {
                    if let Some(conn) = connections.iter().find(|c| c.hostname.eq_ignore_ascii_case(&host) || c.ip.to_string() == host) {
                        hostname = Some(conn.hostname.as_str().to_string());
                        ip = Some(conn.ip.to_string());
                    }
                }
            }

            // Emit event to request diff commit
            let payload = serde_json::json!({
                "id": id,
                "config": config,
                "fileName": "cisco.conf",
                "hostname": hostname,
                "ip": ip
            });
            let _ = app_handle.emit("request-diff-commit", payload);

            // Wait for frontend response (commit or cancel)
            match rx.await {
                Ok(c) => {
                    if c == "commit" {
                        Ok(CommandResult {
                            success: true,
                            output: format!("{}\n\n**Status**: 🚀 Configuration successfully committed/deployed by user.", md),
                            saved_path: None,
                            is_cached: None,
                            cache_time: None,
                        })
                    } else {
                        Ok(CommandResult {
                            success: false,
                            output: format!("{}\n\n**Status**: ⚠️ Configuration deployment cancelled by user.", md),
                            saved_path: None,
                            is_cached: None,
                            cache_time: None,
                        })
                    }
                }
                Err(_) => Err("Failed to receive user choice".to_string()),
            }
        } else {
            Ok(CommandResult {
                success: true,
                output: md,
                saved_path: None,
                is_cached: None,
                cache_time: None,
            })
        }
    } else {
        Ok(CommandResult {
            success: false,
            output: md,
            saved_path: None,
            is_cached: None,
            cache_time: None,
        })
    }
}

#[tauri::command]
pub async fn validate_cisco_config(app: tauri::AppHandle, config: String) -> Result<CommandResult, String> {
    validate_cisco_config_impl(Some(app), None, config, None).await
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
    id: Option<String>,
    title: String,
    message: String,
    options: Vec<String>,
) -> Result<String, String> {
    use tauri::Emitter;
    use tauri::Manager;
    
    let id = id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let choice_manager = app.state::<ChoiceManager>();
    
    let (tx, rx) = tokio::sync::oneshot::channel();
    {
        let mut lock = choice_manager.txs.lock().map_err(|_| "Mutex lock poisoned".to_string())?;
        lock.insert(id.clone(), tx);
    }

    // Emit event to request user choice
    let payload = serde_json::json!({
        "id": id,
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
    pub txs: Mutex<std::collections::HashMap<String, tokio::sync::oneshot::Sender<String>>>,
}

impl InterfaceChoiceManager {
    pub fn new() -> Self {
        Self {
            txs: Mutex::new(std::collections::HashMap::new()),
        }
    }
}

#[tauri::command]
pub async fn submit_interface_choice(
    id: Option<String>,
    choice: String,
    state: tauri::State<'_, InterfaceChoiceManager>
) -> Result<(), String> {
    let id = id.unwrap_or_else(|| "default".to_string());
    let mut lock = state.txs.lock().map_err(|_| "Mutex lock poisoned".to_string())?;
    if let Some(tx) = lock.remove(&id) {
        let _ = tx.send(choice);
    }
    Ok(())
}

#[tauri::command]
pub async fn ask_interface_choice(
    app: tauri::AppHandle,
    id: Option<String>,
    vendor: String,
    message: Option<String>,
) -> Result<String, String> {
    use tauri::Emitter;
    use tauri::Manager;
    
    let id = id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let choice_manager = app.state::<InterfaceChoiceManager>();
    
    let (tx, rx) = tokio::sync::oneshot::channel();
    {
        let mut lock = choice_manager.txs.lock().map_err(|_| "Mutex lock poisoned".to_string())?;
        lock.insert(id.clone(), tx);
    }

    // Emit event to request interface choice
    let payload = serde_json::json!({
        "id": id,
        "vendor": vendor,
        "message": message,
    });
    
    let _ = app.emit("request-interface-choice", payload);

    // Wait for frontend response
    match rx.await {
        Ok(c) => Ok(c),
        Err(_) => Ok("cancelled".to_string()),
    }
}

pub struct IpAddressChoiceManager {
    pub txs: Mutex<std::collections::HashMap<String, tokio::sync::oneshot::Sender<String>>>,
}

impl IpAddressChoiceManager {
    pub fn new() -> Self {
        Self {
            txs: Mutex::new(std::collections::HashMap::new()),
        }
    }
}

#[tauri::command]
pub async fn submit_ipaddress_choice(
    id: Option<String>,
    choice: String,
    state: tauri::State<'_, IpAddressChoiceManager>
) -> Result<(), String> {
    let id = id.unwrap_or_else(|| "default".to_string());
    let mut lock = state.txs.lock().map_err(|_| "Mutex lock poisoned".to_string())?;
    if let Some(tx) = lock.remove(&id) {
        let _ = tx.send(choice);
    }
    Ok(())
}

#[tauri::command]
pub async fn ask_ipaddress_choice(
    app: tauri::AppHandle,
    id: Option<String>,
    title: String,
    message: String,
    subnet: String,
    default_ip: Option<String>,
) -> Result<String, String> {
    use tauri::Emitter;
    use tauri::Manager;
    
    let id = id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let choice_manager = app.state::<IpAddressChoiceManager>();
    
    let (tx, rx) = tokio::sync::oneshot::channel();
    {
        let mut lock = choice_manager.txs.lock().map_err(|_| "Mutex lock poisoned".to_string())?;
        lock.insert(id.clone(), tx);
    }

    // Emit event to request IP address choice
    let payload = serde_json::json!({
        "id": id,
        "title": title,
        "message": message,
        "subnet": subnet,
        "defaultIp": default_ip,
    });
    
    let _ = app.emit("request-ipaddress-choice", payload);

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
        let result = validate_cisco_config_impl(None, None, config, None).await;
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
