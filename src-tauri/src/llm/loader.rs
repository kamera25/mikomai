use std::sync::Arc;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::LlamaModel;
use crate::llm::llm_manager::SharedModel;
use crate::llm::llm::{LlamaState, ModelState};

#[tauri::command]
pub fn load_model(path: String, state: tauri::State<'_, LlamaState>) -> Result<String, String> {
    {
        let mut status_lock = state.status.lock().map_err(|_| "Mutex lock poisoned".to_string())?;
        *status_lock = ModelState::Loading;
    }

    let mut model_params = std::pin::pin!(LlamaModelParams::default());

    // Add overrides to move Vision tensors to CPU (null backend to skip loading to VRAM/saving memory)
    model_params.as_mut().add_cpu_buft_override(c".*vision.*");

    let model = match LlamaModel::load_from_file(&*state.backend, &path, &model_params) {
        Ok(m) => m,
        Err(e) => {
            let err_msg = format!("Failed to load model: {}", e);
            if let Ok(mut status_lock) = state.status.lock() {
                *status_lock = ModelState::Error(err_msg.clone());
            }
            return Err(err_msg);
        }
    };
    
    let mut shared_lock = state.shared.lock().map_err(|_| "Mutex lock poisoned".to_string())?;
    *shared_lock = Some(SharedModel {
        model: Arc::new(model),
        backend: state.backend.clone(),
    });
    
    {
        let mut status_lock = state.status.lock().map_err(|_| "Mutex lock poisoned".to_string())?;
        *status_lock = ModelState::Loaded;
    }
    
    Ok("Model loaded successfully".to_string())
}
