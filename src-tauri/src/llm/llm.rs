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

fn prepare_prompt_tokens(
    model: &LlamaModel,
    prompt: &str,
) -> Result<Vec<llama_cpp_2::token::LlamaToken>, String> {
    let mut tokens = model.str_to_token(prompt, AddBos::Always).map_err(|e| format!("Tokenization error: {:?}", e))?;

    let max_tokens = 2048 - 512;
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

#[tauri::command]
pub async fn ask_llm(
    window: tauri::Window,
    prompt: String,
    llama_state: tauri::State<'_, LlamaState>,
) -> Result<String, String> {
    println!("Received prompt: {}", prompt);

    let _inference_guard = llama_state.inference_lock.lock().map_err(|_| "Mutex lock poisoned".to_string())?;
    let shared_lock = llama_state.shared.lock().map_err(|_| "Mutex lock poisoned".to_string())?;
    let shared = match &*shared_lock {
        Some(s) => s,
        None => return Err("Model not loaded. Please configure and load a model first.".to_string()),
    };

    let formatted_prompt = format!(
        "<|turn>system\n{}<turn|>\n<|turn>user\n{}<turn|>\n<|turn>model\n",
        SYSTEM_PROMPT,
        prompt
    );

    let mut ctx_params = LlamaContextParams::default();
    ctx_params = ctx_params.with_n_ctx(NonZeroU32::new(2048));

    let mut ctx = shared.model.new_context(&llama_state.backend, ctx_params).map_err(|e| format!("Failed to create context: {:?}", e))?;

    let tokens = prepare_prompt_tokens(&shared.model, &formatted_prompt)?;
    println!("Total tokens in prompt: {}", tokens.len());

    let mut batch = LlamaBatch::new(2048, 1);
    let last_index = tokens.len() - 1;
    for (i, token) in tokens.into_iter().enumerate() {
        let is_last = i == last_index;
        batch.add(token, i as i32, &[0], is_last).map_err(|e| format!("Failed to add to batch: {:?}", e))?;
    }

    ctx.decode(&mut batch).map_err(|e| format!("Decode error: {:?}", e))?;

    let mut result_string = String::new();
    let mut n_cur = batch.n_tokens();
    let settings = crate::settings::load_settings(window.app_handle().clone()).unwrap_or_default();
    println!("LLM Temperature setting: {}", settings.temperature);

    let sampler = if settings.temperature <= 0.0 {
        LlamaSampler::chain_simple([
            LlamaSampler::penalties(64, settings.repetition_penalty, 0.0, 0.0),
            LlamaSampler::greedy(),
        ])
    } else {
        LlamaSampler::chain_simple([
            LlamaSampler::penalties(64, settings.repetition_penalty, 0.0, 0.0),
            LlamaSampler::temp(settings.temperature),
            LlamaSampler::dist(42),
        ])
    };
    let mut sampler = sampler;

    let turn_end_tokens = shared.model.str_to_token("<turn|>", AddBos::Never).unwrap_or_default();
    let turn_end_token = turn_end_tokens.first().copied();

    let n_len = 500; // max length

    let mut bytes_accumulator = Vec::new();

    for _ in 0..n_len {
        let new_token_id = sampler.sample(&mut ctx, batch.n_tokens() - 1);

        if new_token_id == shared.model.token_eos() || Some(new_token_id) == turn_end_token {
            break;
        }

        let mut token_bytes = shared.model.token_to_piece_bytes(new_token_id, 16, false, None).unwrap_or(vec![]);
        bytes_accumulator.append(&mut token_bytes);

        process_token_bytes(&mut bytes_accumulator, &mut result_string, Some(&window));

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
    let _inference_guard = state.inference_lock.lock().map_err(|_| "Mutex lock poisoned".to_string())?;
    let shared_lock = state.shared.lock().map_err(|_| "Mutex lock poisoned".to_string())?;
    let shared = match &*shared_lock {
        Some(s) => s,
        None => return Err("Model not loaded.".to_string()),
    };

    let formatted_prompt = format!(
        "<|turn>system\n{}<turn|>\n<|turn>user\n{}<turn|>\n<|turn>model\n",
        SUMMARIZATION_SYSTEM_PROMPT,
        prompt
    );

    let mut ctx_params = LlamaContextParams::default();
    ctx_params = ctx_params.with_n_ctx(NonZeroU32::new(2048));

    let mut ctx = shared.model.new_context(&state.backend, ctx_params).map_err(|e| format!("Failed to create context: {:?}", e))?;

    let tokens = prepare_prompt_tokens(&shared.model, &formatted_prompt)?;

    let mut batch = LlamaBatch::new(2048, 1);
    let last_index = tokens.len() - 1;
    for (i, token) in tokens.into_iter().enumerate() {
        let is_last = i == last_index;
        batch.add(token, i as i32, &[0], is_last).map_err(|e| format!("Failed to add to batch: {:?}", e))?;
    }

    ctx.decode(&mut batch).map_err(|e| format!("Decode error: {:?}", e))?;

    let mut result_string = String::new();
    let mut n_cur = batch.n_tokens();
    let settings = crate::settings::load_settings(app).unwrap_or_default();
    let mut sampler = LlamaSampler::chain_simple([
        LlamaSampler::penalties(64, settings.repetition_penalty, 0.0, 0.0),
        LlamaSampler::greedy(),
    ]);

    let turn_end_tokens = shared.model.str_to_token("<turn|>", AddBos::Never).unwrap_or_default();
    let turn_end_token = turn_end_tokens.first().copied();

    let n_len = 500; // max length

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
