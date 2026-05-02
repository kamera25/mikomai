use serde::{Deserialize, Serialize};
use std::process::Command;
use std::path::PathBuf;
use tauri::AppHandle;

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

// Implementation using a Python Subprocess (Netmiko) fallback
pub struct PythonNetmikoWrapper {
    script_path: PathBuf,
}

impl PythonNetmikoWrapper {
    pub fn new(app: &AppHandle) -> Self {
        // In a real build, we'd resolve this via `app.path().resource_dir()`
        // For development, we assume it's in `src-tauri/python/netmiko_wrapper.py`
        let mut path = std::env::current_dir().unwrap_or_default();
        path.push("python");
        path.push("netmiko_wrapper.py");
        Self { script_path: path }
    }

    fn run_python_script(&self, args: Vec<String>) -> Result<String, String> {
        let output = Command::new("python3")
            .arg(&self.script_path)
            .args(&args)
            .output()
            .map_err(|e| format!("Failed to execute python script: {}", e))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }
}

impl NetworkInterface for PythonNetmikoWrapper {
    fn execute_show(&self, device: &DeviceConfig, command: &str) -> Result<String, String> {
        let args = vec![
            "--action".to_string(), "show".to_string(),
            "--host".to_string(), device.host.clone(),
            "--username".to_string(), device.username.clone(),
            "--password".to_string(), device.password.clone().unwrap_or_default(),
            "--device_type".to_string(), device.device_type.clone(),
            "--command".to_string(), command.to_string()
        ];
        self.run_python_script(args)
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
        self.run_python_script(args)
    }
}

#[tauri::command]
pub async fn network_show(
    app: AppHandle,
    device: DeviceConfig,
    command: String,
) -> Result<CommandResult, String> {
    println!("Executing read-only command on {}: {}", device.host, command);
    let wrapper = PythonNetmikoWrapper::new(&app);
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
    let wrapper = PythonNetmikoWrapper::new(&app);
    match wrapper.execute_config(&device, commands) {
        Ok(output) => Ok(CommandResult { success: true, output }),
        Err(err) => Ok(CommandResult { success: false, output: err }),
    }
}
