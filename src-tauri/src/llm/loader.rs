use std::sync::Arc;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::LlamaModel;
use crate::llm::llm_manager::SharedModel;
use crate::llm::llm::{LlamaState, ModelState, LlmError};
use crate::llm::worker::{
    Router, InvestigateWorker, KnowledgeWorker, AnalysisWorker, RagWorker, SummarizationWorker
};
use tauri::Emitter;
use crate::error::TauriError;

#[tauri::command]
pub async fn load_model(
    app: tauri::AppHandle,
    path: String,
    state: tauri::State<'_, LlamaState>,
) -> Result<String, TauriError> {
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
    }).await.map_err(|e| LlmError::SpawnBlocking(e.to_string()))?;

    let model = match model_res {
        Ok(m) => m,
        Err(e) => {
            let err_msg = format!("Failed to load model: {}", e);
            if let Ok(mut status_lock) = state.status.try_lock() {
                *status_lock = ModelState::Error(err_msg.clone());
                let _ = app.emit("model-status-changed", &*status_lock);
            }
            return Err(LlmError::ModelLoad(e.to_string()).into());
        }
    };
    
    let model_arc = Arc::new(model);
    
    let settings = crate::settings::load_settings(app.clone()).unwrap_or_default();

    let model_clone = model_arc.clone();
    let backend_clone = state.backend.clone();
    let workers_res = tokio::task::spawn_blocking(move || -> Result<_, LlmError> {
        let router = Router::new(&model_clone, &backend_clone).map_err(|e| LlmError::Routing(format!("{:?}", e)))?;
        let investigate = InvestigateWorker::new(&model_clone, &backend_clone, settings.preload_investigate).map_err(LlmError::Worker)?;
        let knowledge = KnowledgeWorker::new(&model_clone, &backend_clone, settings.preload_knowledge).map_err(LlmError::Worker)?;
        let analysis = AnalysisWorker::new(&model_clone, &backend_clone, settings.preload_analysis).map_err(LlmError::Worker)?;
        let rag = RagWorker::new(&model_clone, &backend_clone, settings.preload_rag).map_err(LlmError::Worker)?;
        let summarization = SummarizationWorker::new(&model_clone, &backend_clone).map_err(LlmError::Worker)?;
        Ok((router, investigate, knowledge, analysis, rag, summarization))
    }).await.map_err(|e| LlmError::SpawnBlocking(e.to_string()))??;

    let (router, investigate, knowledge, analysis, rag, summarization) = workers_res;
    
    let mut shared_lock = state.shared.lock().await;
    *shared_lock = Some(Arc::new(SharedModel {
        workers: Some(crate::llm::llm_manager::SharedWorkers {
            router: std::sync::Mutex::new(router),
            investigate: std::sync::Mutex::new(investigate),
            knowledge: std::sync::Mutex::new(knowledge),
            analysis: std::sync::Mutex::new(analysis),
            rag: std::sync::Mutex::new(rag),
            summarization: std::sync::Mutex::new(summarization),
        }),
        model: model_arc,
        backend: state.backend.clone(),
    }));
    
    {
        let mut status_lock = state.status.lock().await;
        *status_lock = ModelState::Loaded;
        let _ = app.emit("model-status-changed", &*status_lock);
    }
    
    Ok("Model loaded successfully".to_string())
}
