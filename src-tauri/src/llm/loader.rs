use crate::error::TauriError;
use crate::llm::llm::{LlamaState, LlmError, ModelState};
use crate::llm::llm_manager::SharedModel;
use crate::llm::worker::{
    AnalysisWorker, BuilderWorker, KnowledgeWorker, LlmWorker, PlotterWorker, RagWorker, Router,
    SummarizationWorker,
};
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::LlamaModel;
use std::sync::Arc;
use tauri::Emitter;

#[tauri::command]
pub async fn load_model(
    app: tauri::AppHandle,
    path: String,
    state: tauri::State<'_, LlamaState>,
) -> Result<String, TauriError>
{
    if path.contains("..")
    {
        return Err(TauriError(crate::error::MikomaiError::Validation(
            "Path traversal detected".to_string(),
        )));
    }
    {
        let mut status_lock = state.status.lock().await;
        *status_lock = ModelState::Loading;
        let _ = app.emit("model-status-changed", &*status_lock);
    }

    let backend = state.backend.clone();
    let path_clone = path.clone();
    let model_res = tokio::task::spawn_blocking(move || {
        let n_gpu_layers = std::env::var("MIKOMAI_N_GPU_LAYERS")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(99);
        let mut model_params =
            std::pin::pin!(LlamaModelParams::default().with_n_gpu_layers(n_gpu_layers));
        model_params.as_mut().add_cpu_buft_override(c".*vision.*");
        LlamaModel::load_from_file(&*backend, &path_clone, &model_params)
    })
    .await
    .map_err(|e| LlmError::SpawnBlocking(e.to_string()))?;

    let model = match model_res
    {
        Ok(m) => m,
        Err(e) =>
        {
            let err_msg = format!("Failed to load model: {}", e);
            if let Ok(mut status_lock) = state.status.try_lock()
            {
                *status_lock = ModelState::Error(err_msg.clone());
                let _ = app.emit("model-status-changed", &*status_lock);
            }
            return Err(LlmError::ModelLoad(e.to_string()).into());
        }
    };

    let model_arc = Arc::new(model);

    let settings = crate::settings::load_settings(app.clone()).unwrap_or_default();

    // Phase 1: Initialize Router and SummarizationWorker, create fast shell instances for other workers
    let router_model = model_arc.clone();
    let router_backend = state.backend.clone();
    let router_task = tokio::task::spawn_blocking(move || {
        Router::new(&router_model, &router_backend)
            .map_err(|e| LlmError::Routing(format!("{:?}", e)))
    });

    let summarization_model = model_arc.clone();
    let summarization_backend = state.backend.clone();
    let summarization_task = tokio::task::spawn_blocking(move || {
        SummarizationWorker::new(&summarization_model, &summarization_backend, true)
            .map_err(LlmError::Worker)
    });

    // Create fast uninitialized shell instances (preload: false)
    let knowledge =
        KnowledgeWorker::new(&model_arc, &state.backend, false).map_err(LlmError::Worker)?;
    let analysis =
        AnalysisWorker::new(&model_arc, &state.backend, false).map_err(LlmError::Worker)?;
    let rag = RagWorker::new(&model_arc, &state.backend, false).map_err(LlmError::Worker)?;
    let plotter =
        PlotterWorker::new(&model_arc, &state.backend, false).map_err(LlmError::Worker)?;
    let builder =
        BuilderWorker::new(&model_arc, &state.backend, false).map_err(LlmError::Worker)?;

    let (router_res, summarization_res) = tokio::try_join!(router_task, summarization_task)
        .map_err(|e| LlmError::SpawnBlocking(e.to_string()))?;

    let router = router_res?;
    let summarization = summarization_res?;

    let shared_model = Arc::new(SharedModel {
        workers: Some(crate::llm::llm_manager::SharedWorkers {
            router: std::sync::Mutex::new(router),
            knowledge: std::sync::Mutex::new(knowledge),
            analysis: std::sync::Mutex::new(analysis),
            rag: std::sync::Mutex::new(rag),
            summarization: std::sync::Mutex::new(summarization),
            plotter: std::sync::Mutex::new(plotter),
            builder: std::sync::Mutex::new(builder),
        }),
        model: model_arc.clone(),
        backend: state.backend.clone(),
    });

    let mut shared_lock = state.shared.lock().await;
    *shared_lock = Some(shared_model.clone());

    // Unlock UI immediately
    {
        let mut status_lock = state.status.lock().await;
        *status_lock = ModelState::Loaded;
        let _ = app.emit("model-status-changed", &*status_lock);
    }

    // Phase 2: Asynchronously warm up preloaded workers in the background
    let model_bg = model_arc.clone();
    let backend_bg = state.backend.clone();
    let shared_bg = shared_model.clone();
    tokio::task::spawn_blocking(move || {
        if settings.preload_knowledge
        {
            if let Ok(mut w) = shared_bg.knowledge.lock()
            {
                let _ = w.ensure_initialized(&model_bg, &backend_bg);
            }
        }
        if settings.preload_analysis
        {
            if let Ok(mut w) = shared_bg.analysis.lock()
            {
                let _ = w.ensure_initialized(&model_bg, &backend_bg);
            }
        }
        if settings.preload_rag
        {
            if let Ok(mut w) = shared_bg.rag.lock()
            {
                let _ = w.ensure_initialized(&model_bg, &backend_bg);
            }
        }
        if settings.preload_plotter
        {
            if let Ok(mut w) = shared_bg.plotter.lock()
            {
                let _ = w.ensure_initialized(&model_bg, &backend_bg);
            }
        }
        if settings.preload_builder
        {
            if let Ok(mut w) = shared_bg.builder.lock()
            {
                let _ = w.ensure_initialized(&model_bg, &backend_bg);
            }
        }
    });

    Ok("Model loaded successfully".to_string())
}
