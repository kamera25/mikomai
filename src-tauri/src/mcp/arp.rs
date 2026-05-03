use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Serialize, Deserialize, Debug)]
pub struct ArpResult {
    pub success: bool,
    pub output: String,
}

#[tauri::command]
pub async fn network_arp() -> Result<ArpResult, String> {
    // On macOS and Linux, 'arp -an' is a standard way to get the ARP table
    // On Windows, 'arp -a' is used.
    
    let output = if cfg!(target_os = "windows") {
        Command::new("arp")
            .arg("-a")
            .output()
    } else {
        // macOS and Linux
        Command::new("arp")
            .arg("-an")
            .output()
    }.map_err(|e| format!("Failed to execute arp command: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    Ok(ArpResult {
        success: output.status.success(),
        output: if output.status.success() {
            stdout
        } else {
            format!("Error: {}\n{}", stderr, stdout)
        },
    })
}
