use hf_hub::api::tokio::Api;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::AddBos;
use llama_cpp_2::sampling::LlamaSampler;
use std::sync::Mutex;
use std::num::NonZeroU32;

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

#[tauri::command]
pub async fn ask_llm(prompt: String, state: tauri::State<'_, LlamaState>) -> Result<String, String> {
    let model_lock = state.model.lock().unwrap();
    let model = match &*model_lock {
        Some(m) => m,
        None => return Err("Model not loaded. Please configure and load a model first.".to_string()),
    };

    let formatted_prompt = format!("<|im_start|>user\n{}<|im_end|>\n<|im_start|>assistant\n", prompt);

    let mut ctx_params = LlamaContextParams::default();
    ctx_params = ctx_params.with_n_ctx(NonZeroU32::new(2048));

    let mut ctx = model.new_context(&state.backend, ctx_params).map_err(|e| format!("Failed to create context: {:?}", e))?;

    let tokens = model.str_to_token(&formatted_prompt, AddBos::Always).map_err(|e| format!("Tokenization error: {:?}", e))?;

    let mut batch = LlamaBatch::new(2048, 1);
    let last_index = tokens.len() - 1;
    for (i, token) in tokens.into_iter().enumerate() {
        let is_last = i == last_index;
        batch.add(token, i as i32, &[0], is_last).map_err(|e| format!("Failed to add to batch: {:?}", e))?;
    }

    ctx.decode(&mut batch).map_err(|e| format!("Decode error: {:?}", e))?;

    let mut result_string = String::new();
    let mut n_cur = batch.n_tokens();
    let mut sampler = LlamaSampler::greedy();

    let im_end_tokens = model.str_to_token("<|im_end|>", AddBos::Never).unwrap_or_default();
    let im_end_token = im_end_tokens.first().copied();

    let n_len = 500; // max length

    let mut bytes_accumulator = Vec::new();

    for _ in 0..n_len {
        let new_token_id = sampler.sample(&mut ctx, batch.n_tokens() - 1);

        if new_token_id == model.token_eos() || Some(new_token_id) == im_end_token {
            break;
        }

        let mut token_bytes = model.token_to_piece_bytes(new_token_id, 16, false, None).unwrap_or(vec![]);
        bytes_accumulator.append(&mut token_bytes);

        // Try converting accumulated bytes to string, if error means not fully formed utf8 character yet
        match String::from_utf8(bytes_accumulator.clone()) {
            Ok(s) => {
                result_string.push_str(&s);
                bytes_accumulator.clear();
            }
            Err(e) => {
                // Keep accumulating if we cannot parse it cleanly yet
                let utf8_error_index = e.utf8_error().valid_up_to();
                let valid_str = String::from_utf8_lossy(&bytes_accumulator[..utf8_error_index]).to_string();
                result_string.push_str(&valid_str);
                let remaining_bytes = bytes_accumulator[utf8_error_index..].to_vec();
                bytes_accumulator = remaining_bytes;
                if bytes_accumulator.len() > 8 {
                     // Failsafe in case we just got junk
                     result_string.push_str(&String::from_utf8_lossy(&bytes_accumulator));
                     bytes_accumulator.clear();
                }
            }
        }

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
