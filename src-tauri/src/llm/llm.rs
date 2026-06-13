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
use crate::llm::worker::{LlmWorker, RagWorker, KnowledgeWorker, AnalysisWorker, InvestigateWorker, Route};


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

const SYSTEM_PROMPT: &str = include_str!("system_prompt.txt");

const SUMMARIZATION_SYSTEM_PROMPT: &str = include_str!("summarization_prompt.txt");

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

    println!("Received original query: '{}'", original_query);

    if !is_rag_query && crate::llm::greeting::is_greeting(&original_query) {
        return Ok(crate::llm::greeting::stream_self_introduction(&window).await);
    }

    // Wrap the synchronous, non-Send inference logic in a separate scope
    let inference_result = {
        let run = || -> Result<String, String> {
            let _inference_guard = llama_state.inference_lock.lock().map_err(|_| "Mutex lock poisoned".to_string())?;
            let shared_lock = llama_state.shared.lock().map_err(|_| "Mutex lock poisoned".to_string())?;
            let shared = match &*shared_lock {
                Some(s) => s,
                None => return Err("Model not loaded. Please configure and load a model first.".to_string()),
            };

            let settings = crate::settings::load_settings(window.app_handle().clone()).unwrap_or_default();

            let mut subsequent_task: Option<String> = None;
            let worker: Box<dyn LlmWorker> = if is_rag_query {
                Box::new(RagWorker)
            } else {
                // Route classification
                let mut router_ctx = crate::llm::llm_manager::AgentContext::new(
                    shared,
                    crate::llm::llm_manager::ROUTER_PROMPT,
                    0,
                    2048
                ).map_err(|e| format!("Failed to create router context: {:?}", e))?;
                println!("--- ROUTER INPUT QUERY ---\n{}\n-------------------------", original_query);

                let route_output = crate::llm::llm_manager::run_inference(
                    &mut router_ctx,
                    &shared.model,
                    &original_query,
                    None,
                    0.0,
                    settings.repetition_penalty,
                ).map_err(|e| format!("Routing failed: {:?}", e))?;
                println!("--- ROUTER OUTPUT ---\n{}\n-------------------------", route_output);

                let mut first_route = Route::Investigate;
                let mut subsequent_route = Route::None;

                for line in route_output.lines() {
                    let trimmed = line.trim();
                    let trimmed_upper = trimmed.to_uppercase();
                    if trimmed_upper.starts_with("FIRST_ROUTE:") {
                        let val = trimmed["FIRST_ROUTE:".len()..].trim();
                        first_route = Route::from_str(val);
                    } else if trimmed_upper.starts_with("SUBSEQUENT_ROUTE:") {
                        let val = trimmed["SUBSEQUENT_ROUTE:".len()..].trim();
                        subsequent_route = Route::from_str(val);
                    } else if trimmed_upper.starts_with("TASK:") {
                        let val = trimmed["TASK:".len()..].trim();
                        let val_upper = val.to_uppercase();
                        if val_upper != "NONE" && !val.is_empty() {
                            subsequent_task = Some(val.to_string());
                        }
                    }
                }

                // Fallback for single-word outputs or older output style
                if !route_output.to_uppercase().contains("FIRST_ROUTE:") {
                    first_route = Route::from_str(&route_output);
                }

                if subsequent_route == Route::None && subsequent_task.is_some() {
                    subsequent_route = Route::Analysis;
                }

                println!("Parsed FIRST_ROUTE: {:?}, SUBSEQUENT_ROUTE: {:?}, TASK: {:?}", first_route, subsequent_route, subsequent_task);

                // Select which route is active depending on whether this is the first call or the second call
                let active_route = if user_message.is_some() {
                    if subsequent_route == Route::None {
                        return Ok("実行が完了しました。".to_string());
                    }
                    subsequent_route
                } else {
                    first_route
                };

                match active_route {
                    Route::Knowledge => Box::new(KnowledgeWorker) as Box<dyn LlmWorker>,
                    Route::Analysis => Box::new(AnalysisWorker) as Box<dyn LlmWorker>,
                    _ => Box::new(InvestigateWorker) as Box<dyn LlmWorker>,
                }
            };

            let agent_name = worker.agent_name();
            let selected_prompt = worker.system_prompt(subsequent_task.as_deref());
            let worker_prompt = worker.build_prompt(
                prompt.clone(),
                user_message.clone(),
                tool_label.clone(),
                output.clone(),
                history_block.clone(),
            );

            // Emit the event to the frontend
            let _ = window.emit("agent-selected", agent_name);

            // Combine base system rules with worker instructions to maintain features
            let full_worker_system_prompt = format!(
                "{}\n\n=== Current Role ===\nあなたは現在「{}」として動作しています。以下の役割指示に特化してください:\n{}",
                SYSTEM_PROMPT,
                agent_name,
                selected_prompt
            );

            println!("--- WORKER SYSTEM PROMPT ---\n{}\n-------------------------", full_worker_system_prompt);
            println!("--- WORKER INPUT PROMPT ---\n{}\n-------------------------", worker_prompt);

            // 3. Initialize and run selected worker
            let mut worker_ctx = crate::llm::llm_manager::AgentContext::new(
                shared,
                &full_worker_system_prompt,
                1,
                worker.max_new_tokens()
            ).map_err(|e| format!("Failed to create worker context: {:?}", e))?;

            let response = crate::llm::llm_manager::run_inference(
                &mut worker_ctx,
                &shared.model,
                &worker_prompt,
                Some(&window),
                settings.temperature,
                settings.repetition_penalty,
            ).map_err(|e| format!("Worker inference failed: {:?}", e))?;

            Ok(response)
        };
        run()
    };
    match inference_result {
        Ok(response) => Ok(response),
        Err(e) => Err(e),
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

    println!("--- INTERNAL LLM PROMPT ---\n{}\n-------------------------", formatted_prompt);

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

    Ok(result_string)
}

#[tauri::command]
pub async fn ask_llm_background(
    prompt: String, 
    app: tauri::AppHandle,
    state: tauri::State<'_, LlamaState>
) -> Result<String, String> {
    ask_llm_internal(&prompt, SUMMARIZATION_SYSTEM_PROMPT, &app, &state).await
}
