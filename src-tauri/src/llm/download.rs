use std::path::PathBuf;
use tauri::Manager;
use crate::error::TauriError;
use crate::llm::LlmError;

#[tauri::command]
pub async fn download_model(app: tauri::AppHandle, repo: String, filename: String) -> Result<String, TauriError> {
    if repo.contains("..") || filename.contains('/') || filename.contains('\\') || filename.contains("..") {
        return Err(TauriError(crate::error::MikomaiError::Validation("Invalid path in repo or filename".to_string())));
    }
    let res = download_model_inner(app, repo, filename).await?;
    Ok(res)
}

#[tauri::command]
pub fn check_model_exists(app: tauri::AppHandle, repo: String, filename: String) -> Result<bool, TauriError> {
    if repo.contains("..") || filename.contains('/') || filename.contains('\\') || filename.contains("..") {
        return Ok(false);
    }
    let home = match app.path().home_dir() {
        Ok(h) => h,
        Err(_) => return Ok(false),
    };
    let target_dir = std::env::var("HF_HUB_CACHE")
        .map(PathBuf::from)
        .or_else(|_| {
            std::env::var("HF_HOME")
                .map(|h| PathBuf::from(h).join("hub"))
        })
        .unwrap_or_else(|_| home.join(".cache").join("huggingface").join("hub"));

    let dest_path = target_dir.join(&repo).join(&filename);
    Ok(dest_path.exists())
}

async fn download_model_inner(app: tauri::AppHandle, repo: String, filename: String) -> Result<String, LlmError> {
    tracing::info!("Starting model download: {}/{}", repo, filename);
    
    let home = app.path().home_dir().map_err(|_| LlmError::HomeDirResolution)?;
    let target_dir = std::env::var("HF_HUB_CACHE")
        .map(PathBuf::from)
        .or_else(|_| {
            std::env::var("HF_HOME")
                .map(|h| PathBuf::from(h).join("hub"))
        })
        .unwrap_or_else(|_| home.join(".cache").join("huggingface").join("hub"));

    let dest_path = target_dir.join(&repo).join(&filename);
    
    if let Some(parent) = dest_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let url = format!("https://huggingface.co/{}/resolve/main/{}", repo, filename);
    tracing::info!("Downloading from URL: {}", url);

    let client = reqwest::Client::new();
    let response = client.get(&url).send().await?;

    if !response.status().is_success() {
        return Err(LlmError::DownloadStatus(response.status().to_string()));
    }

    let temp_path = dest_path.with_extension("downloading");
    
    {
        use futures::StreamExt;
        use tokio::io::AsyncWriteExt;
        
        let mut file = tokio::fs::File::create(&temp_path).await?;
        let mut stream = response.bytes_stream();
        
        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result?;
            file.write_all(&chunk).await?;
        }
        file.flush().await?;
    }
    
    tokio::fs::rename(&temp_path, &dest_path).await?;
    
    tracing::info!("Model available at: {:?}", dest_path);
    Ok(dest_path.to_string_lossy().to_string())
}


#[tauri::command]
pub fn open_model_dir(app: tauri::AppHandle, model_path: Option<String>) -> Result<(), TauriError> {
    if let Some(ref path) = model_path {
        if path.contains("..") {
            return Err(TauriError(crate::error::MikomaiError::Validation("Path traversal detected".to_string())));
        }
    }
    open_model_dir_inner(app, model_path)?;
    Ok(())
}

fn open_model_dir_inner(app: tauri::AppHandle, model_path: Option<String>) -> Result<(), LlmError> {
    use std::path::PathBuf;
    use tauri::Manager;
    use tauri_plugin_opener::OpenerExt;

    let target_dir = if let Some(ref path_str) = model_path {
        let path = PathBuf::from(path_str);
        if path.exists() {
            if path.is_file() {
                path.parent().map(|p| p.to_path_buf()).unwrap_or(path)
            } else {
                path
            }
        } else {
            let home = app.path().home_dir().map_err(LlmError::Tauri)?;
            std::env::var("HF_HUB_CACHE")
                .map(PathBuf::from)
                .or_else(|_| {
                    std::env::var("HF_HOME")
                        .map(|h| PathBuf::from(h).join("hub"))
                })
                .unwrap_or_else(|_| home.join(".cache").join("huggingface").join("hub"))
        }
    } else {
        let home = app.path().home_dir().map_err(LlmError::Tauri)?;
        std::env::var("HF_HUB_CACHE")
            .map(PathBuf::from)
            .or_else(|_| {
                std::env::var("HF_HOME")
                    .map(|h| PathBuf::from(h).join("hub"))
            })
            .unwrap_or_else(|_| home.join(".cache").join("huggingface").join("hub"))
    };

    if !target_dir.exists() {
        let _ = std::fs::create_dir_all(&target_dir);
    }

    app.opener().open_path(target_dir.to_string_lossy().to_string(), None::<String>)
        .map_err(|e| LlmError::Opener(e.to_string()))?;

    Ok(())
}

#[tauri::command]
pub fn open_path_in_file_manager(app: tauri::AppHandle, path: String) -> Result<(), TauriError> {
    use std::path::PathBuf;
    use tauri_plugin_opener::OpenerExt;

    if path.contains("..") {
        return Err(TauriError(crate::error::MikomaiError::Validation("Path traversal detected".to_string())));
    }

    let p = PathBuf::from(&path);
    let target = if p.is_file() {
        p.parent().map(|parent| parent.to_path_buf()).unwrap_or(p)
    } else {
        p
    };

    if !target.exists() {
        let _ = std::fs::create_dir_all(&target);
    }

    app.opener().open_path(target.to_string_lossy().to_string(), None::<String>)
        .map_err(|e| TauriError(crate::error::MikomaiError::Llm(LlmError::Opener(e.to_string()))))?;

    Ok(())
}

#[tauri::command]
pub fn copy_file_to_destination(src_path: String, dest_path: String) -> Result<(), TauriError> {
    use std::path::PathBuf;

    let src = PathBuf::from(&src_path);
    let dest = PathBuf::from(&dest_path);

    if !src.exists() {
        return Err(TauriError(crate::error::MikomaiError::Validation(format!(
            "Source file not found: {}",
            src_path
        ))));
    }

    if let Some(parent) = dest.parent() {
        if !parent.exists() {
            let _ = std::fs::create_dir_all(parent);
        }
    }

    std::fs::copy(&src, &dest).map_err(|e| {
        TauriError(crate::error::MikomaiError::Io(e))
    })?;

    Ok(())
}


