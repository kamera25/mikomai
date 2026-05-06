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

    async fn run_sidecar(&self, args: Vec<String>) -> Result<String, String> {
        let sidecar = self.app.shell()
            .sidecar("netmiko_wrapper")
            .map_err(|e| format!("Failed to create sidecar command: {}", e))?
            .args(args);

        let output = sidecar.output().await
            .map_err(|e| format!("Failed to execute sidecar: {}", e))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }
}

impl NetworkInterface for SidecarNetmikoWrapper {
    async fn execute_show(&self, device: &DeviceConfig, command: &str) -> Result<String, String> {
        let args = vec![
            "--action".to_string(), "show".to_string(),
            "--host".to_string(), device.host.clone(),
            "--username".to_string(), device.username.clone(),
            "--password".to_string(), device.password.clone().unwrap_or_default(),
            "--device_type".to_string(), device.device_type.clone(),
            "--command".to_string(), command.to_string()
        ];
        self.run_sidecar(args).await
    }

    async fn execute_config(&self, device: &DeviceConfig, commands: Vec<String>) -> Result<String, String> {
        let commands_json = serde_json::to_string(&commands).unwrap_or_default();
        let args = vec![
            "--action".to_string(), "config".to_string(),
            "--host".to_string(), device.host.clone(),
            "--username".to_string(), device.username.clone(),
            "--password".to_string(), device.password.clone().unwrap_or_default(),
            "--device_type".to_string(), device.device_type.clone(),
            "--commands".to_string(), commands_json
        ];
        self.run_sidecar(args).await
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
    if let Some((ip, user, password, dtype)) = get_device_config(&app, &device.host) {
        target_device.host = ip;
        target_device.username = user;
        if password.is_some() {
            target_device.password = password;
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
    if let Some((ip, user, password, dtype)) = get_device_config(&app, &device.host) {
        target_device.host = ip;
        target_device.username = user;
        if password.is_some() {
            target_device.password = password;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_config_serialization() {
        let config = DeviceConfig {
            host: "10.0.0.1".to_string(),
            username: "admin".to_string(),
            password: Some("pass".to_string()),
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
