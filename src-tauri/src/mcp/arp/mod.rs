pub mod canonical;
pub mod llm;
pub mod macos;
pub mod windows;

use crate::mcp::safe_cmd::resolve_safe_command_path;
use std::process::Command;

/// Returns whether a target refers to the machine running Mikomai rather than
/// a registered network device.
pub fn is_localhost_target(target: &str) -> bool {
    matches!(
        target.trim().to_ascii_lowercase().as_str(),
        "localhost" | "127.0.0.1" | "::1" | "自機"
    )
}

/// Collect raw local ARP output for get_state's graph read-through path.
pub(crate) fn collect_local_arp() -> Result<String, String> {
    let arp_path = resolve_safe_command_path("arp")?;
    let output = Command::new(&arp_path)
        .arg("-a")
        .output()
        .map_err(|error| format!("Failed to execute arp -a: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "arp -a failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_localhost_aliases() {
        assert!(is_localhost_target("localhost"));
        assert!(is_localhost_target(" 127.0.0.1 "));
        assert!(is_localhost_target("::1"));
        assert!(is_localhost_target("自機"));
        assert!(!is_localhost_target("router-01"));
    }
}
