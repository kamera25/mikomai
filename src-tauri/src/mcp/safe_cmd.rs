use std::path::{Path, PathBuf};

/// Safely resolves the absolute path of a command to prevent PATH hijacking.
/// It searches only well-known system directories and does not use the user's PATH environment variable.
pub fn resolve_safe_command_path(cmd_name: &str) -> Result<PathBuf, String>
{
    if cfg!(target_os = "windows")
    {
        let windir = std::env::var("SystemRoot")
            .or_else(|_| std::env::var("windir"))
            .unwrap_or_else(|_| "C:\\Windows".to_string());
        let system32 = Path::new(&windir).join("System32");
        let cmd_path = system32.join(format!("{}.exe", cmd_name));
        if cmd_path.exists()
        {
            Ok(cmd_path)
        }
        else
        {
            Err(format!(
                "Command '{}' not found in secure system directory: {:?}",
                cmd_name, cmd_path
            ))
        }
    }
    else
    {
        // macOS & Linux
        let safe_dirs = ["/usr/sbin", "/usr/bin", "/sbin", "/bin"];
        for dir in &safe_dirs
        {
            let path = Path::new(dir).join(cmd_name);
            if path.exists()
            {
                return Ok(path);
            }
        }
        Err(format!(
            "Command '{}' not found in safe directories: {:?}",
            cmd_name, safe_dirs
        ))
    }
}

#[cfg(test)]
mod tests
{
    use super::*;

    #[test]
    fn test_resolve_safe_command_path_valid()
    {
        let cmd = if cfg!(target_os = "windows")
        {
            "arp"
        }
        else
        {
            "arp"
        };
        let path = resolve_safe_command_path(cmd);
        assert!(
            path.is_ok(),
            "Expected safe path for '{}', got: {:?}",
            cmd,
            path
        );
        let path = path.unwrap();
        assert!(path.is_absolute());
    }

    #[test]
    fn test_resolve_safe_command_path_invalid()
    {
        let path = resolve_safe_command_path("some_completely_fake_command_12345");
        assert!(path.is_err());
    }
}
