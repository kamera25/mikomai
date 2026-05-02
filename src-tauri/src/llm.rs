use hf_hub::api::tokio::Api;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::model::params::LlamaModelParams;
use serde::Serialize;
use std::sync::Mutex;

pub struct LlamaState {
    pub backend: LlamaBackend,
    pub model: Mutex<Option<LlamaModel>>,
}

impl LlamaState {
    pub fn new() -> Result<Self, String> {
        let backend = LlamaBackend::init().map_err(|e| e.to_string())?;
        Ok(Self {
            backend,
            model: Mutex::new(None),
        })
    }
}


#[tauri::command]
pub async fn download_model(repo: String, filename: String) -> Result<String, String> {
    println!("Starting model download: {}/{}", repo, filename);
    let api = Api::new().map_err(|e| e.to_string())?;
    let api_repo = api.model(repo);
    
    // This will download the file if not cached, or return the path if cached
    let path = api_repo.get(&filename).await.map_err(|e| e.to_string())?;
    
    println!("Model available at: {:?}", path);
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub fn load_model(path: String, state: tauri::State<'_, LlamaState>) -> Result<String, String> {
    let model_params = LlamaModelParams::default();
    let model = LlamaModel::load_from_file(&state.backend, &path, &model_params)
        .map_err(|e| format!("Failed to load model: {}", e))?;
    
    let mut model_lock = state.model.lock().unwrap();
    *model_lock = Some(model);
    
    Ok("Model loaded successfully".to_string())
}
