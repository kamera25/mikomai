use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::AddBos;
use llama_cpp_2::sampling::LlamaSampler;

use std::num::NonZeroU32;
use tauri::Emitter;
use tauri::Manager;
use crate::llm::llm_manager::SharedModel;
use std::sync::Arc;
use crate::llm::worker::{LlmWorker, Route};


#[derive(serde::Serialize)]
pub enum ModelState {
    NotLoaded,
    Loading,
    Loaded,
    Error(String),
}

#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("Mutex lock poisoned")]
    PoisonedLock,
    #[error("Llama backend initialization failed")]
    BackendInit,
    #[error("Model not loaded. Please configure and load a model first.")]
    ModelNotLoaded,
    #[error("Tokenization error: {0}")]
    Tokenization(String),
    #[error("Failed to create context: {0}")]
    ContextCreation(String),
    #[error("Failed to add token to batch: {0}")]
    BatchAdd(String),
    #[error("Decode error: {0}")]
    Decode(String),
    #[error("Routing failed: {0}")]
    Routing(String),
    #[error("Background worker failed: {0}")]
    Worker(String),
}

impl serde::Serialize for LlmError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub struct LlamaState {
    pub shared: tokio::sync::Mutex<Option<SharedModel>>,
    pub status: tokio::sync::Mutex<ModelState>,
    pub inference_lock: tokio::sync::Mutex<()>,
    pub backend: Arc<LlamaBackend>,
}

impl LlamaState {
    pub fn new() -> Result<Self, LlmError> {
        let backend = LlamaBackend::init().map_err(|_| LlmError::BackendInit)?;
        Ok(Self {
            shared: tokio::sync::Mutex::new(None),
            status: tokio::sync::Mutex::new(ModelState::NotLoaded),
            inference_lock: tokio::sync::Mutex::new(()),
            backend: Arc::new(backend),
        })
    }
}



#[tauri::command]
pub async fn get_model_status(state: tauri::State<'_, LlamaState>) -> Result<ModelState, String> {
    let status_lock = state.status.lock().await;
    let status = match &*status_lock {
        ModelState::NotLoaded => ModelState::NotLoaded,
        ModelState::Loading => ModelState::Loading,
        ModelState::Loaded => ModelState::Loaded,
        ModelState::Error(e) => ModelState::Error(e.clone()),
    };
    Ok(status)
}

pub const SYSTEM_PROMPT: &str = include_str!("system_prompt.txt");

fn prepare_prompt_tokens_with_limit(
    model: &LlamaModel,
    prompt: &str,
    n_ctx: usize,
    max_gen: usize,
) -> Result<Vec<llama_cpp_2::token::LlamaToken>, LlmError> {
    let mut tokens = model.str_to_token(prompt, AddBos::Always).map_err(|e| LlmError::Tokenization(format!("{:?}", e)))?;

    let max_tokens = n_ctx.saturating_sub(max_gen);
    if tokens.len() > max_tokens {
        let to_remove = tokens.len() - max_tokens;
        let start_keep = 500;

        if tokens.len() > start_keep + to_remove {
            tokens.drain(start_keep..(start_keep + to_remove));
        } else {
            tokens.truncate(max_tokens);
        }
    }
    Ok(tokens)
}


fn process_token_bytes(
    bytes_accumulator: &mut Vec<u8>,
    result_string: &mut String,
    window: Option<&tauri::Window>,
) {
    match std::str::from_utf8(bytes_accumulator) {
        Ok(s) => {
            if let Some(w) = window {
                let _ = w.emit("llm-chunk", s);
            }
            result_string.push_str(s);
            bytes_accumulator.clear();
        }
        Err(e) => {
            let utf8_error_index = e.valid_up_to();
            let valid_str = String::from_utf8_lossy(&bytes_accumulator[..utf8_error_index]).to_string();
            if let Some(w) = window {
                let _ = w.emit("llm-chunk", &valid_str);
            }
            result_string.push_str(&valid_str);
            bytes_accumulator.drain(..utf8_error_index);
            if bytes_accumulator.len() > 8 {
                 result_string.push_str(&String::from_utf8_lossy(bytes_accumulator));
                 bytes_accumulator.clear();
            }
        }
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AskInitialPayload {
    pub prompt: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzePayload {
    pub user_message: String,
    pub tool_label: String,
    pub output: String,
    pub is_rag: bool,
    pub history_block: Option<String>,
}

#[tauri::command]
pub async fn ask_llm_initial(
    window: tauri::Window,
    payload: AskInitialPayload,
    llama_state: tauri::State<'_, LlamaState>,
) -> Result<String, LlmError> {
    let AskInitialPayload { prompt } = payload;
    let original_query = if prompt.starts_with("【ユーザー入力】\n") {
        prompt.strip_prefix("【ユーザー入力】\n")
            .unwrap()
            .split("\n\n<memory>")
            .next()
            .unwrap_or(&prompt)
            .to_string()
    } else {
        prompt.split("\n\n<memory>").next().unwrap_or(&prompt).chars().take(300).collect::<String>()
    };

    if let Some((tool_name, params, message)) = crate::llm::shortcut::detect_shortcut_tool(&original_query) {
        let tool_call = serde_json::json!({
            "tool_name": tool_name,
            "params": params
        });
        let response_str = format!("{}\n\n```json\n{}\n```", message, serde_json::to_string_pretty(&tool_call).unwrap());
        let _ = window.emit("llm-chunk", &response_str);
        return Ok(response_str);
    }

    log::info!("Received original query: '{}'", original_query);

    if crate::llm::greeting::is_greeting(&original_query) {
        return Ok(crate::llm::greeting::stream_self_introduction(&window).await);
    }

    let app_handle = window.app_handle().clone();
    let window_clone = window.clone();
    let original_query_clone = original_query.clone();
    let prompt_clone = prompt.clone();

    let inference_result = tokio::task::spawn_blocking(move || -> Result<String, LlmError> {
        let state = app_handle.state::<LlamaState>();
        let _inference_guard = state.inference_lock.blocking_lock();
        let mut shared_lock = state.shared.blocking_lock();
        let shared = match &mut *shared_lock {
            Some(s) => s,
            None => return Err(LlmError::ModelNotLoaded),
        };

        let settings = crate::settings::load_settings(window_clone.app_handle().clone()).unwrap_or_default();

        log::info!("--- ROUTER INPUT QUERY ---\n{}\n-------------------------", original_query_clone);
        let route_result = shared.router.route(
            &shared.model,
            &original_query_clone,
            settings.repetition_penalty,
        ).map_err(|e| LlmError::Routing(format!("{:?}", e)))?;
        log::info!("--- ROUTER OUTPUT ---\n{:?}\n-------------------------", route_result);

        let active_route = route_result.routes[0];

        let worker: &mut dyn LlmWorker = match active_route {
            Route::Investigate => &mut shared.investigate,
            Route::Knowledge => &mut shared.knowledge,
            Route::Analysis => &mut shared.analysis,
            Route::None => return Ok("実行が完了しました。".to_string()),
        };

        let agent_name = worker.agent_name();
        let _ = window_clone.emit("agent-selected", agent_name);

        worker.ask(
            &shared.model,
            &shared.backend,
            Some(prompt_clone),
            None,
            None,
            None,
            None,
            route_result.subsequent_task.as_deref(),
            Some(&window_clone),
            settings.temperature,
            settings.repetition_penalty,
        ).map_err(LlmError::Worker)
    }).await.map_err(|e| LlmError::Worker(format!("Spawn blocking failed: {}", e)))??;

    log::info!("LLM Initial Prompt: {:?}\nResponse: {}", prompt, inference_result);
    Ok(inference_result)
}

#[tauri::command]
pub async fn analyze_tool_output(
    window: tauri::Window,
    payload: AnalyzePayload,
    llama_state: tauri::State<'_, LlamaState>,
) -> Result<String, LlmError> {
    let AnalyzePayload {
        user_message,
        tool_label,
        output,
        is_rag,
        history_block,
    } = payload;

    let app_handle = window.app_handle().clone();
    let window_clone = window.clone();
    let user_message_clone = user_message.clone();
    let tool_label_clone = tool_label.clone();
    let output_clone = output.clone();
    let history_block_clone = history_block.clone();

    let inference_result = tokio::task::spawn_blocking(move || -> Result<String, LlmError> {
        let state = app_handle.state::<LlamaState>();
        let _inference_guard = state.inference_lock.blocking_lock();
        let mut shared_lock = state.shared.blocking_lock();
        let shared = match &mut *shared_lock {
            Some(s) => s,
            None => return Err(LlmError::ModelNotLoaded),
        };

        let settings = crate::settings::load_settings(window_clone.app_handle().clone()).unwrap_or_default();

        if is_rag {
            let worker = &mut shared.rag;
            let agent_name = worker.agent_name();
            let _ = window_clone.emit("agent-selected", agent_name);

            worker.ask(
                &shared.model,
                &shared.backend,
                None,
                Some(user_message_clone),
                Some(tool_label_clone),
                Some(output_clone),
                history_block_clone,
                None,
                Some(&window_clone),
                settings.temperature,
                settings.repetition_penalty,
            ).map_err(LlmError::Worker)
        } else {
            log::info!("--- ROUTER INPUT QUERY ---\n{}\n-------------------------", user_message_clone);
            let route_result = shared.router.route(
                &shared.model,
                &user_message_clone,
                settings.repetition_penalty,
            ).map_err(|e| LlmError::Routing(format!("{:?}", e)))?;
            log::info!("--- ROUTER OUTPUT ---\n{:?}\n-------------------------", route_result);

            let active_route = if route_result.routes.len() > 1 {
                route_result.routes[1]
            } else {
                return Ok("実行が完了しました。".to_string());
            };

            let worker: &mut dyn LlmWorker = match active_route {
                Route::Investigate => &mut shared.investigate,
                Route::Knowledge => &mut shared.knowledge,
                Route::Analysis => &mut shared.analysis,
                Route::None => return Ok("実行が完了しました。".to_string()),
            };

            let agent_name = worker.agent_name();
            let _ = window_clone.emit("agent-selected", agent_name);

            worker.ask(
                &shared.model,
                &shared.backend,
                None,
                Some(user_message_clone),
                Some(tool_label_clone),
                Some(output_clone),
                history_block_clone,
                route_result.subsequent_task.as_deref(),
                Some(&window_clone),
                settings.temperature,
                settings.repetition_penalty,
            ).map_err(LlmError::Worker)
        }
    }).await.map_err(|e| LlmError::Worker(format!("Spawn blocking failed: {}", e)))??;

    log::info!("LLM Analysis User Message: {:?}\nResponse: {}", user_message, inference_result);
    Ok(inference_result)
}

pub async fn ask_llm_internal(
    prompt: &str,
    system_prompt: &str,
    app: &tauri::AppHandle,
    _state: &LlamaState,
) -> Result<String, LlmError> {
    let prompt_string = prompt.to_string();
    let system_prompt_string = system_prompt.to_string();
    let app_clone = app.clone();

    // Re-retrieve tauri state dynamically inside the thread using the cloned app handle
    let inference_result = tokio::task::spawn_blocking(move || -> Result<String, LlmError> {
        let state = app_clone.state::<LlamaState>();
        let _inference_guard = state.inference_lock.blocking_lock();
        let shared_lock = state.shared.blocking_lock();
        let shared = match &*shared_lock {
            Some(s) => s,
            None => return Err(LlmError::ModelNotLoaded),
        };

        let formatted_prompt = format!(
            "<|turn>system\n{}<turn|>\n<|turn>user\n{}<turn|>\n<|turn>model\n",
            system_prompt_string,
            prompt_string
        );

        log::info!("--- INTERNAL LLM PROMPT ---\n{}\n-------------------------", formatted_prompt);

        let settings = crate::settings::load_settings(app_clone.clone()).unwrap_or_default();
        let n_ctx = settings.n_ctx;
        let max_gen = settings.max_gen;

        let mut ctx_params = LlamaContextParams::default();
        ctx_params = ctx_params.with_n_ctx(NonZeroU32::new(n_ctx as u32));
        ctx_params = ctx_params.with_n_batch(n_ctx as u32);
        ctx_params = ctx_params.with_flash_attention_policy(1);

        let mut ctx = shared.model.new_context(&state.backend, ctx_params).map_err(|e| LlmError::ContextCreation(format!("{:?}", e)))?;

        let tokens = prepare_prompt_tokens_with_limit(&shared.model, &formatted_prompt, n_ctx, max_gen)?;

        let mut batch = LlamaBatch::new(n_ctx, 1);
        let last_index = tokens.len() - 1;
        for (i, token) in tokens.into_iter().enumerate() {
            let is_last = i == last_index;
            batch.add(token, i as i32, &[0], is_last).map_err(|e| LlmError::BatchAdd(format!("{:?}", e)))?;
        }

        ctx.decode(&mut batch).map_err(|e| LlmError::Decode(format!("{:?}", e)))?;

        let mut result_string = String::new();
        let mut n_cur = batch.n_tokens();
        let mut sampler = LlamaSampler::chain_simple([
            LlamaSampler::penalties(64, settings.repetition_penalty, 0.0, 0.0),
            LlamaSampler::greedy(),
        ]);

        let turn_end_tokens = shared.model.str_to_token("<turn|>", AddBos::Never).unwrap_or_default();
        let turn_end_token = turn_end_tokens.first().copied();

        let n_len = max_gen;

        let mut bytes_accumulator = Vec::new();

        for _ in 0..n_len {
            let new_token_id = sampler.sample(&mut ctx, batch.n_tokens() - 1);

            if new_token_id == shared.model.token_eos() || Some(new_token_id) == turn_end_token {
                break;
            }

            let mut token_bytes = shared.model.token_to_piece_bytes(new_token_id, 16, false, None).unwrap_or(vec![]);
            bytes_accumulator.append(&mut token_bytes);

            process_token_bytes(&mut bytes_accumulator, &mut result_string, None);

            batch.clear();
            batch.add(new_token_id, n_cur, &[0], true).map_err(|e| LlmError::BatchAdd(format!("{:?}", e)))?;
            n_cur += 1;

            ctx.decode(&mut batch).map_err(|e| LlmError::Decode(format!("{:?}", e)))?;
        }

        if !bytes_accumulator.is_empty() {
            result_string.push_str(&String::from_utf8_lossy(&bytes_accumulator));
        }

        log::info!("--- INTERNAL LLM RESPONSE ---\n{}\n-------------------------", result_string);
        Ok(result_string)
    }).await.map_err(|e| LlmError::Worker(format!("Spawn blocking failed: {}", e)))??;

    Ok(inference_result)
}

#[tauri::command]
pub async fn ask_llm_background(
    prompt: String, 
    app: tauri::AppHandle,
    state: tauri::State<'_, LlamaState>
) -> Result<String, LlmError> {
    let prompt_clone = prompt.clone();
    let app_clone = app.clone();
    let inference_result = tokio::task::spawn_blocking(move || -> Result<String, LlmError> {
        let state = app_clone.state::<LlamaState>();
        let _inference_guard = state.inference_lock.blocking_lock();
        let mut shared_lock = state.shared.blocking_lock();
        let shared = match &mut *shared_lock {
            Some(s) => s,
            None => return Err(LlmError::ModelNotLoaded),
        };

        let settings = crate::settings::load_settings(app_clone.clone()).unwrap_or_default();
        let worker = &mut shared.summarization;

        log::info!("LLM Background Prompt: {}", prompt_clone);
        let res = worker.ask(
            &shared.model,
            &shared.backend,
            Some(prompt_clone),
            None,
            None,
            None,
            None,
            None,
            None,
            settings.temperature,
            settings.repetition_penalty,
        ).map_err(LlmError::Worker);
        match &res {
            Ok(out) => log::info!("LLM Background Response: {}", out),
            Err(e) => log::error!("LLM Background Error: {}", e),
        }
        res
    }).await.map_err(|e| LlmError::Worker(format!("Spawn blocking failed: {}", e)))??;
    Ok(inference_result)
}

