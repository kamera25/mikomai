use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::AddBos;
use llama_cpp_2::sampling::LlamaSampler;
use std::sync::Mutex;
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

pub struct LlamaState {
    pub shared: Mutex<Option<SharedModel>>,
    pub status: Mutex<ModelState>,
    pub inference_lock: Mutex<()>,
    pub backend: Arc<LlamaBackend>,
}

impl LlamaState {
    pub fn new() -> Result<Self, String> {
        let backend = LlamaBackend::init().map_err(|e| e.to_string())?;
        Ok(Self {
            shared: Mutex::new(None),
            status: Mutex::new(ModelState::NotLoaded),
            inference_lock: Mutex::new(()),
            backend: Arc::new(backend),
        })
    }
}



#[tauri::command]
pub fn get_model_status(state: tauri::State<'_, LlamaState>) -> ModelState {
    let status_lock = match state.status.lock() {
        Ok(lock) => lock,
        Err(_) => return ModelState::Error("Mutex lock poisoned".to_string()),
    };
    match &*status_lock {
        ModelState::NotLoaded => ModelState::NotLoaded,
        ModelState::Loading => ModelState::Loading,
        ModelState::Loaded => ModelState::Loaded,
        ModelState::Error(e) => ModelState::Error(e.clone()),
    }
}

pub const SYSTEM_PROMPT: &str = include_str!("system_prompt.txt");

fn prepare_prompt_tokens_with_limit(
    model: &LlamaModel,
    prompt: &str,
    n_ctx: usize,
    max_gen: usize,
) -> Result<Vec<llama_cpp_2::token::LlamaToken>, String> {
    let mut tokens = model.str_to_token(prompt, AddBos::Always).map_err(|e| format!("Tokenization error: {:?}", e))?;

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

#[allow(dead_code)]
fn prepare_prompt_tokens(
    model: &LlamaModel,
    prompt: &str,
) -> Result<Vec<llama_cpp_2::token::LlamaToken>, String> {
    prepare_prompt_tokens_with_limit(model, prompt, 2048, 512)
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

#[tauri::command]
pub async fn ask_llm(
    window: tauri::Window,
    prompt: Option<String>,
    user_message: Option<String>,
    tool_label: Option<String>,
    output: Option<String>,
    is_rag: Option<bool>,
    history_block: Option<String>,
    llama_state: tauri::State<'_, LlamaState>,
) -> Result<String, String> {
    // 1. Extract the original query for routing (if not RAG)
    let is_rag_query = is_rag.unwrap_or(false) || prompt.as_ref().map(|p| p.starts_with("ユーザーの質問: \"")).unwrap_or(false);

    let original_query = if let Some(ref um) = user_message {
        um.clone()
    } else if let Some(ref p) = prompt {
        if p.starts_with("【ユーザー入力】\n") {
            p.strip_prefix("【ユーザー入力】\n")
                .unwrap()
                .split("\n\n<memory>")
                .next()
                .unwrap_or(p)
                .to_string()
        } else {
            p.split("\n\n<memory>").next().unwrap_or(p).chars().take(300).collect::<String>()
        }
    } else {
        "".to_string()
    };

    log::info!("Received original query: '{}'", original_query);

    if !is_rag_query && crate::llm::greeting::is_greeting(&original_query) {
        return Ok(crate::llm::greeting::stream_self_introduction(&window).await);
    }

    // Wrap the synchronous, non-Send inference logic in a separate scope
    let inference_result = {
        let run = || -> Result<String, String> {
            let _inference_guard = llama_state.inference_lock.lock().map_err(|_| "Mutex lock poisoned".to_string())?;
            let mut shared_lock = llama_state.shared.lock().map_err(|_| "Mutex lock poisoned".to_string())?;
            let shared = match &mut *shared_lock {
                Some(s) => s,
                None => return Err("Model not loaded. Please configure and load a model first.".to_string()),
            };

            let settings = crate::settings::load_settings(window.app_handle().clone()).unwrap_or_default();

            if is_rag_query {
                let worker = &mut shared.rag;
                let agent_name = worker.agent_name();
                let _ = window.emit("agent-selected", agent_name);

                worker.ask(
                    &shared.model,
                    &shared.backend,
                    prompt.clone(),
                    user_message.clone(),
                    tool_label.clone(),
                    output.clone(),
                    history_block.clone(),
                    None,
                    Some(&window),
                    settings.temperature,
                    settings.repetition_penalty,
                )
            } else {
                // Route classification
                log::info!("--- ROUTER INPUT QUERY ---\n{}\n-------------------------", original_query);
                let route_result = shared.router.route(
                    &shared.model,
                    &original_query,
                    settings.repetition_penalty,
                ).map_err(|e| format!("Routing failed: {:?}", e))?;
                log::info!("--- ROUTER OUTPUT ---\n{:?}\n-------------------------", route_result);

                // Select which route is active depending on whether this is the first call or the second call
                let is_subsequent = user_message.is_some();
                let active_route = if is_subsequent {
                    if route_result.routes.len() > 1 {
                        route_result.routes[1]
                    } else {
                        return Ok("実行が完了しました。".to_string());
                    }
                } else {
                    route_result.routes[0]
                };

                let worker: &mut dyn LlmWorker = match active_route {
                    Route::Investigate => &mut shared.investigate,
                    Route::Knowledge => &mut shared.knowledge,
                    Route::Analysis => &mut shared.analysis,
                    Route::None => return Ok("実行が完了しました。".to_string()),
                };

                let agent_name = worker.agent_name();
                let _ = window.emit("agent-selected", agent_name);

                worker.ask(
                    &shared.model,
                    &shared.backend,
                    prompt.clone(),
                    user_message.clone(),
                    tool_label.clone(),
                    output.clone(),
                    history_block.clone(),
                    route_result.subsequent_task.as_deref(),
                    Some(&window),
                    settings.temperature,
                    settings.repetition_penalty,
                )
            }
        };
        run()
    };
    match inference_result {
        Ok(response) => {
            log::info!("LLM Prompt: {:?}\nUser Message: {:?}\nResponse: {}", prompt, user_message, response);
            Ok(response)
        }
        Err(e) => {
            log::error!("LLM Prompt: {:?}\nUser Message: {:?}\nError: {}", prompt, user_message, e);
            Err(e)
        }
    }
}


pub async fn ask_llm_internal(
    prompt: &str,
    system_prompt: &str,
    app: &tauri::AppHandle,
    state: &LlamaState,
) -> Result<String, String> {
    let _inference_guard = state.inference_lock.lock().map_err(|_| "Mutex lock poisoned".to_string())?;
    let shared_lock = state.shared.lock().map_err(|_| "Mutex lock poisoned".to_string())?;
    let shared = match &*shared_lock {
        Some(s) => s,
        None => return Err("Model not loaded.".to_string()),
    };

    let formatted_prompt = format!(
        "<|turn>system\n{}<turn|>\n<|turn>user\n{}<turn|>\n<|turn>model\n",
        system_prompt,
        prompt
    );

    log::info!("--- INTERNAL LLM PROMPT ---\n{}\n-------------------------", formatted_prompt);

    let n_ctx = 4096;
    let max_gen = 2048;

    let mut ctx_params = LlamaContextParams::default();
    ctx_params = ctx_params.with_n_ctx(NonZeroU32::new(n_ctx as u32));
    ctx_params = ctx_params.with_n_batch(n_ctx as u32);

    let mut ctx = shared.model.new_context(&state.backend, ctx_params).map_err(|e| format!("Failed to create context: {:?}", e))?;

    let tokens = prepare_prompt_tokens_with_limit(&shared.model, &formatted_prompt, n_ctx, max_gen)?;

    let mut batch = LlamaBatch::new(n_ctx, 1);
    let last_index = tokens.len() - 1;
    for (i, token) in tokens.into_iter().enumerate() {
        let is_last = i == last_index;
        batch.add(token, i as i32, &[0], is_last).map_err(|e| format!("Failed to add to batch: {:?}", e))?;
    }

    ctx.decode(&mut batch).map_err(|e| format!("Decode error: {:?}", e))?;

    let mut result_string = String::new();
    let mut n_cur = batch.n_tokens();
    let settings = crate::settings::load_settings(app.clone()).unwrap_or_default();
    let mut sampler = LlamaSampler::chain_simple([
        LlamaSampler::penalties(64, settings.repetition_penalty, 0.0, 0.0),
        LlamaSampler::greedy(),
    ]);

    let turn_end_tokens = shared.model.str_to_token("<turn|>", AddBos::Never).unwrap_or_default();
    let turn_end_token = turn_end_tokens.first().copied();

    let n_len = max_gen; // max length

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
        batch.add(new_token_id, n_cur, &[0], true).map_err(|e| format!("Failed to add: {:?}", e))?;
        n_cur += 1;

        ctx.decode(&mut batch).map_err(|e| format!("Decode error: {:?}", e))?;
    }

    if !bytes_accumulator.is_empty() {
        result_string.push_str(&String::from_utf8_lossy(&bytes_accumulator));
    }

    log::info!("--- INTERNAL LLM RESPONSE ---\n{}\n-------------------------", result_string);
    Ok(result_string)
}

#[tauri::command]
pub async fn ask_llm_background(
    prompt: String, 
    app: tauri::AppHandle,
    state: tauri::State<'_, LlamaState>
) -> Result<String, String> {
    let run = || -> Result<String, String> {
        let _inference_guard = state.inference_lock.lock().map_err(|_| "Mutex lock poisoned".to_string())?;
        let mut shared_lock = state.shared.lock().map_err(|_| "Mutex lock poisoned".to_string())?;
        let shared = match &mut *shared_lock {
            Some(s) => s,
            None => return Err("Model not loaded. Please configure and load a model first.".to_string()),
        };

        let settings = crate::settings::load_settings(app.clone()).unwrap_or_default();
        let worker = &mut shared.summarization;

        log::info!("LLM Background Prompt: {}", prompt);
        let res = worker.ask(
            &shared.model,
            &shared.backend,
            Some(prompt),
            None,
            None,
            None,
            None,
            None,
            None,
            settings.temperature,
            settings.repetition_penalty,
        );
        match &res {
            Ok(out) => log::info!("LLM Background Response: {}", out),
            Err(e) => log::error!("LLM Background Error: {}", e),
        }
        res
    };
    run()
}
