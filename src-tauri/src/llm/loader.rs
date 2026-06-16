use std::sync::Arc;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::LlamaModel;
use crate::llm::llm_manager::SharedModel;
use crate::llm::llm::{LlamaState, ModelState};
use crate::llm::worker::{
    Router, InvestigateWorker, KnowledgeWorker, AnalysisWorker, RagWorker, SummarizationWorker
};
use tauri::Emitter;

#[tauri::command]
pub async fn load_model(
    app: tauri::AppHandle,
    path: String,
    state: tauri::State<'_, LlamaState>,
) -> Result<String, String> {
    {
        let mut status_lock = state.status.lock().await;
        *status_lock = ModelState::Loading;
        let _ = app.emit("model-status-changed", &*status_lock);
    }

    let backend = state.backend.clone();
    let path_clone = path.clone();
    let model_res = tokio::task::spawn_blocking(move || {
        let mut model_params = std::pin::pin!(LlamaModelParams::default());
        model_params.as_mut().add_cpu_buft_override(c".*vision.*");
        LlamaModel::load_from_file(&*backend, &path_clone, &model_params)
    }).await.map_err(|e| format!("Spawn blocking failed: {}", e))?;

    let model = match model_res {
        Ok(m) => m,
        Err(e) => {
            let err_msg = format!("Failed to load model: {}", e);
            if let Ok(mut status_lock) = state.status.try_lock() {
                *status_lock = ModelState::Error(err_msg.clone());
                let _ = app.emit("model-status-changed", &*status_lock);
            }
            return Err(err_msg);
        }
    };
    
    let model_arc = Arc::new(model);
    
    let settings = crate::settings::load_settings(app.clone()).unwrap_or_default();

    let model_clone = model_arc.clone();
    let backend_clone = state.backend.clone();
    let workers_res = tokio::task::spawn_blocking(move || -> Result<_, String> {
        let router = Router::new(&model_clone, &backend_clone)?;
        let investigate = InvestigateWorker::new(&model_clone, &backend_clone, settings.preload_investigate)?;
        let knowledge = KnowledgeWorker::new(&model_clone, &backend_clone, settings.preload_knowledge)?;
        let analysis = AnalysisWorker::new(&model_clone, &backend_clone, settings.preload_analysis)?;
        let rag = RagWorker::new(&model_clone, &backend_clone, settings.preload_rag)?;
        let summarization = SummarizationWorker::new(&model_clone, &backend_clone)?;
        Ok((router, investigate, knowledge, analysis, rag, summarization))
    }).await.map_err(|e| format!("Spawn blocking failed: {}", e))??;

    let (router, investigate, knowledge, analysis, rag, summarization) = workers_res;
    
    let mut shared_lock = state.shared.lock().await;
    *shared_lock = Some(SharedModel {
        router,
        investigate,
        knowledge,
        analysis,
        rag,
        summarization,
        model: model_arc,
        backend: state.backend.clone(),
    });
    
    {
        let mut status_lock = state.status.lock().await;
        *status_lock = ModelState::Loaded;
        let _ = app.emit("model-status-changed", &*status_lock);
    }
    
    Ok("Model loaded successfully".to_string())
}
