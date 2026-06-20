use std::fs;
use std::process::Command;
use base64::{Engine as _, engine::general_purpose};
use crate::network::CommandResult;
use crate::snapshot::SnapshotManager;

/// Compiles nwdiag DSL code to SVG using the Python wrapper script.
/// This helper is free of Tauri-specific structs so it can be easily unit tested.
pub fn compile_nwdiag_to_svg(schema: &str) -> Result<Vec<u8>, String> {
    if schema.trim().is_empty() {
        return Err("Schema cannot be empty".to_string());
    }

    let mut current_dir = std::env::current_dir()
        .map_err(|e| format!("Failed to get current directory: {}", e))?;
    if current_dir.ends_with("src-tauri") {
        current_dir.pop();
    }
    
    let python_path = current_dir.join("venv").join("bin").join("python");
    let wrapper_path = current_dir.join("src-tauri").join("python").join("nwdiag_wrapper.py");

    if !python_path.exists() {
        return Err(format!("Python virtual environment binary not found at {:?}", python_path));
    }
    if !wrapper_path.exists() {
        return Err(format!("nwdiag wrapper script not found at {:?}", wrapper_path));
    }

    let temp_dir = current_dir.join("src-tauri").join("target").join("tmp_nwdiag");
    if !temp_dir.exists() {
        fs::create_dir_all(&temp_dir)
            .map_err(|e| format!("Failed to create temporary directory: {}", e))?;
    }

    let uuid_str = uuid::Uuid::new_v4().to_string();
    let diag_path = temp_dir.join(format!("{}.diag", uuid_str));
    let svg_path = temp_dir.join(format!("{}.svg", uuid_str));

    fs::write(&diag_path, schema)
        .map_err(|e| format!("Failed to write schema to temp file: {}", e))?;

    let output = Command::new(&python_path)
        .arg(&wrapper_path)
        .arg("-T")
        .arg("svg")
        .arg("-o")
        .arg(&svg_path)
        .arg(&diag_path)
        .output();

    let _ = fs::remove_file(&diag_path);

    let output = output.map_err(|e| format!("Failed to run nwdiag command: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let _ = fs::remove_file(&svg_path);
        return Err(format!(
            "nwdiag compilation failed:\nStderr: {}\nStdout: {}",
            stderr, stdout
        ));
    }

    if !svg_path.exists() {
        return Err("nwdiag completed successfully but output SVG was not found".to_string());
    }

    let svg_content = fs::read(&svg_path)
        .map_err(|e| format!("Failed to read compiled SVG file: {}", e))?;

    let _ = fs::remove_file(&svg_path);

    Ok(svg_content)
}

#[tauri::command]
pub async fn self_network_nwdiag(
    app: tauri::AppHandle,
    schema: String,
) -> Result<CommandResult, String> {
    let svg_content = compile_nwdiag_to_svg(&schema)?;

    let b64_encoded = general_purpose::STANDARD.encode(&svg_content);
    let data_url = format!("data:image/svg+xml;base64,{}", b64_encoded);
    let output_markdown = format!("![Network Diagram]({})", data_url);

    let mut manager = SnapshotManager::new(&app)
        .map_err(|e| format!("Failed to initialize SnapshotManager: {}", e))?;
    
    let svg_string = String::from_utf8(svg_content)
        .map_err(|e| format!("SVG content is not valid UTF-8: {}", e))?;

    let saved_path = manager.save_artifact("network", "diagram.svg", &svg_string)
        .map_err(|e| format!("Failed to save artifact: {}", e))?;

    let _ = manager.update_current_link(saved_path.parent().unwrap());

    Ok(CommandResult {
        success: true,
        output: output_markdown,
        saved_path: Some(saved_path.to_string_lossy().to_string()),
        is_cached: None,
        cache_time: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compile_empty_schema() {
        let result = compile_nwdiag_to_svg("");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Schema cannot be empty");
    }

    #[test]
    fn test_compile_valid_schema() {
        let schema = r#"
            nwdiag {
                network dmz {
                    web01;
                    web02;
                }
            }
        "#;
        let result = compile_nwdiag_to_svg(schema);
        assert!(result.is_ok(), "Expected compile success, got: {:?}", result);
        let bytes = result.unwrap();
        assert!(!bytes.is_empty());
        let svg_str = String::from_utf8_lossy(&bytes);
        assert!(svg_str.contains("<svg"), "Expected SVG content, got: {}", svg_str);
    }

    #[test]
    fn test_compile_invalid_schema() {
        let schema = "invalid syntax {";
        let result = compile_nwdiag_to_svg(schema);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("nwdiag compilation failed"));
    }
}
