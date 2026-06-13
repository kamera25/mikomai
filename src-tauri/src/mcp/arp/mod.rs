pub mod llm;
pub mod yaml;
pub mod macos;
pub mod windows;

use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Serialize, Deserialize, Debug)]
pub struct ArpResult {
    pub success: bool,
    pub output: String,
    pub parsed: Option<crate::schema::arp::UniversalArpTable>,
    #[serde(rename = "savedPath")]
    pub saved_path: Option<String>,
}

#[tauri::command]
pub async fn self_network_arp() -> Result<ArpResult, String> {
    // On macOS and Linux, 'arp -an' is a standard way to get the ARP table
    // On Windows, 'arp -a' is used.
    
    let is_windows = cfg!(target_os = "windows");
    let output = if is_windows {
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

    let success = output.status.success();
    let display_output = if success {
        stdout.clone()
    } else {
        format!("Error: {}\n{}", stderr, stdout)
    };

    let mut saved_path = None;
    let parsed = if success {
        let parsed_table = if is_windows {
            windows::parse_windows_arp(&stdout).ok()
        } else {
            macos::parse_macos_arp(&stdout).ok()
        };

        if let Some(ref table) = parsed_table {
            if let Ok(yaml_content) = serde_yaml::to_string(table) {
                match yaml::save_validated_yaml("localhost", &yaml_content) {
                    Ok(path) => {
                        saved_path = Some(path);
                    }
                    Err(e) => {
                        println!("Failed to save local ARP table yaml: {}", e);
                    }
                }
            }
        }
        parsed_table
    } else {
        None
    };

    Ok(ArpResult {
        success,
        output: display_output,
        parsed,
        saved_path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arp_result_serialization() {
        let result = ArpResult {
            success: true,
            output: "arp info".to_string(),
            parsed: None,
            saved_path: None,
        };
        let serialized = serde_json::to_string(&result).unwrap();
        assert_eq!(serialized, r#"{"success":true,"output":"arp info","parsed":null,"savedPath":null}"#);
    }
}

