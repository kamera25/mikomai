use hf_hub::api::tokio::Api;

#[tauri::command]
pub async fn download_model(repo: String, filename: String) -> Result<String, String> {
    println!("Starting model download: {}/{}", repo, filename);
    let api = Api::new().map_err(|e| e.to_string())?;
    let api_repo = api.model(repo);
    let path = api_repo.get(&filename).await.map_err(|e| e.to_string())?;
    println!("Model available at: {:?}", path);
    Ok(path.to_string_lossy().to_string())
}
