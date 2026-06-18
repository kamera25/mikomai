use std::path::PathBuf;
use tauri::Manager;

#[tauri::command]
pub async fn download_model(app: tauri::AppHandle, repo: String, filename: String) -> Result<String, String> {
    tracing::info!("Starting model download: {}/{}", repo, filename);
    
    let home = app.path().home_dir().map_err(|e: tauri::Error| e.to_string())?;
    let target_dir = std::env::var("HF_HUB_CACHE")
        .map(PathBuf::from)
        .or_else(|_| {
            std::env::var("HF_HOME")
                .map(|h| PathBuf::from(h).join("hub"))
        })
        .unwrap_or_else(|_| home.join(".cache").join("huggingface").join("hub"));

    let dest_path = target_dir.join(&repo).join(&filename);
    
    if let Some(parent) = dest_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Failed to create directories: {}", e))?;
    }

    let url = format!("https://huggingface.co/{}/resolve/main/{}", repo, filename);
    tracing::info!("Downloading from URL: {}", url);

    let client = reqwest::Client::new();
    let response = client.get(&url).send().await.map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        return Err(format!("Download failed with status: {}", response.status()));
    }

    let temp_path = dest_path.with_extension("downloading");
    
    {
        use futures::StreamExt;
        use tokio::io::AsyncWriteExt;
        
        let mut file = tokio::fs::File::create(&temp_path).await.map_err(|e| e.to_string())?;
        let mut stream = response.bytes_stream();
        
        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result.map_err(|e| e.to_string())?;
            file.write_all(&chunk).await.map_err(|e| e.to_string())?;
        }
        file.flush().await.map_err(|e| e.to_string())?;
    }
    
    tokio::fs::rename(&temp_path, &dest_path).await.map_err(|e| e.to_string())?;
    
    tracing::info!("Model available at: {:?}", dest_path);
    Ok(dest_path.to_string_lossy().to_string())
}


#[tauri::command]
pub fn open_model_dir(app: tauri::AppHandle, model_path: Option<String>) -> Result<(), String> {
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
            let home = app.path().home_dir().map_err(|e: tauri::Error| e.to_string())?;
            std::env::var("HF_HUB_CACHE")
                .map(PathBuf::from)
                .or_else(|_| {
                    std::env::var("HF_HOME")
                        .map(|h| PathBuf::from(h).join("hub"))
                })
                .unwrap_or_else(|_| home.join(".cache").join("huggingface").join("hub"))
        }
    } else {
        let home = app.path().home_dir().map_err(|e: tauri::Error| e.to_string())?;
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
        .map_err(|e| e.to_string())?;

    Ok(())
}

