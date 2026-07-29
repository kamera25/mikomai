use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_shell::ShellExt;
use crate::error::TauriError;
use validator::Validate;

#[derive(Debug, thiserror::Error)]
pub enum NetworkError {
    #[error("Failed to serialize payload: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Failed to create sidecar command: {0}")]
    SidecarCreate(String),
    #[error("Failed to spawn sidecar: {0}")]
    SidecarSpawn(String),
    #[error("Failed to write to sidecar stdin: {0}")]
    SidecarWrite(String),
    #[error("Sidecar error: {0}")]
    SidecarError(String),
    #[error("Sidecar failed with exit code {0}: {1}")]
    SidecarFailed(i32, String),
    #[error("Sidecar completed unexpectedly")]
    SidecarUnexpectedCompletion,
    #[error("Mutex lock poisoned")]
    PoisonedLock,
    #[error("Spawn blocking failed: {0}")]
    SpawnBlocking(String),
    #[error("Connection resolution failed: {0}")]
    ConnectionError(#[from] crate::connections::ConnectionError),
}

#[derive(Serialize, Deserialize, Clone, Debug, Validate)]
#[serde(rename_all = "camelCase")]
pub struct NetmikoDeviceConfig {
    #[validate(length(min = 1))]
    pub host: String,
    #[validate(length(min = 1))]
    pub username: String,
    pub password: Option<String>,
    #[serde(alias = "enable_password")]
    pub enable_password: Option<String>,
    #[serde(alias = "device_type")]
    #[validate(length(min = 1))]
    pub device_type: String,
    #[serde(default, alias = "console_port")]
    pub console_port: Option<String>,
    #[serde(default, alias = "console_baud_rate")]
    #[validate(range(min = 110, max = 1000000))]
    pub console_baud_rate: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CommandResult {
    pub success: bool,
    pub output: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saved_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_cached: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_time: Option<String>,
}

// Abstract trait for network operations
pub trait NetworkInterface {
    async fn execute_show(&self, device: &NetmikoDeviceConfig, command: &str) -> Result<String, NetworkError>;
    async fn execute_config(&self, device: &NetmikoDeviceConfig, commands: Vec<String>) -> Result<String, NetworkError>;
}

// Implementation using a Tauri Sidecar fallback
pub struct SidecarNetmikoWrapper {
    app: AppHandle,
}

impl SidecarNetmikoWrapper {
    pub fn new(app: &AppHandle) -> Self {
        Self { app: app.clone() }
    }

    async fn run_sidecar(&self, payload: serde_json::Value) -> Result<String, NetworkError> {
        let payload_str = serde_json::to_string(&payload)?;

        let (mut rx, mut child) = self.app.shell()
            .sidecar("netmiko_wrapper")
            .map_err(|e| NetworkError::SidecarCreate(e.to_string()))?
            .arg("--stdin")
            .spawn()
            .map_err(|e| NetworkError::SidecarSpawn(e.to_string()))?;

        child.write(format!("{}\n", payload_str).as_bytes())
            .map_err(|e| NetworkError::SidecarWrite(e.to_string()))?;

        let mut stdout = String::new();
        let mut stderr = String::new();

        while let Some(event) = rx.recv().await {
            match event {
                tauri_plugin_shell::process::CommandEvent::Stdout(line) => {
                    let text = String::from_utf8_lossy(&line).to_string();
                    stdout.push_str(&text);
                    stdout.push('\n');
                    use tauri::Emitter;
                    let _ = self.app.emit("commit-log", serde_json::json!({
                        "line": text,
                        "stream": "stdout"
                    }));
                }
                tauri_plugin_shell::process::CommandEvent::Stderr(line) => {
                    let text = String::from_utf8_lossy(&line).to_string();
                    stderr.push_str(&text);
                    stderr.push('\n');
                    use tauri::Emitter;
                    let _ = self.app.emit("commit-log", serde_json::json!({
                        "line": text,
                        "stream": "stderr"
                    }));
                }
                tauri_plugin_shell::process::CommandEvent::Error(err) => {
                    return Err(NetworkError::SidecarError(err.to_string()));
                }
                tauri_plugin_shell::process::CommandEvent::Terminated(payload) => {
                    let code = payload.code.unwrap_or(-1);
                    if code == 0 {
                        return Ok(stdout);
                    } else {
                        return Err(NetworkError::SidecarFailed(code, stderr.trim().to_string()));
                    }
                }
                _ => {}
            }
        }

        Err(NetworkError::SidecarUnexpectedCompletion)
    }
}

impl NetworkInterface for SidecarNetmikoWrapper {
    async fn execute_show(&self, device: &NetmikoDeviceConfig, command: &str) -> Result<String, NetworkError> {
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

    async fn execute_config(&self, device: &NetmikoDeviceConfig, commands: Vec<String>) -> Result<String, NetworkError> {
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

pub fn sanitize_network_command(cmd: &str) -> Result<(), String> {
    let trimmed = cmd.trim();
    if trimmed.is_empty() {
        return Err("Command cannot be empty".to_string());
    }

    // 1. Block command injection characters / shell metacharacters
    let disallowed_chars = [';', '|', '&', '$', '(', ')', '`', '>', '<', '\\', '\n', '\r', '"', '\''];
    for c in trimmed.chars() {
        if disallowed_chars.contains(&c) {
            return Err(format!("Command contains forbidden character: '{}'", c));
        }
    }

    // 2. Allowlist of safe characters
    for c in trimmed.chars() {
        if !c.is_alphanumeric() && ![' ', '-', '_', '.', '/', ':', '?', '*', '[', ']', ','].contains(&c) {
            return Err(format!("Command contains unsafe character: '{}'", c));
        }
    }

    // 3. For show commands, ensure they don't contain config keywords or destructive commands
    let lower = trimmed.to_lowercase();
    let words: Vec<&str> = lower.split_whitespace().collect();
    if words.is_empty() {
        return Err("Command cannot be empty".to_string());
    }

    let blocked_keywords = [
        "config", "configure", "write", "reload", "reboot", "erase", "delete", "copy",
        "format", "sysreq", "terminal", "enable", "disable", "configuration"
    ];
    for word in &words {
        if blocked_keywords.contains(word) {
            return Err(format!("Command contains forbidden keyword: '{}'", word));
        }
    }

    Ok(())
}

pub fn sanitize_config_command(cmd: &str) -> Result<(), String> {
    let trimmed = cmd.trim();
    if trimmed.is_empty() {
        return Err("Command cannot be empty".to_string());
    }

    // Block command injection characters / shell metacharacters
    let disallowed_chars = [';', '|', '&', '$', '(', ')', '`', '>', '<', '\\', '\n', '\r', '"', '\''];
    for c in trimmed.chars() {
        if disallowed_chars.contains(&c) {
            return Err(format!("Config command contains forbidden character: '{}'", c));
        }
    }

    // Allow only safe characters
    for c in trimmed.chars() {
        if !c.is_alphanumeric() && ![' ', '-', '_', '.', '/', ':', '?', '*', '[', ']', ','].contains(&c) {
            return Err(format!("Config command contains unsafe character: '{}'", c));
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn network_show(
    app: AppHandle,
    device: NetmikoDeviceConfig,
    command: String,
) -> Result<CommandResult, TauriError> {
    device.validate().map_err(|e| TauriError(crate::error::MikomaiError::Validation(e.to_string())))?;
    sanitize_network_command(&command).map_err(|e| TauriError(crate::error::MikomaiError::Validation(e)))?;

    let target_device = device_resolver::TargetDeviceBuilder::new(app.clone(), device)
        .resolve()
        .await?;

    if target_device.console_port().is_none() {
        log::info!("Executing read-only command on {}: {}", target_device.host(), command);
    } else {
        log::info!("Executing read-only command via console port {}: {}", target_device.console_port().unwrap(), command);
    }

    let wrapper = SidecarNetmikoWrapper::new(&app);
    match wrapper.execute_show(&target_device.to_netmiko_config(), &command).await {
        Ok(output) => Ok(CommandResult { success: true, output, saved_path: None, is_cached: None, cache_time: None }),
        Err(err) => Ok(CommandResult { success: false, output: err.to_string(), saved_path: None, is_cached: None, cache_time: None }),
    }
}

#[tauri::command]
pub async fn network_config(
    app: AppHandle,
    device: NetmikoDeviceConfig,
    commands: Vec<String>,
) -> Result<CommandResult, TauriError> {
    device.validate().map_err(|e| TauriError(crate::error::MikomaiError::Validation(e.to_string())))?;
    for cmd in &commands {
        sanitize_config_command(cmd).map_err(|e| TauriError(crate::error::MikomaiError::Validation(e)))?;
    }

    let target_device = device_resolver::TargetDeviceBuilder::new(app.clone(), device)
        .resolve()
        .await?;

    if target_device.console_port().is_none() {
        log::info!("Executing WRITE command on {}: {:?}", target_device.host(), commands);
    } else {
        log::info!("Executing WRITE command via console port {:?}: {:?}", target_device.console_port().unwrap(), commands);
    }

    let wrapper = SidecarNetmikoWrapper::new(&app);
    match wrapper.execute_config(&target_device.to_netmiko_config(), commands).await {
        Ok(output) => Ok(CommandResult { success: true, output, saved_path: None, is_cached: None, cache_time: None }),
        Err(err) => Ok(CommandResult { success: false, output: err.to_string(), saved_path: None, is_cached: None, cache_time: None }),
    }
}


pub mod dns;
pub mod fact_graph;
pub mod device_resolver;

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
            is_cached: None,
            cache_time: None,
        };
        let serialized = serde_json::to_string(&result).unwrap();
        assert_eq!(serialized, r#"{"success":true,"output":"show run output"}"#);
    }

    #[test]
    fn test_netmiko_device_config_validation() {
        let mut config = NetmikoDeviceConfig {
            host: "".to_string(),
            username: "admin".to_string(),
            password: Some("pass".to_string()),
            enable_password: None,
            device_type: "cisco_ios".to_string(),
            console_port: None,
            console_baud_rate: Some(9600),
        };
        assert!(config.validate().is_err()); // empty host

        config.host = "10.0.0.1".to_string();
        assert!(config.validate().is_ok());

        config.console_baud_rate = Some(50); // too low
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_sanitize_network_command() {
        assert!(sanitize_network_command("show ip interface brief").is_ok());
        assert!(sanitize_network_command("show version").is_ok());
        assert!(sanitize_network_command("show run").is_ok());
        assert!(sanitize_network_command("configure terminal").is_err());
        assert!(sanitize_network_command("show run; rm -rf /").is_err()); // contains semicolon
        assert!(sanitize_network_command("show version | include 12.4").is_err()); // contains pipe
        assert!(sanitize_network_command("").is_err()); // empty
    }

    #[test]
    fn test_sanitize_config_command() {
        assert!(sanitize_config_command("interface GigabitEthernet1/1").is_ok());
        assert!(sanitize_config_command("ip address 192.168.1.1 255.255.255.0").is_ok());
        assert!(sanitize_config_command("no shutdown").is_ok());
        assert!(sanitize_config_command("interface GigabitEthernet1/1; rm -rf /").is_err()); // contains semicolon
    }
}
