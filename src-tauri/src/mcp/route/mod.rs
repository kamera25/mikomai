use crate::mcp::safe_cmd::resolve_safe_command_path;
use serde::{Deserialize, Serialize};
use std::process::Command;

pub mod llm;
pub mod macos;
pub mod windows;
pub mod yaml;

#[derive(Serialize, Deserialize, Debug)]
pub struct RouteResult
{
    pub success: bool,
    pub output: String,
    pub parsed: Option<crate::schema::route::UniversalRouteTable>,
    #[serde(rename = "savedPath")]
    pub saved_path: Option<std::path::PathBuf>,
}

impl From<RouteResult> for crate::network::CommandResult
{
    fn from(res: RouteResult) -> Self
    {
        Self {
            success: res.success,
            output: res.output,
            saved_path: res.saved_path,
            is_cached: None,
            cache_time: None,
        }
    }
}

#[tauri::command]
pub async fn self_network_route(app: tauri::AppHandle) -> Result<RouteResult, String>
{
    let is_windows = cfg!(target_os = "windows");
    let output = if is_windows
    {
        let route_path = resolve_safe_command_path("route")?;
        Command::new(&route_path).arg("print").output()
    }
    else
    {
        // macOS and Linux
        let netstat_path = resolve_safe_command_path("netstat")?;
        Command::new(&netstat_path).arg("-rn").output()
    }
    .map_err(|e| format!("Failed to execute route/netstat command: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    let success = output.status.success();
    let display_output = if success
    {
        stdout.clone()
    }
    else
    {
        format!("Error: {}\n{}", stderr, stdout)
    };

    let mut saved_path = None;
    let parsed = if success
    {
        let parsed_table = if is_windows
        {
            windows::parse_windows_route(&stdout).ok()
        }
        else
        {
            macos::parse_macos_route(&stdout).ok()
        };

        if let Some(ref table) = parsed_table
        {
            if let Ok(yaml_content) = serde_yaml::to_string(table)
            {
                match yaml::save_validated_yaml(&app, "localhost", &yaml_content)
                {
                    Ok(path) =>
                    {
                        saved_path = Some(path);
                    }
                    Err(e) =>
                    {
                        log::error!("Failed to save local route table yaml: {}", e);
                    }
                }
            }
        }
        parsed_table
    }
    else
    {
        None
    };

    Ok(RouteResult {
        success,
        output: display_output,
        parsed,
        saved_path,
    })
}

#[cfg(test)]
mod tests
{
    use super::*;

    #[test]
    fn test_route_result_serialization()
    {
        let result = RouteResult {
            success: true,
            output: "routing info".to_string(),
            parsed: None,
            saved_path: None,
        };
        let serialized = serde_json::to_string(&result).unwrap();
        assert_eq!(
            serialized,
            r#"{"success":true,"output":"routing info","parsed":null,"savedPath":null}"#
        );
    }
}
