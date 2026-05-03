use serde::{Deserialize, Serialize};
use std::process::Command;

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
        let output = Command::new("ipconfig")
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
        let ifconfig = Command::new("ifconfig")
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
            let netstat = Command::new("netstat")
                .args(["-rn"])
                .output()
                .map_err(|e| format!("Failed to execute netstat: {}", e))?;
            combined_output.push_str(&String::from_utf8_lossy(&netstat.stdout));
            if !netstat.status.success() { all_success = false; }

            combined_output.push_str("\n--- DNS Configuration ---\n");
            let scutil = Command::new("scutil")
                .arg("--dns")
                .output();

            match scutil {
                Ok(output) => {
                    combined_output.push_str(&String::from_utf8_lossy(&output.stdout));
                    if !output.status.success() { all_success = false; }
                },
                Err(_) => {
                    // Fallback to /etc/resolv.conf if scutil fails
                    let resolv = Command::new("cat")
                        .arg("/etc/resolv.conf")
                        .output()
                        .map_err(|e| format!("Failed to read resolv.conf: {}", e))?;
                    combined_output.push_str(&String::from_utf8_lossy(&resolv.stdout));
                }
            }
        }
    }

    Ok(IpInfoResult {
        success: all_success,
        output: combined_output,
    })
}
