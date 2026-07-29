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
use crate::error::TauriError;
use std::sync::atomic::{AtomicBool, Ordering};

static CANCEL_LLM: AtomicBool = AtomicBool::new(false);

pub fn cancel() {
    CANCEL_LLM.store(true, Ordering::SeqCst);
}

pub fn reset_cancel() {
    CANCEL_LLM.store(false, Ordering::SeqCst);
}

pub fn is_cancelled() -> bool {
    CANCEL_LLM.load(Ordering::SeqCst)
}

#[tauri::command]
pub fn stop_llm() {
    log::info!("stop_llm command invoked by user");
    cancel();
}



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
    #[error("Failed to get home directory")]
    HomeDirResolution,
    #[error("File I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Download failed with status: {0}")]
    DownloadStatus(String),
    #[error("Opener error: {0}")]
    Opener(String),
    #[error("Failed to load model file: {0}")]
    ModelLoad(String),
    #[error("Spawn blocking failed: {0}")]
    SpawnBlocking(String),
    #[error("Tauri error: {0}")]
    Tauri(#[from] tauri::Error),
}

impl serde::Serialize for LlmError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub enum InferenceRequest {
    Initial {
        window: tauri::Window,
        prompt: String,
        original_query: String,
        respond_to: tokio::sync::oneshot::Sender<Result<(String, Route), LlmError>>,
    },
    Analyze {
        window: tauri::Window,
        user_message: String,
        tool_label: String,
        output: String,
        is_rag: bool,
        is_builder: bool,
        history_block: Option<String>,
        subsequent_task: Option<String>,
        respond_to: tokio::sync::oneshot::Sender<Result<String, LlmError>>,
    },
    Background {
        prompt: String,
        app: tauri::AppHandle,
        respond_to: tokio::sync::oneshot::Sender<Result<String, LlmError>>,
    },
    Internal {
        prompt: String,
        system_prompt: String,
        app: tauri::AppHandle,
        respond_to: tokio::sync::oneshot::Sender<Result<String, LlmError>>,
    },
}

pub struct LlamaState {
    pub shared: Arc<tokio::sync::Mutex<Option<Arc<SharedModel>>>>,
    pub status: tokio::sync::Mutex<ModelState>,
    pub backend: Arc<LlamaBackend>,
    pub inference_tx: tokio::sync::mpsc::Sender<InferenceRequest>,
}

fn get_worker_for_route(shared: &SharedModel, route: Route) -> Option<&std::sync::Mutex<dyn LlmWorker>> {
    match route {
        Route::Investigate => Some(&shared.investigate),
        Route::Knowledge => Some(&shared.knowledge),
        Route::Analysis => Some(&shared.analysis),
        Route::Plotter => Some(&shared.plotter),
        Route::Builder => Some(&shared.builder),
        Route::None => None,
    }
}

impl LlamaState {
    pub fn new() -> Result<Self, LlmError> {
        let backend = LlamaBackend::init().map_err(|_| LlmError::BackendInit)?;
        let backend_arc = Arc::new(backend);
        let shared = Arc::new(tokio::sync::Mutex::new(None));
        let status = tokio::sync::Mutex::new(ModelState::NotLoaded);
        
        let (tx, mut rx) = tokio::sync::mpsc::channel::<InferenceRequest>(100);
        
        let shared_clone = shared.clone();
        std::thread::spawn(move || {
            while let Some(req) = rx.blocking_recv() {
                let shared_model_opt = {
                    let lock = shared_clone.blocking_lock();
                    lock.clone()
                };
                let shared_model: Arc<SharedModel> = match shared_model_opt {
                    Some(s) => s,
                    None => {
                        match req {
                            InferenceRequest::Initial { respond_to, .. } => { let _ = respond_to.send(Err(LlmError::ModelNotLoaded)); },
                            InferenceRequest::Analyze { respond_to, .. } => { let _ = respond_to.send(Err(LlmError::ModelNotLoaded)); },
                            InferenceRequest::Background { respond_to, .. } => { let _ = respond_to.send(Err(LlmError::ModelNotLoaded)); },
                            InferenceRequest::Internal { respond_to, .. } => { let _ = respond_to.send(Err(LlmError::ModelNotLoaded)); },
                        }
                        continue;
                    }
                };

                match req {
                    InferenceRequest::Initial { window, prompt, original_query, respond_to } => {
                        let res = (|| -> Result<(String, Route), LlmError> {
                            let settings = crate::settings::load_settings(window.app_handle().clone()).unwrap_or_default();
                            let model = shared_model.model.clone();
                            let backend = shared_model.backend.clone();

                            log::info!("--- ROUTER INPUT QUERY ---\n{}\n-------------------------", original_query);
                            let route_result = {
                                let mut router_lock = shared_model.router.lock().unwrap();
                                router_lock.route(
                                    &model,
                                    &original_query,
                                    settings.repetition_penalty,
                                ).map_err(|e| LlmError::Routing(format!("{:?}", e)))?
                            };
                            log::info!("--- ROUTER OUTPUT ---\n{:?}\n-------------------------", route_result);

                            if route_result.confidence < 0.5 {
                                let ask_msg = "ご質問の意図を確認させてください。\n\n```json\n{\n  \"tool_name\": \"ask_user_choice\",\n  \"params\": {\n    \"title\": \"ご質問の意図の確認\",\n    \"message\": \"ご質問の意図を確認させてください。以下のどれに該当しますか？\",\n    \"options\": [\n      \"1. ネットワーク機器の調査 (INVESTIGATE)\",\n      \"2. 技術知識の解説 (KNOWLEDGE)\",\n      \"3. Config作成 (BUILDER)\"\n    ]\n  }\n}\n```";
                                let _ = window.emit("chat-event", crate::mcp::protocol::ChatEvent::AgentSelected("MIKOMAI (アシスタント)".to_string()));
                                let _ = window.emit("chat-event", crate::mcp::protocol::ChatEvent::LlmChunk(ask_msg.to_string()));
                                return Ok((ask_msg.to_string(), Route::None));
                            }

                            let active_route = route_result.routes[0];
                            let final_prompt = if active_route == Route::Builder {
                                replace_interface_abbreviations(&prompt)
                            } else {
                                prompt
                            };
                            let worker_res = if let Some(worker_mutex) = get_worker_for_route(&shared_model, active_route) {
                                let mut worker = worker_mutex.lock().unwrap();
                                let agent_name = worker.agent_name();
                                let _ = window.emit("chat-event", crate::mcp::protocol::ChatEvent::AgentSelected(agent_name.to_string()));
                                worker.ask(
                                    &model,
                                    &backend,
                                    Some(final_prompt),
                                    None,
                                    None,
                                    None,
                                    None,
                                    route_result.subsequent_task.as_deref(),
                                    Some(&window),
                                    settings.temperature,
                                    settings.repetition_penalty,
                                ).map_err(LlmError::Worker)
                            } else {
                                Ok("実行が完了しました。".to_string())
                            };
                            worker_res.map(|s| (s, active_route))
                        })();
                        let _ = respond_to.send(res);
                    }
                    InferenceRequest::Analyze { window, user_message, tool_label, output, is_rag, is_builder, history_block, subsequent_task, respond_to } => {
                        let res = (|| -> Result<String, LlmError> {
                            let settings = crate::settings::load_settings(window.app_handle().clone()).unwrap_or_default();
                            let model = shared_model.model.clone();
                            let backend = shared_model.backend.clone();

                            if is_rag {
                                if is_builder {
                                    let mut worker = shared_model.builder.lock().unwrap();
                                    let agent_name = worker.agent_name();
                                    let _ = window.emit("chat-event", crate::mcp::protocol::ChatEvent::AgentSelected(agent_name.to_string()));

                                    worker.ask(
                                        &model,
                                        &backend,
                                        None,
                                        Some(user_message),
                                        Some(tool_label),
                                        Some(output),
                                        history_block,
                                        None,
                                        Some(&window),
                                        settings.temperature,
                                        settings.repetition_penalty,
                                    ).map_err(LlmError::Worker)
                                } else {
                                    let mut worker = shared_model.rag.lock().unwrap();
                                    let agent_name = worker.agent_name();
                                    let _ = window.emit("chat-event", crate::mcp::protocol::ChatEvent::AgentSelected(agent_name.to_string()));

                                    worker.ask(
                                        &model,
                                        &backend,
                                        None,
                                        Some(user_message),
                                        Some(tool_label),
                                        Some(output),
                                        history_block,
                                        None,
                                        Some(&window),
                                        settings.temperature,
                                        settings.repetition_penalty,
                                    ).map_err(LlmError::Worker)
                                }
                            } else {
                                let is_ask_user_choice = tool_label.contains("ask_user_choice");
                                let is_ask_interface_choice = tool_label.contains("ask_interface_choice");
                                let is_ask_ipaddress_choice = tool_label.contains("ask_ipaddress_choice");
                                let is_any_choice = is_ask_user_choice || is_ask_interface_choice || is_ask_ipaddress_choice;

                                if is_any_choice && (output.trim() == "cancelled" || output.lines().any(|l| l.trim() == "cancelled") || output.trim().ends_with("cancelled")) {
                                    log::info!("Choice prompt cancelled by user (tool_label: {}), stopping subsequent sequence.", tool_label);
                                    let cancel_msg = "応答が停止されました。";
                                    let _ = window.emit("chat-event", crate::mcp::protocol::ChatEvent::LlmChunk(cancel_msg.to_string()));
                                    return Ok(cancel_msg.to_string());
                                }

                                let (active_route, route_subsequent_task) = if is_any_choice {
                                    (Route::Builder, None)
                                } else {
                                    log::info!("--- ROUTER INPUT QUERY ---\n{}\n-------------------------", user_message);
                                    let route_result = {
                                        let mut router_lock = shared_model.router.lock().unwrap();
                                        router_lock.route(
                                            &model,
                                            &user_message,
                                            settings.repetition_penalty,
                                        ).map_err(|e| LlmError::Routing(format!("{:?}", e)))?
                                    };
                                     log::info!("--- ROUTER OUTPUT ---\n{:?}\n-------------------------", route_result);
 
                                     if route_result.confidence < 0.5 {
                                         let ask_msg = "ご質問の意図を確認させてください。\n\n```json\n{\n  \"tool_name\": \"ask_user_choice\",\n  \"params\": {\n    \"title\": \"ご質問の意図の確認\",\n    \"message\": \"ご質問の意図を確認させてください。以下のどれに該当しますか？\",\n    \"options\": [\n      \"1. ネットワーク機器の調査 (INVESTIGATE)\",\n      \"2. 技術知識の解説 (KNOWLEDGE)\",\n      \"3. Config作成 (BUILDER)\"\n    ]\n  }\n}\n```";
                                         let _ = window.emit("chat-event", crate::mcp::protocol::ChatEvent::AgentSelected("MIKOMAI (アシスタント)".to_string()));
                                         let _ = window.emit("chat-event", crate::mcp::protocol::ChatEvent::LlmChunk(ask_msg.to_string()));
                                         return Ok(ask_msg.to_string());
                                     }
 
                                     let route = if route_result.routes.len() > 1 {
                                         route_result.routes[1]
                                     } else {
                                         return Ok("実行が完了しました。".to_string());
                                     };
                                    (route, route_result.subsequent_task)
                                };

                                let custom_subsequent_task = if is_ask_user_choice {
                                    Some(format!("ユーザーが「{}」を選択しました。この回答要件を含めてCisco Configを設定・生成してください。", output))
                                } else if is_ask_interface_choice {
                                    Some(format!("ユーザーがインターフェースとして「{}」を選択・入力しました。この情報を反映して設定を生成または変更してください。", output))
                                } else if is_ask_ipaddress_choice {
                                    Some(format!("ユーザーがIPアドレス（およびサブネット）として「{}」を指定・確定しました。この情報を反映して設定を生成または変更してください。", output))
                                } else {
                                    None
                                };
                                let subsequent_task_ref = if let Some(ref s) = subsequent_task {
                                    Some(s.as_str())
                                } else if is_any_choice {
                                    custom_subsequent_task.as_deref()
                                } else {
                                    route_subsequent_task.as_deref()
                                };

                                 let user_message = if active_route == Route::Builder {
                                     replace_interface_abbreviations(&user_message)
                                 } else {
                                     user_message
                                 };
                                 let worker_res = if let Some(worker_mutex) = get_worker_for_route(&shared_model, active_route) {
                                    let mut worker = worker_mutex.lock().unwrap();
                                    let agent_name = worker.agent_name();
                                    let _ = window.emit("chat-event", crate::mcp::protocol::ChatEvent::AgentSelected(agent_name.to_string()));
                                    worker.ask(
                                        &model,
                                        &backend,
                                        None,
                                        Some(user_message),
                                        Some(tool_label),
                                        Some(output),
                                        history_block,
                                        subsequent_task_ref,
                                        Some(&window),
                                        settings.temperature,
                                        settings.repetition_penalty,
                                    ).map_err(LlmError::Worker)
                                } else {
                                    Ok("実行が完了しました。".to_string())
                                };
                                worker_res
                            }
                        })();
                        let _ = respond_to.send(res);
                    }
                    InferenceRequest::Background { prompt, app, respond_to } => {
                        let res = (|| -> Result<String, LlmError> {
                            let settings = crate::settings::load_settings(app.clone()).unwrap_or_default();
                            let model = shared_model.model.clone();
                            let backend = shared_model.backend.clone();
                            let mut worker = shared_model.summarization.lock().unwrap();

                            log::info!("LLM Background Prompt: {}", prompt);
                            let res = worker.ask(
                                &model,
                                &backend,
                                Some(prompt),
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
                        })();
                        let _ = respond_to.send(res);
                    }
                    InferenceRequest::Internal { prompt, system_prompt, app, respond_to } => {
                        let res = (|| -> Result<String, LlmError> {
                            let formatted_prompt = format!(
                                "<|turn>system\n{}<turn|>\n<|turn>user\n{}<turn|>\n<|turn>model\n",
                                system_prompt,
                                prompt
                            );

                            log::info!("--- INTERNAL LLM PROMPT ---\n{}\n-------------------------", formatted_prompt);

                            let settings = crate::settings::load_settings(app.clone()).unwrap_or_default();
                            let n_ctx = settings.n_ctx;
                            let max_gen = settings.max_gen;

                            let mut ctx_params = LlamaContextParams::default();
                            ctx_params = ctx_params.with_n_ctx(NonZeroU32::new(n_ctx as u32));
                            ctx_params = ctx_params.with_n_batch(n_ctx as u32);
                            ctx_params = ctx_params.with_type_k(llama_cpp_2::context::params::KvCacheType::Q4_0);
                            ctx_params = ctx_params.with_type_v(llama_cpp_2::context::params::KvCacheType::Q4_0);
                            ctx_params = ctx_params.with_flash_attention_policy(1);

                            let mut ctx = shared_model.model.new_context(&shared_model.backend, ctx_params)
                                .map_err(|e| LlmError::ContextCreation(format!("{:?}", e)))?;

                            let tokens = prepare_prompt_tokens_with_limit(&shared_model.model, &formatted_prompt, n_ctx, max_gen, settings.prompt_keep_tokens)?;

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

                            let turn_end_tokens = shared_model.model.str_to_token("<turn|>", AddBos::Never).unwrap_or_default();
                            let turn_end_token = turn_end_tokens.first().copied();

                            let n_len = max_gen;

                            let mut bytes_accumulator = Vec::new();

                            for _ in 0..n_len {
                                if is_cancelled() {
                                    log::info!("LLM internal loop cancelled");
                                    break;
                                }
                                let new_token_id = sampler.sample(&mut ctx, batch.n_tokens() - 1);

                                if new_token_id == shared_model.model.token_eos() || Some(new_token_id) == turn_end_token {
                                    break;
                                }

                                let mut token_bytes = shared_model.model.token_to_piece_bytes(new_token_id, 256, false, None).unwrap_or(vec![]);
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
                        })();
                        let _ = respond_to.send(res);
                    }
                }
            }
        });

        Ok(Self {
            shared,
            status,
            backend: backend_arc,
            inference_tx: tx,
        })
    }
}



#[tauri::command]
pub async fn get_model_status(state: tauri::State<'_, LlamaState>) -> Result<ModelState, TauriError> {
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
    keep_tokens: usize,
) -> Result<Vec<llama_cpp_2::token::LlamaToken>, LlmError> {
    let mut tokens = model.str_to_token(prompt, AddBos::Always).map_err(|e| LlmError::Tokenization(format!("{:?}", e)))?;

    let max_tokens = n_ctx.saturating_sub(max_gen);
    if tokens.len() > max_tokens {
        log::warn!("Prompt too long ({} tokens). Truncating to {} tokens.", tokens.len(), max_tokens);

        let note_tokens = model.str_to_token("\n※コンテキストの一部が省略されました\n", AddBos::Never).map_err(|e| LlmError::Tokenization(format!("{:?}", e)))?;
        let note_len = note_tokens.len();

        let to_remove = tokens.len() - max_tokens;
        let start_keep = keep_tokens;

        if tokens.len() > start_keep + to_remove + note_len {
            let remaining_space = max_tokens.saturating_sub(start_keep).saturating_sub(note_len);
            let start_take = tokens.len() - remaining_space;
            
            let mut new_tokens = tokens[..start_keep].to_vec();
            new_tokens.extend_from_slice(&note_tokens);
            new_tokens.extend_from_slice(&tokens[start_take..]);
            tokens = new_tokens;
        } else if max_tokens > note_len {
            let remaining_space = max_tokens - note_len;
            let start_take = tokens.len() - remaining_space;
            let mut new_tokens = note_tokens;
            new_tokens.extend_from_slice(&tokens[start_take..]);
            tokens = new_tokens;
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
                let _ = w.emit("chat-event", crate::mcp::protocol::ChatEvent::LlmChunk(s.to_string()));
            }
            result_string.push_str(s);
            bytes_accumulator.clear();
        }
        Err(e) => {
            let utf8_error_index = e.valid_up_to();
            let valid_str = String::from_utf8_lossy(&bytes_accumulator[..utf8_error_index]).to_string();
            if let Some(w) = window {
                let _ = w.emit("chat-event", crate::mcp::protocol::ChatEvent::LlmChunk(valid_str.clone()));
            }
            result_string.push_str(&valid_str);
            bytes_accumulator.drain(..utf8_error_index);
            if bytes_accumulator.len() > 8 {
                 let s = String::from_utf8_lossy(bytes_accumulator);
                 if let Some(w) = window {
                     let _ = w.emit("chat-event", crate::mcp::protocol::ChatEvent::LlmChunk(s.to_string()));
                 }
                 result_string.push_str(&s);
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
    pub is_builder: Option<bool>,
    pub history_block: Option<String>,
    pub subsequent_task: Option<String>,
}

pub async fn ask_llm_initial_internal(
    window: tauri::Window,
    prompt: String,
    llama_state: &LlamaState,
) -> Result<(String, Route), LlmError> {
    let AskInitialPayload { prompt: _ } = AskInitialPayload { prompt: prompt.clone() };
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

    let has_image_attachment = original_query.contains("【添付画像Vision解析情報") || original_query.contains("[添付画像:");

    if !has_image_attachment {
        if let Some((tool_name, params, message, confidence)) = crate::llm::fastrouter::detect_shortcut_tool(&original_query) {
            if confidence >= 0.8 {
                let tool_call = serde_json::json!({
                    "tool_name": tool_name,
                    "params": params
                });
                let response_str = format!("{}\n\n```json\n{}\n```", message, serde_json::to_string_pretty(&tool_call).unwrap());
                let _ = window.emit("chat-event", crate::mcp::protocol::ChatEvent::LlmChunk(response_str.clone()));
                return Ok((response_str, Route::None));
            }
        }
    }

    log::info!("Received original query (has_image={}): '{}'", has_image_attachment, original_query);

    if crate::llm::greeting::is_greeting(&original_query) {
        let resp = crate::llm::greeting::stream_self_introduction(&window).await;
        return Ok((resp, Route::None));
    }

    let (tx, rx) = tokio::sync::oneshot::channel();
    llama_state.inference_tx.send(InferenceRequest::Initial {
        window,
        prompt: prompt.clone(),
        original_query,
        respond_to: tx,
    }).await.map_err(|e| LlmError::Worker(format!("Failed to send inference request: {}", e)))?;

    let inference_result = rx.await.map_err(|e| LlmError::Worker(format!("Failed to receive inference result: {}", e)))??;

    log::info!("LLM Initial Prompt: {:?}\nResponse: {:?}", prompt, inference_result);
    Ok(inference_result)
}

#[tauri::command]
pub async fn ask_llm_initial(
    window: tauri::Window,
    payload: AskInitialPayload,
    llama_state: tauri::State<'_, LlamaState>,
) -> Result<String, TauriError> {
    let AskInitialPayload { prompt } = payload;
    let (response, _route) = ask_llm_initial_internal(window, prompt, &*llama_state)
        .await
        .map_err(TauriError::from)?;
    Ok(response)
}

#[tauri::command]
pub async fn analyze_tool_output(
    window: tauri::Window,
    payload: AnalyzePayload,
    llama_state: tauri::State<'_, LlamaState>,
) -> Result<String, TauriError> {
    let AnalyzePayload {
        user_message,
        tool_label,
        output,
        is_rag,
        is_builder,
        history_block,
        subsequent_task,
    } = payload;

    let user_message_log = user_message.clone();
    let is_builder_val = is_builder.unwrap_or(false);

    let (tx, rx) = tokio::sync::oneshot::channel();
    llama_state.inference_tx.send(InferenceRequest::Analyze {
        window,
        user_message,
        tool_label,
        output,
        is_rag,
        is_builder: is_builder_val,
        history_block,
        subsequent_task,
        respond_to: tx,
    }).await.map_err(|e| TauriError::from(LlmError::Worker(format!("Failed to send inference request: {}", e))))?;

    let inference_result = rx.await.map_err(|e| TauriError::from(LlmError::Worker(format!("Failed to receive inference result: {}", e))))??;

    log::info!("LLM Analysis User Message: {:?}\nResponse: {}", user_message_log, inference_result);
    Ok(inference_result)
}

pub async fn ask_llm_internal(
    prompt: &str,
    system_prompt: &str,
    app: &tauri::AppHandle,
    state: &LlamaState,
) -> Result<String, LlmError> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    state.inference_tx.send(InferenceRequest::Internal {
        prompt: prompt.to_string(),
        system_prompt: system_prompt.to_string(),
        app: app.clone(),
        respond_to: tx,
    }).await.map_err(|e| LlmError::Worker(format!("Failed to send inference request: {}", e)))?;

    rx.await.map_err(|e| LlmError::Worker(format!("Failed to receive inference result: {}", e)))?
}

#[tauri::command]
pub async fn ask_llm_background(
    prompt: String, 
    app: tauri::AppHandle,
    state: tauri::State<'_, LlamaState>
) -> Result<String, TauriError> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    state.inference_tx.send(InferenceRequest::Background {
        prompt,
        app,
        respond_to: tx,
    }).await.map_err(|e| TauriError::from(LlmError::Worker(format!("Failed to send inference request: {}", e))))?;

    let inference_result = rx.await.map_err(|e| TauriError::from(LlmError::Worker(format!("Failed to receive inference result: {}", e))))??;
    Ok(inference_result)
}

pub fn replace_interface_abbreviations(input: &str) -> String {
    use regex::{Regex, Captures};
    use std::sync::OnceLock;

    static FA_REGEX: OnceLock<Regex> = OnceLock::new();
    static GI_REGEX: OnceLock<Regex> = OnceLock::new();
    static TE_REGEX: OnceLock<Regex> = OnceLock::new();

    let fa = FA_REGEX.get_or_init(|| Regex::new(r"(?i)(?:^|([^a-zA-Z]))fa\s*(\d)").unwrap());
    let gi = GI_REGEX.get_or_init(|| Regex::new(r"(?i)(?:^|([^a-zA-Z]))gi\s*(\d)").unwrap());
    let te = TE_REGEX.get_or_init(|| Regex::new(r"(?i)(?:^|([^a-zA-Z]))te\s*(\d)").unwrap());

    let res = fa.replace_all(input, |caps: &Captures| {
        let prefix = caps.get(1).map_or("", |m| m.as_str());
        let num = &caps[2];
        format!("{}fastethernet{}", prefix, num)
    });
    let res = gi.replace_all(&res, |caps: &Captures| {
        let prefix = caps.get(1).map_or("", |m| m.as_str());
        let num = &caps[2];
        format!("{}gigabitethernet{}", prefix, num)
    });
    let res = te.replace_all(&res, |caps: &Captures| {
        let prefix = caps.get(1).map_or("", |m| m.as_str());
        let num = &caps[2];
        format!("{}tengigabitethernet{}", prefix, num)
    });

    res.into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replace_interface_abbreviations() {
        assert_eq!(replace_interface_abbreviations("Fa0/1"), "fastethernet0/1");
        assert_eq!(replace_interface_abbreviations("gi 1/0/2"), "gigabitethernet1/0/2");
        assert_eq!(replace_interface_abbreviations("Te2/1"), "tengigabitethernet2/1");
        assert_eq!(replace_interface_abbreviations("Interface Fa0/1をアクセスに"), "Interface fastethernet0/1をアクセスに");
        assert_eq!(replace_interface_abbreviations("Sofa0/1"), "Sofa0/1");
        assert_eq!(replace_interface_abbreviations("FA1"), "fastethernet1");
    }
}


