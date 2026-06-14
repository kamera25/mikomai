use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_shell::{process::CommandChild, ShellExt};
use crate::connections::get_device_config;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct NetmikoDeviceConfig {
    pub host: String,
    pub username: String,
    pub password: Option<String>,
    #[serde(alias = "enable_password")]
    pub enable_password: Option<String>,
    #[serde(alias = "device_type")]
    pub device_type: String,
    #[serde(default, alias = "console_port")]
    pub console_port: Option<String>,
    #[serde(default, alias = "console_baud_rate")]
    pub console_baud_rate: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CommandResult {
    pub success: bool,
    pub output: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saved_path: Option<String>,
}

// Abstract trait for network operations
pub trait NetworkInterface {
    async fn execute_show(&self, device: &NetmikoDeviceConfig, command: &str) -> Result<String, String>;
    async fn execute_config(&self, device: &NetmikoDeviceConfig, commands: Vec<String>) -> Result<String, String>;
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
    async fn execute_show(&self, device: &NetmikoDeviceConfig, command: &str) -> Result<String, String> {
        let mut payload = serde_json::json!({
            "action": "show",
            "username": device.username,
            "password": device.password.clone().unwrap_or_default(),
            "secret": device.enable_password.clone().unwrap_or_default(),
            "device_type": device.device_type,
            "command": command,
            "console_port": device.console_port,
            "console_baud_rate": device.console_baud_rate,
        });
        if device.console_port.is_none() {
            payload["host"] = serde_json::json!(device.host);
        }
        self.run_sidecar(payload).await
    }

    async fn execute_config(&self, device: &NetmikoDeviceConfig, commands: Vec<String>) -> Result<String, String> {
        let mut payload = serde_json::json!({
            "action": "config",
            "username": device.username,
            "password": device.password.clone().unwrap_or_default(),
            "secret": device.enable_password.clone().unwrap_or_default(),
            "device_type": device.device_type,
            "commands": commands,
            "console_port": device.console_port,
            "console_baud_rate": device.console_baud_rate,
        });
        if device.console_port.is_none() {
            payload["host"] = serde_json::json!(device.host);
        }
        self.run_sidecar(payload).await
    }
}

#[tauri::command]
pub async fn network_show(
    app: AppHandle,
    device: NetmikoDeviceConfig,
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

    // Load settings for console override if connection type is console/serial
    let mut is_console = target_device.console_port.is_some();
    if !is_console {
        if let Ok(connections) = crate::connections::load_connections(app.clone()) {
            if let Some(conn) = connections.iter().find(|c| c.hostname.eq_ignore_ascii_case(&target_device.host) || c.ip.as_str() == target_device.host) {
                if conn.conn_type == crate::connections::ConnectionType::Console {
                    is_console = true;
                }
            }
        }
    }
    if !is_console {
        if let Ok(mcp_hosts) = crate::connections::get_mcp_hosts() {
            if let Some(mcp) = mcp_hosts.iter().find(|h| h.hostname.eq_ignore_ascii_case(&target_device.host) || h.ip.as_str() == target_device.host) {
                if mcp.device_type.contains("Console") || mcp.device_type.contains("Serial") {
                    is_console = true;
                }
            }
        }
    }

    if is_console {
        let settings = crate::settings::load_settings(app.clone()).unwrap_or_default();
        if let Some(ref port) = settings.console_port {
            if !port.trim().is_empty() && port != "None" {
                target_device.console_port = Some(port.clone());
                target_device.console_baud_rate = settings.console_baud_rate;
            }
        }
    } else {
        target_device.console_port = None;
        target_device.console_baud_rate = None;
    }

    if target_device.console_port.is_none() {
        // Resolve using preference
        let host_to_resolve = target_device.host.clone();
        let app_clone = app.clone();
        let ip = tokio::task::spawn_blocking(move || {
            crate::connections::resolve_host_with_preference(&app_clone, &host_to_resolve)
        })
        .await
        .map_err(|e| e.to_string())??;
        target_device.host = ip.to_string();
        log::info!("Executing read-only command on {}: {}", target_device.host, command);
    } else {
        log::info!("Executing read-only command via console port {}: {}", target_device.console_port.as_ref().unwrap(), command);
    }

    let wrapper = SidecarNetmikoWrapper::new(&app);
    match wrapper.execute_show(&target_device, &command).await {
        Ok(output) => Ok(CommandResult { success: true, output, saved_path: None }),
        Err(err) => Ok(CommandResult { success: false, output: err, saved_path: None }),
    }
}

#[tauri::command]
pub async fn network_config(
    app: AppHandle,
    device: NetmikoDeviceConfig,
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

    // Load settings for console override if connection type is console/serial
    let mut is_console = target_device.console_port.is_some();
    if !is_console {
        if let Ok(connections) = crate::connections::load_connections(app.clone()) {
            if let Some(conn) = connections.iter().find(|c| c.hostname.eq_ignore_ascii_case(&target_device.host) || c.ip.as_str() == target_device.host) {
                if conn.conn_type == crate::connections::ConnectionType::Console {
                    is_console = true;
                }
            }
        }
    }
    if !is_console {
        if let Ok(mcp_hosts) = crate::connections::get_mcp_hosts() {
            if let Some(mcp) = mcp_hosts.iter().find(|h| h.hostname.eq_ignore_ascii_case(&target_device.host) || h.ip.as_str() == target_device.host) {
                if mcp.device_type.contains("Console") || mcp.device_type.contains("Serial") {
                    is_console = true;
                }
            }
        }
    }

    if is_console {
        let settings = crate::settings::load_settings(app.clone()).unwrap_or_default();
        if let Some(ref port) = settings.console_port {
            if !port.trim().is_empty() && port != "None" {
                target_device.console_port = Some(port.clone());
                target_device.console_baud_rate = settings.console_baud_rate;
            }
        }
    } else {
        target_device.console_port = None;
        target_device.console_baud_rate = None;
    }

    if target_device.console_port.is_none() {
        // Resolve using preference
        let host_to_resolve = target_device.host.clone();
        let app_clone = app.clone();
        let ip = tokio::task::spawn_blocking(move || {
            crate::connections::resolve_host_with_preference(&app_clone, &host_to_resolve)
        })
        .await
        .map_err(|e| e.to_string())??;
        target_device.host = ip.to_string();
        log::info!("Executing WRITE command on {}: {:?}", target_device.host, commands);
    } else {
        log::info!("Executing WRITE command via console port {:?}: {:?}", target_device.console_port.as_ref().unwrap(), commands);
    }

    let wrapper = SidecarNetmikoWrapper::new(&app);
    match wrapper.execute_config(&target_device, commands).await {
        Ok(output) => Ok(CommandResult { success: true, output, saved_path: None }),
        Err(err) => Ok(CommandResult { success: false, output: err, saved_path: None }),
    }
}

pub struct McpState {
    pub process: Mutex<Option<CommandChild>>,
}

#[tauri::command]
pub async fn start_ns_mcp_server(app: AppHandle, state: State<'_, McpState>) -> Result<String, String> {
    log::info!("Starting Network Sketcher MCP Server...");

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
                    log::error!("[MCP Server stderr] {}", err);
                    let _ = app_handle.emit("mcp-error", err);
                }
                tauri_plugin_shell::process::CommandEvent::Error(err) => {
                    log::error!("[MCP Server error] {}", err);
                    let _ = app_handle.emit("mcp-error", err.to_string());
                }
                tauri_plugin_shell::process::CommandEvent::Terminated(payload) => {
                    log::info!("[MCP Server terminated] code: {:?}", payload.code);
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
        let config = NetmikoDeviceConfig {
            host: "10.0.0.1".to_string(),
            username: "admin".to_string(),
            password: Some("pass".to_string()),
            enable_password: None,
            device_type: "cisco_ios".to_string(),
            console_port: None,
            console_baud_rate: None,
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
            saved_path: None,
        };
        let serialized = serde_json::to_string(&result).unwrap();
        assert_eq!(serialized, r#"{"success":true,"output":"show run output"}"#);
    }
}
