use serde::{Deserialize, Serialize};
use std::process::Command;
use crate::mcp::safe_cmd::resolve_safe_command_path;

#[derive(Serialize, Deserialize, Debug)]
pub struct IpInfoResult {
    pub success: bool,
    pub output: String,
}

#[tauri::command]
pub async fn network_get_ip_info(verbose: Option<bool>) -> Result<IpInfoResult, String> {
    let is_verbose = verbose.unwrap_or(false);
    let mut combined_output = String::new();
    let mut all_success = true;

    if cfg!(target_os = "windows") {
        let ipconfig_path = resolve_safe_command_path("ipconfig")?;
        let output = Command::new(&ipconfig_path)
            .arg("/all")
            .output()
            .map_err(|e| format!("Failed to execute ipconfig: {}", e))?;
        
        combined_output.push_str("--- IP Configuration ---\n");
        let output_str = String::from_utf8_lossy(&output.stdout);

        if is_verbose {
            combined_output.push_str(&output_str);
        } else {
            let keywords = ["IPv4", "IPv6", "Physical Address", "物理アドレス", "Windows IP"];
            for line in output_str.lines() {
                let trimmed = line.trim();
                // Interface lines in ipconfig often start with no indent or are adapter names
                if !line.starts_with(' ') && !line.starts_with('\t') && !trimmed.is_empty() {
                    combined_output.push_str(line);
                    combined_output.push('\n');
                } else if keywords.iter().any(|k| trimmed.contains(k)) {
                    combined_output.push_str(line);
                    combined_output.push('\n');
                }
            }
        }

        all_success = output.status.success();
    } else {
        // macOS and Linux
        combined_output.push_str("--- Interfaces & IP Addresses ---\n");
        let ifconfig_path = resolve_safe_command_path("ifconfig")?;
        let ifconfig = Command::new(&ifconfig_path)
            .output()
            .map_err(|e| format!("Failed to execute ifconfig: {}", e))?;

        let output_str = String::from_utf8_lossy(&ifconfig.stdout);
        if is_verbose {
            combined_output.push_str(&output_str);
        } else {
            let keywords = ["inet ", "inet6 ", "ether "];
            for line in output_str.lines() {
                let trimmed = line.trim();
                if !line.starts_with(' ') && !line.starts_with('\t') && !trimmed.is_empty() {
                    combined_output.push_str(line);
                    combined_output.push('\n');
                } else if keywords.iter().any(|k| trimmed.contains(k)) {
                    combined_output.push_str(line);
                    combined_output.push('\n');
                }
            }
        }

        if !ifconfig.status.success() { all_success = false; }

        if is_verbose {
            combined_output.push_str("\n--- Routing Table (Gateway) ---\n");
            let netstat_path = resolve_safe_command_path("netstat")?;
            let netstat = Command::new(&netstat_path)
                .args(["-rn"])
                .output()
                .map_err(|e| format!("Failed to execute netstat: {}", e))?;
            combined_output.push_str(&String::from_utf8_lossy(&netstat.stdout));
            if !netstat.status.success() { all_success = false; }

            combined_output.push_str("\n--- DNS Configuration ---\n");
            let scutil_path = resolve_safe_command_path("scutil")?;
            let scutil = Command::new(&scutil_path)
                .arg("--dns")
                .output();

            match scutil {
                Ok(output) => {
                    combined_output.push_str(&String::from_utf8_lossy(&output.stdout));
                    if !output.status.success() { all_success = false; }
                },
                Err(_) => {
                    // Fallback to /etc/resolv.conf if scutil fails
                    let resolv_content = std::fs::read_to_string("/etc/resolv.conf")
                        .map_err(|e| format!("Failed to read resolv.conf: {}", e))?;
                    combined_output.push_str(&resolv_content);
                }
            }
        }
    }

    Ok(IpInfoResult {
        success: all_success,
        output: combined_output,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ip_info_result_serialization() {
        let result = IpInfoResult {
            success: true,
            output: "ip info".to_string(),
        };
        let serialized = serde_json::to_string(&result).unwrap();
        assert_eq!(serialized, r#"{"success":true,"output":"ip info"}"#);
    }
}
