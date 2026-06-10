use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_shell::{process::CommandChild, ShellExt};
use crate::connections::get_device_config;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DeviceConfig {
    pub host: String,
    pub username: String,
    pub password: Option<String>,
    pub enable_password: Option<String>,
    pub device_type: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CommandResult {
    pub success: bool,
    pub output: String,
}

// Abstract trait for network operations
pub trait NetworkInterface {
    async fn execute_show(&self, device: &DeviceConfig, command: &str) -> Result<String, String>;
    async fn execute_config(&self, device: &DeviceConfig, commands: Vec<String>) -> Result<String, String>;
}

// Implementation using a Tauri Sidecar fallback
pub struct SidecarNetmikoWrapper {
    app: AppHandle,
}

impl SidecarNetmikoWrapper {
    pub fn new(app: &AppHandle) -> Self {
        Self { app: app.clone() }
    }

    async fn run_sidecar(&self, payload: serde_json::Value) -> Result<String, String> {
        let payload_str = serde_json::to_string(&payload)
            .map_err(|e| format!("Failed to serialize payload: {}", e))?;

        let (mut rx, mut child) = self.app.shell()
            .sidecar("netmiko_wrapper")
            .map_err(|e| format!("Failed to create sidecar command: {}", e))?
            .arg("--stdin")
            .spawn()
            .map_err(|e| format!("Failed to spawn sidecar: {}", e))?;

        child.write(format!("{}\n", payload_str).as_bytes())
            .map_err(|e| format!("Failed to write to sidecar stdin: {}", e))?;

        let mut stdout = String::new();
        let mut stderr = String::new();

        while let Some(event) = rx.recv().await {
            match event {
                tauri_plugin_shell::process::CommandEvent::Stdout(line) => {
                    stdout.push_str(&String::from_utf8_lossy(&line));
                    stdout.push('\n');
                }
                tauri_plugin_shell::process::CommandEvent::Stderr(line) => {
                    stderr.push_str(&String::from_utf8_lossy(&line));
                    stderr.push('\n');
                }
                tauri_plugin_shell::process::CommandEvent::Error(err) => {
                    return Err(format!("Sidecar error: {}", err));
                }
                tauri_plugin_shell::process::CommandEvent::Terminated(payload) => {
                    let code = payload.code.unwrap_or(-1);
                    if code == 0 {
                        return Ok(stdout);
                    } else {
                        return Err(stderr.trim().to_string());
                    }
                }
                _ => {}
            }
        }

        Err("Sidecar completed unexpectedly".to_string())
    }
}

impl NetworkInterface for SidecarNetmikoWrapper {
    async fn execute_show(&self, device: &DeviceConfig, command: &str) -> Result<String, String> {
        let payload = serde_json::json!({
            "action": "show",
            "host": device.host,
            "username": device.username,
            "password": device.password.clone().unwrap_or_default(),
            "secret": device.enable_password.clone().unwrap_or_default(),
            "device_type": device.device_type,
            "command": command
        });
        self.run_sidecar(payload).await
    }

    async fn execute_config(&self, device: &DeviceConfig, commands: Vec<String>) -> Result<String, String> {
        let payload = serde_json::json!({
            "action": "config",
            "host": device.host,
            "username": device.username,
            "password": device.password.clone().unwrap_or_default(),
            "secret": device.enable_password.clone().unwrap_or_default(),
            "device_type": device.device_type,
            "commands": commands
        });
        self.run_sidecar(payload).await
    }
}

#[tauri::command]
pub async fn network_show(
    app: AppHandle,
    device: DeviceConfig,
    command: String,
) -> Result<CommandResult, String> {
    let mut target_device = device.clone();
    
    // Try to resolve it from MCP/Connections, falling back to passed-in device if not found
    if let Some((ip, user, password, enable_password, dtype)) = get_device_config(&app, &device.host) {
        target_device.host = ip;
        target_device.username = user;
        if password.is_some() {
            target_device.password = password;
        }
        if enable_password.is_some() {
            target_device.enable_password = enable_password;
        }
        target_device.device_type = dtype;
    }

    println!("Executing read-only command on {}: {}", target_device.host, command);
    let wrapper = SidecarNetmikoWrapper::new(&app);
    match wrapper.execute_show(&target_device, &command).await {
        Ok(output) => Ok(CommandResult { success: true, output }),
        Err(err) => Ok(CommandResult { success: false, output: err }),
    }
}

#[tauri::command]
pub async fn network_config(
    app: AppHandle,
    device: DeviceConfig,
    commands: Vec<String>,
) -> Result<CommandResult, String> {
    let mut target_device = device.clone();
    
    // Try to resolve it from MCP/Connections, falling back to passed-in device if not found
    if let Some((ip, user, password, enable_password, dtype)) = get_device_config(&app, &device.host) {
        target_device.host = ip;
        target_device.username = user;
        if password.is_some() {
            target_device.password = password;
        }
        if enable_password.is_some() {
            target_device.enable_password = enable_password;
        }
        target_device.device_type = dtype;
    }

    println!("Executing WRITE command on {}: {:?}", target_device.host, commands);
    let wrapper = SidecarNetmikoWrapper::new(&app);
    match wrapper.execute_config(&target_device, commands).await {
        Ok(output) => Ok(CommandResult { success: true, output }),
        Err(err) => Ok(CommandResult { success: false, output: err }),
    }
}

pub struct McpState {
    pub process: Mutex<Option<CommandChild>>,
}

#[tauri::command]
pub async fn start_ns_mcp_server(app: AppHandle, state: State<'_, McpState>) -> Result<String, String> {
    println!("Starting Network Sketcher MCP Server...");

    let mut process_lock = state.process.lock().map_err(|_| "Mutex lock poisoned".to_string())?;
    if process_lock.is_some() {
        return Ok("MCP Server is already running".to_string());
    }

    let (mut rx, child) = app.shell()
        .sidecar("ns_mcp_server")
        .map_err(|e| format!("Failed to create sidecar command: {}", e))?
        .spawn()
        .map_err(|e| format!("Failed to spawn sidecar: {}", e))?;

    *process_lock = Some(child);

    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(event) = rx.recv().await {
            match event {
                tauri_plugin_shell::process::CommandEvent::Stdout(line) => {
                    let output = String::from_utf8_lossy(&line).to_string();
                    let _ = app_handle.emit("mcp-response", output);
                }
                tauri_plugin_shell::process::CommandEvent::Stderr(line) => {
                    let err = String::from_utf8_lossy(&line).to_string();
                    eprintln!("[MCP Server stderr] {}", err);
                    let _ = app_handle.emit("mcp-error", err);
                }
                tauri_plugin_shell::process::CommandEvent::Error(err) => {
                    eprintln!("[MCP Server error] {}", err);
                    let _ = app_handle.emit("mcp-error", err.to_string());
                }
                tauri_plugin_shell::process::CommandEvent::Terminated(payload) => {
                    println!("[MCP Server terminated] code: {:?}", payload.code);
                    let _ = app_handle.emit("mcp-terminated", payload.code);
                    // We ideally want to clear the state here, but we can't easily access it
                }
                _ => {}
            }
        }
    });

    Ok("Network Sketcher MCP Server started".to_string())
}

#[tauri::command]
pub async fn send_mcp_message(state: State<'_, McpState>, message: String) -> Result<(), String> {
    let mut process_lock = state.process.lock().map_err(|_| "Mutex lock poisoned".to_string())?;
    if let Some(child) = process_lock.as_mut() {
        let payload = format!("{}\n", message);
        child.write(payload.as_bytes())
            .map_err(|e| format!("Failed to write to MCP Server stdin: {}", e))?;
        Ok(())
    } else {
        Err("MCP Server is not running".to_string())
    }
}

pub mod dns;
pub mod fact_graph;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_config_serialization() {
        let config = DeviceConfig {
            host: "10.0.0.1".to_string(),
            username: "admin".to_string(),
            password: Some("pass".to_string()),
            enable_password: None,
            device_type: "cisco_ios".to_string(),
        };
        let serialized = serde_json::to_string(&config).unwrap();
        assert!(serialized.contains(r#""host":"10.0.0.1""#));
        assert!(serialized.contains(r#""password":"pass""#));
    }

    #[test]
    fn test_command_result_serialization() {
        let result = CommandResult {
            success: true,
            output: "show run output".to_string(),
        };
        let serialized = serde_json::to_string(&result).unwrap();
        assert_eq!(serialized, r#"{"success":true,"output":"show run output"}"#);
    }
}
