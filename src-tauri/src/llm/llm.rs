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

fn is_greeting(query: &str) -> bool {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return false;
    }
    
    let greetings = [
        "こんにちは",
        "こんにちわ",
        "はじめまして",
        "おはよう",
        "おはようございます",
        "こんばんは",
        "お疲れ様です",
        "おつかれさま",
        "お疲れ様",
        "ハロー",
        "はろー",
        "自己紹介",
        "じこしょうかい",
        "hello",
        "hi",
        "hey",
        "who are you",
        "あなたは誰",
        "あなたはだれ",
        "お名前は",
        "なまえは",
        "名前は",
        "おなまえは",
    ];

    for g in greetings {
        if q.contains(g) && q.chars().count() <= 20 {
            return true;
        }
    }
    
    let self_intro_keywords = [
        "自己紹介して",
        "自己紹介してください",
        "自己紹介をおねがいします",
        "自己紹介をお願いします",
        "何ができますか",
        "なにができますか",
    ];
    for kw in self_intro_keywords {
        if q.contains(kw) {
            return true;
        }
    }

    false
}

async fn stream_self_introduction(window: &tauri::Window) -> String {
    let intro = "はじめまして！私は「MIKOMAI (Managed Infrastructure Knowledge Operator ML Agent Interface)」です。\n\
                 ネットワークインフラの診断、運用、トラブルシューティングを最高精度で支援するプロフェッショナルAIアシスタントです。\n\n\
                 以下のような操作や調査をお手伝いできます：\n\
                 - **ネットワーク機器のステータス確認** (例: `show ip int brief` など)\n\
                 - **疎通確認・調査** (PingやTracerouteの実行)\n\
                 - **ホスト一覧やARPテーブルの取得**\n\
                 - **ネットワーク技術データベース (NW-DB) の検索と解説** (Cisco機器の設定手順など)\n\
                 - **ログの分析とトラブルシューティング**\n\n\
                 何かお手伝いできることはありますか？お気軽に話しかけてください！";

    let _ = window.emit("agent-selected", "MIKOMAI (アシスタント)");

    let chars: Vec<char> = intro.chars().collect();
    let chunk_size = 5;
    for chunk in chars.chunks(chunk_size) {
        let chunk_str: String = chunk.iter().collect();
        let _ = window.emit("llm-chunk", &chunk_str);
        tokio::time::sleep(std::time::Duration::from_millis(15)).await;
    }

    intro.to_string()
}

#[tauri::command]
pub async fn ask_llm(
    window: tauri::Window,
    prompt: String,
    llama_state: tauri::State<'_, LlamaState>,
) -> Result<String, String> {
    println!("Received prompt: {}", prompt);

    // Extract clean query for the router to avoid passing huge histories/tool outputs and hitting context limit
    let router_query = if prompt.starts_with("【ユーザー入力】\n") {
        prompt.strip_prefix("【ユーザー入力】\n")
            .unwrap()
            .split("\n\n<memory>")
            .next()
            .unwrap_or(&prompt)
            .to_string()
    } else if prompt.starts_with("ユーザーの入力: \"") {
        prompt.strip_prefix("ユーザーの入力: \"")
            .unwrap()
            .split("\"\nに対する")
            .next()
            .unwrap_or(&prompt)
            .to_string()
    } else {
        prompt.chars().take(300).collect::<String>()
    };

    println!("Router clean query: '{}'", router_query);

    if is_greeting(&router_query) {
        return Ok(stream_self_introduction(&window).await);
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

            // 1. Initialize router and classify the request
            let mut router_ctx = crate::llm::llm_manager::AgentContext::new(
                shared,
                crate::llm::llm_manager::ROUTER_PROMPT,
                0
            ).map_err(|e| format!("Failed to create router context: {:?}", e))?;

            let route = crate::llm::llm_manager::run_inference(
                &mut router_ctx,
                &shared.model,
                &router_query,
                None, // No chunk emission for router
                0.0,  // Greedy to ensure stable routing
                settings.repetition_penalty,
            ).map_err(|e| format!("Routing failed: {:?}", e))?;

            let route_trimmed = route.trim().to_uppercase();
            println!("Router raw decision: '{}'", route_trimmed);

            // 2. Select appropriate worker context
            let (selected_prompt, agent_name) = if route_trimmed.contains("INVESTIGATE") {
                (crate::llm::llm_manager::INVESTIGATE_WORKER_PROMPT, "Investigator (調査員)")
            } else if route_trimmed.contains("KNOWLEDGE") {
                (crate::llm::llm_manager::KNOWLEDGE_WORKER_PROMPT, "Knowledge Expert (知識専門家)")
            } else if route_trimmed.contains("ANALYSIS") {
                (crate::llm::llm_manager::ANALYSIS_WORKER_PROMPT, "Analyst (分析官)")
            } else {
                println!("Warning: Router output invalid classification, falling back to Investigator");
                (crate::llm::llm_manager::INVESTIGATE_WORKER_PROMPT, "Investigator (調査員)")
            };

            // Emit the event to the frontend
            let _ = window.emit("agent-selected", agent_name);

            // Combine base system rules with worker instructions to maintain features
            let full_worker_system_prompt = format!(
                "{}\n\n=== Current Role ===\nあなたは現在「{}」として動作しています。以下の役割指示に特化してください:\n{}",
                SYSTEM_PROMPT,
                agent_name,
                selected_prompt
            );

            // 3. Initialize and run selected worker
            let mut worker_ctx = crate::llm::llm_manager::AgentContext::new(
                shared,
                &full_worker_system_prompt,
                1
            ).map_err(|e| format!("Failed to create worker context: {:?}", e))?;

            let response = crate::llm::llm_manager::run_inference(
                &mut worker_ctx,
                &shared.model,
                &prompt,
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
        Err(e) => {
            if e.contains("n_tokens == 0") || is_greeting(&router_query) {
                return Ok(stream_self_introduction(&window).await);
            }
            Err(e)
        }
    }
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
