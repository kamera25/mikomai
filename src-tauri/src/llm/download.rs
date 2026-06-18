use hf_hub::api::tokio::Api;

#[tauri::command]
pub async fn download_model(repo: String, filename: String) -> Result<String, String> {
    tracing::info!("Starting model download: {}/{}", repo, filename);
    let api = Api::new().map_err(|e| e.to_string())?;
    let api_repo = api.model(repo);
    let path = api_repo.get(&filename).await.map_err(|e| e.to_string())?;
    tracing::info!("Model available at: {:?}", path);
    Ok(path.to_string_lossy().to_string())
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

