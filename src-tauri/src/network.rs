use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::AppHandle;
use tauri_plugin_shell::ShellExt;

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
    fn execute_show(&self, device: &DeviceConfig, command: &str) -> Result<String, String>;
    fn execute_config(&self, device: &DeviceConfig, commands: Vec<String>) -> Result<String, String>;
}

// Implementation using a Tauri Sidecar fallback
pub struct SidecarNetmikoWrapper {
    app: AppHandle,
}

impl SidecarNetmikoWrapper {
    pub fn new(app: &AppHandle) -> Self {
        Self { app: app.clone() }
    }

    fn run_sidecar(&self, args: Vec<String>) -> Result<String, String> {
        let sidecar = self.app.shell()
            .sidecar("netmiko_wrapper")
            .map_err(|e| format!("Failed to create sidecar command: {}", e))?
            .args(args);

        // Run synchronously blockingly for now to match the trait
        // Note: we might want to change NetworkInterface to be async later
        let output = tauri::async_runtime::block_on(async { sidecar.output().await })
            .map_err(|e| format!("Failed to execute sidecar: {}", e))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }
}

impl NetworkInterface for SidecarNetmikoWrapper {
    fn execute_show(&self, device: &DeviceConfig, command: &str) -> Result<String, String> {
        let args = vec![
            "--action".to_string(), "show".to_string(),
            "--host".to_string(), device.host.clone(),
            "--username".to_string(), device.username.clone(),
            "--password".to_string(), device.password.clone().unwrap_or_default(),
            "--device_type".to_string(), device.device_type.clone(),
            "--command".to_string(), command.to_string()
        ];
        self.run_sidecar(args)
    }

    fn execute_config(&self, device: &DeviceConfig, commands: Vec<String>) -> Result<String, String> {
        let commands_json = serde_json::to_string(&commands).unwrap_or_default();
        let args = vec![
            "--action".to_string(), "config".to_string(),
            "--host".to_string(), device.host.clone(),
            "--username".to_string(), device.username.clone(),
            "--password".to_string(), device.password.clone().unwrap_or_default(),
            "--device_type".to_string(), device.device_type.clone(),
            "--commands".to_string(), commands_json
        ];
        self.run_sidecar(args)
    }
}

#[tauri::command]
pub async fn network_show(
    app: AppHandle,
    device: DeviceConfig,
    command: String,
) -> Result<CommandResult, String> {
    println!("Executing read-only command on {}: {}", device.host, command);
    let wrapper = SidecarNetmikoWrapper::new(&app);
    match wrapper.execute_show(&device, &command) {
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
    println!("Executing WRITE command on {}: {:?}", device.host, commands);
    let wrapper = SidecarNetmikoWrapper::new(&app);
    match wrapper.execute_config(&device, commands) {
        Ok(output) => Ok(CommandResult { success: true, output }),
        Err(err) => Ok(CommandResult { success: false, output: err }),
    }
}
