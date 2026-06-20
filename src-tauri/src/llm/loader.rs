use std::sync::Arc;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::LlamaModel;
use crate::llm::llm_manager::SharedModel;
use crate::llm::llm::{LlamaState, ModelState, LlmError};
use crate::llm::worker::{
    Router, InvestigateWorker, KnowledgeWorker, AnalysisWorker, RagWorker, SummarizationWorker, PloterWorker
};
use tauri::Emitter;
use crate::error::TauriError;

#[tauri::command]
pub async fn load_model(
    app: tauri::AppHandle,
    path: String,
    state: tauri::State<'_, LlamaState>,
) -> Result<String, TauriError> {
    if path.contains("..") {
        return Err(TauriError(crate::error::MikomaiError::Validation("Path traversal detected".to_string())));
    }
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

    let router_model = model_arc.clone();
    let router_backend = state.backend.clone();
    let router_task = tokio::task::spawn_blocking(move || {
        Router::new(&router_model, &router_backend).map_err(|e| LlmError::Routing(format!("{:?}", e)))
    });

    let investigate_model = model_arc.clone();
    let investigate_backend = state.backend.clone();
    let investigate_task = tokio::task::spawn_blocking(move || {
        InvestigateWorker::new(&investigate_model, &investigate_backend, settings.preload_investigate).map_err(LlmError::Worker)
    });

    let knowledge_model = model_arc.clone();
    let knowledge_backend = state.backend.clone();
    let knowledge_task = tokio::task::spawn_blocking(move || {
        KnowledgeWorker::new(&knowledge_model, &knowledge_backend, settings.preload_knowledge).map_err(LlmError::Worker)
    });

    let analysis_model = model_arc.clone();
    let analysis_backend = state.backend.clone();
    let analysis_task = tokio::task::spawn_blocking(move || {
        AnalysisWorker::new(&analysis_model, &analysis_backend, settings.preload_analysis).map_err(LlmError::Worker)
    });

    let rag_model = model_arc.clone();
    let rag_backend = state.backend.clone();
    let rag_task = tokio::task::spawn_blocking(move || {
        RagWorker::new(&rag_model, &rag_backend, settings.preload_rag).map_err(LlmError::Worker)
    });

    let summarization_model = model_arc.clone();
    let summarization_backend = state.backend.clone();
    let summarization_task = tokio::task::spawn_blocking(move || {
        SummarizationWorker::new(&summarization_model, &summarization_backend).map_err(LlmError::Worker)
    });

    let ploter_model = model_arc.clone();
    let ploter_backend = state.backend.clone();
    let ploter_task = tokio::task::spawn_blocking(move || {
        PloterWorker::new(&ploter_model, &ploter_backend, settings.preload_ploter).map_err(LlmError::Worker)
    });

    let (router_res, investigate_res, knowledge_res, analysis_res, rag_res, summarization_res, ploter_res) = tokio::try_join!(
        router_task,
        investigate_task,
        knowledge_task,
        analysis_task,
        rag_task,
        summarization_task,
        ploter_task
    ).map_err(|e| LlmError::SpawnBlocking(e.to_string()))?;

    let router = router_res?;
    let investigate = investigate_res?;
    let knowledge = knowledge_res?;
    let analysis = analysis_res?;
    let rag = rag_res?;
    let summarization = summarization_res?;
    let ploter = ploter_res?;
    
    let mut shared_lock = state.shared.lock().await;
    *shared_lock = Some(Arc::new(SharedModel {
        workers: Some(crate::llm::llm_manager::SharedWorkers {
            router: std::sync::Mutex::new(router),
            investigate: std::sync::Mutex::new(investigate),
            knowledge: std::sync::Mutex::new(knowledge),
            analysis: std::sync::Mutex::new(analysis),
            rag: std::sync::Mutex::new(rag),
            summarization: std::sync::Mutex::new(summarization),
            ploter: std::sync::Mutex::new(ploter),
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
