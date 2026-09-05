use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::model::AddBos;
use llama_cpp_2::model::LlamaModel;

use crate::error::TauriError;
use crate::llm::llm_manager::SharedModel;
use crate::llm::request::{InferenceRequest, InferenceRequestHandler};
use crate::llm::worker::Route;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{Emitter, Manager};

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
    #[allow(dead_code)]
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

pub struct LlamaState {
    pub shared: Arc<tokio::sync::Mutex<Option<Arc<SharedModel>>>>,
    pub status: tokio::sync::Mutex<ModelState>,
    pub backend: Arc<LlamaBackend>,
    pub inference_tx: tokio::sync::mpsc::Sender<InferenceRequest>,
}

static LLAMA_LOGS_INIT: std::sync::Once = std::sync::Once::new();

pub fn configure_llama_logs(enabled: bool) {
    LLAMA_LOGS_INIT.call_once(|| {
        llama_cpp_2::send_logs_to_tracing(
            llama_cpp_2::LogOptions::default().with_logs_enabled(enabled),
        );
    });
}

impl LlamaState {
    pub fn new() -> Result<Self, LlmError> {
        configure_llama_logs(false);
        let backend = LlamaBackend::init().map_err(|_| LlmError::BackendInit)?;
        let backend_arc = Arc::new(backend);
        let shared: Arc<tokio::sync::Mutex<Option<Arc<SharedModel>>>> =
            Arc::new(tokio::sync::Mutex::new(None));
        let status = tokio::sync::Mutex::new(ModelState::NotLoaded);

        let (tx, mut rx) = tokio::sync::mpsc::channel::<InferenceRequest>(100);

        let shared_clone = shared.clone();
        std::thread::spawn(move || {
            while let Some(req) = rx.blocking_recv() {
                let shared_model_opt: Option<Arc<SharedModel>> = {
                    let lock = shared_clone.blocking_lock();
                    lock.clone()
                };
                match shared_model_opt {
                    Some(shared_model) => req.handle(&shared_model),
                    None => req.reject(LlmError::ModelNotLoaded),
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
pub async fn get_model_status(
    state: tauri::State<'_, LlamaState>,
) -> Result<ModelState, TauriError> {
    let status_lock = state.status.lock().await;
    let status = match &*status_lock {
        ModelState::NotLoaded => ModelState::NotLoaded,
        ModelState::Loading => ModelState::Loading,
        ModelState::Loaded => ModelState::Loaded,
        ModelState::Error(e) => ModelState::Error(e.clone()),
    };
    Ok(status)
}

pub const SYSTEM_PROMPT: &str = include_str!("prompts/system_prompt.txt");

pub(crate) fn prepare_prompt_tokens_with_limit(
    model: &LlamaModel,
    prompt: &str,
    n_ctx: usize,
    max_gen: usize,
    keep_tokens: usize,
) -> Result<Vec<llama_cpp_2::token::LlamaToken>, LlmError> {
    let mut tokens = model
        .str_to_token(prompt, AddBos::Always)
        .map_err(|e| LlmError::Tokenization(format!("{:?}", e)))?;

    let max_tokens = n_ctx.saturating_sub(max_gen);
    if tokens.len() > max_tokens {
        log::warn!(
            "Prompt too long ({} tokens). Truncating to {} tokens.",
            tokens.len(),
            max_tokens
        );

        let note_tokens = model
            .str_to_token("\n※コンテキストの一部が省略されました\n", AddBos::Never)
            .map_err(|e| LlmError::Tokenization(format!("{:?}", e)))?;
        let note_len = note_tokens.len();

        let to_remove = tokens.len() - max_tokens;
        let start_keep = keep_tokens;

        if tokens.len() > start_keep + to_remove + note_len {
            let remaining_space = max_tokens
                .saturating_sub(start_keep)
                .saturating_sub(note_len);
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

pub(crate) fn process_token_bytes(
    bytes_accumulator: &mut Vec<u8>,
    result_string: &mut String,
    window: Option<&tauri::Window>,
) {
    match std::str::from_utf8(bytes_accumulator) {
        Ok(s) => {
            if let Some(w) = window {
                let _ = w.emit(
                    "chat-event",
                    crate::mcp::protocol::ChatEvent::LlmChunk(s.to_string()),
                );
            }
            result_string.push_str(s);
            bytes_accumulator.clear();
        }
        Err(e) => {
            let utf8_error_index = e.valid_up_to();
            let valid_str =
                String::from_utf8_lossy(&bytes_accumulator[..utf8_error_index]).to_string();
            if let Some(w) = window {
                let _ = w.emit(
                    "chat-event",
                    crate::mcp::protocol::ChatEvent::LlmChunk(valid_str.clone()),
                );
            }
            result_string.push_str(&valid_str);
            bytes_accumulator.drain(..utf8_error_index);
            if bytes_accumulator.len() > 8 {
                let s = String::from_utf8_lossy(bytes_accumulator);
                if let Some(w) = window {
                    let _ = w.emit(
                        "chat-event",
                        crate::mcp::protocol::ChatEvent::LlmChunk(s.to_string()),
                    );
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
    let AskInitialPayload { prompt: _ } = AskInitialPayload {
        prompt: prompt.clone(),
    };
    let original_query = if prompt.starts_with("【ユーザー入力】\n") {
        prompt
            .strip_prefix("【ユーザー入力】\n")
            .unwrap()
            .split("\n\n<memory>")
            .next()
            .unwrap_or(&prompt)
            .to_string()
    } else {
        prompt
            .split("\n\n<memory>")
            .next()
            .unwrap_or(&prompt)
            .chars()
            .take(300)
            .collect::<String>()
    };

    let has_image_attachment = original_query.contains("【添付画像Vision解析情報")
        || original_query.contains("[添付画像:");

    if !has_image_attachment {
        if let Some(decision) = crate::llm::router::shortcut::detect_shortcut(&original_query) {
            if decision.confidence >= 0.8 {
                let response_str = match decision.action {
                    crate::llm::router::RouteAction::StaticReply { message } => message,
                    crate::llm::router::RouteAction::DirectToolCall {
                        tool_name,
                        params,
                        message,
                    } => {
                        let tool_call = serde_json::json!({
                            "tool_name": tool_name,
                            "params": params
                        });
                        format!(
                            "{}\n\n```json\n{}\n```",
                            message,
                            serde_json::to_string_pretty(&tool_call).unwrap()
                        )
                    }
                    _ => String::new(),
                };
                if !response_str.is_empty() {
                    let _ = window.emit(
                        "chat-event",
                        crate::mcp::protocol::ChatEvent::LlmChunk(response_str.clone()),
                    );
                    return Ok((response_str, Route::None));
                }
            }
        }
    }

    log::info!(
        "Received original query (has_image={}): '{}'",
        has_image_attachment,
        original_query
    );

    let (tx, rx) = tokio::sync::oneshot::channel();
    llama_state
        .inference_tx
        .send(InferenceRequest::Initial {
            window,
            prompt: prompt.clone(),
            original_query,
            respond_to: tx,
        })
        .await
        .map_err(|e| LlmError::Worker(format!("Failed to send inference request: {}", e)))?;

    let inference_result = rx
        .await
        .map_err(|e| LlmError::Worker(format!("Failed to receive inference result: {}", e)))??;

    log::info!(
        "LLM Initial Prompt: {:?}\nResponse: {:?}",
        prompt,
        inference_result
    );
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
    analyze_tool_output_internal(window, payload, &llama_state)
        .await
        .map_err(TauriError::from)
}

/// Internal counterpart of the Tauri command.  Retrieval workers use this to
/// submit the answer-generation turn without manufacturing an MCP request.
pub async fn analyze_tool_output_internal(
    window: tauri::Window,
    payload: AnalyzePayload,
    llama_state: &LlamaState,
) -> Result<String, LlmError> {
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
    llama_state
        .inference_tx
        .send(InferenceRequest::Analyze {
            window,
            user_message,
            tool_label,
            output,
            is_rag,
            is_builder: is_builder_val,
            history_block,
            subsequent_task,
            respond_to: tx,
        })
        .await
        .map_err(|e| LlmError::Worker(format!("Failed to send inference request: {}", e)))?;

    let inference_result = rx
        .await
        .map_err(|e| LlmError::Worker(format!("Failed to receive inference result: {}", e)))??;

    log::info!(
        "LLM Analysis User Message: {:?}\nResponse: {}",
        user_message_log,
        inference_result
    );
    Ok(inference_result)
}

/// Delegates retrieval to the RAG co-worker. It uses constrained decoding to
/// choose document paths from metadata, then returns the selected documents'
/// original bodies. No user-facing answer or citation-index summary is made.
pub async fn ask_rag_co_worker(
    app: &tauri::AppHandle,
    user_message: String,
    search_output: String,
    state: &LlamaState,
) -> Result<String, LlmError> {
    let graph = app.state::<crate::graph::SurrealDbState>();
    let previews = crate::mcp::rag::previews_for_search_result(&search_output, &graph)
        .await
        .map_err(|error| {
            LlmError::Worker(format!("Failed to load RAG document previews: {error}"))
        })?;

    if previews.is_empty() {
        return Ok(search_output);
    }

    let settings = crate::settings::load_settings(app.clone()).unwrap_or_default();
    let selected_paths = {
        let shared = state.shared.lock().await;
        let shared = shared.as_ref().ok_or(LlmError::ModelNotLoaded)?;
        crate::llm::worker::rag::select_documents(
            &shared.model,
            &shared.backend,
            &user_message,
            &previews,
            settings.temperature,
            settings.repetition_penalty,
        )
        .map_err(LlmError::Worker)?
    };

    let selected_paths = if selected_paths.is_empty() {
        previews
            .iter()
            .take(3)
            .map(|preview| preview.path.clone())
            .collect::<Vec<_>>()
    } else {
        selected_paths
    };

    let documents = crate::mcp::rag::expand_selected_documents(&selected_paths, &graph)
        .await
        .map_err(|error| {
            LlmError::Worker(format!("Failed to expand RAG co-worker documents: {error}"))
        })?;

    if documents.trim().is_empty() {
        Ok(search_output)
    } else {
        Ok(documents)
    }
}

pub async fn ask_llm_internal(
    prompt: &str,
    system_prompt: &str,
    app: &tauri::AppHandle,
    state: &LlamaState,
) -> Result<String, LlmError> {
    ask_llm_internal_with_schema(prompt, system_prompt, None, app, state).await
}

pub async fn ask_llm_internal_with_schema(
    prompt: &str,
    system_prompt: &str,
    schema: Option<&str>,
    app: &tauri::AppHandle,
    state: &LlamaState,
) -> Result<String, LlmError> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    state
        .inference_tx
        .send(InferenceRequest::Internal {
            prompt: prompt.to_string(),
            system_prompt: system_prompt.to_string(),
            schema: schema.map(|s| s.to_string()),
            app: app.clone(),
            respond_to: tx,
        })
        .await
        .map_err(|e| LlmError::Worker(format!("Failed to send inference request: {}", e)))?;

    rx.await
        .map_err(|e| LlmError::Worker(format!("Failed to receive inference result: {}", e)))?
}

#[tauri::command]
pub async fn ask_llm_background(
    prompt: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, LlamaState>,
) -> Result<String, TauriError> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    state
        .inference_tx
        .send(InferenceRequest::Background {
            prompt,
            app,
            respond_to: tx,
        })
        .await
        .map_err(|e| {
            TauriError::from(LlmError::Worker(format!(
                "Failed to send inference request: {}",
                e
            )))
        })?;

    let inference_result = rx.await.map_err(|e| {
        TauriError::from(LlmError::Worker(format!(
            "Failed to receive inference result: {}",
            e
        )))
    })??;
    Ok(inference_result)
}

pub const BUILDER_DIFF_CONFIG_PROMPT: &str = "ユーザーから提供される設定情報は「差分（部分設定）」であることが前提です。たとえホスト名の変更のみであっても不完全とみなさず、提供されたパラメータだけを対象機器のコマンドに変換してください。ユーザーからの明示的な指示がない限り、不足していると思われる他の設定項目（インターフェースや経路など）を推測して補完したり、そのためにRAGを検索したりすることは厳禁です。";

pub fn prepare_builder_prompt(input: &str) -> String {
    let text = replace_interface_abbreviations(input);
    if text.contains(BUILDER_DIFF_CONFIG_PROMPT) {
        text
    } else {
        format!("{}\n\n{}", BUILDER_DIFF_CONFIG_PROMPT, text)
    }
}

pub fn replace_interface_abbreviations(input: &str) -> String {
    use regex::{Captures, Regex};
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
        assert_eq!(
            replace_interface_abbreviations("gi 1/0/2"),
            "gigabitethernet1/0/2"
        );
        assert_eq!(
            replace_interface_abbreviations("Te2/1"),
            "tengigabitethernet2/1"
        );
        assert_eq!(
            replace_interface_abbreviations("Interface Fa0/1をアクセスに"),
            "Interface fastethernet0/1をアクセスに"
        );
        assert_eq!(replace_interface_abbreviations("Sofa0/1"), "Sofa0/1");
        assert_eq!(replace_interface_abbreviations("FA1"), "fastethernet1");
    }

    #[test]
    fn test_prepare_builder_prompt() {
        let input = "Fa0/1 を設定する";
        let res = prepare_builder_prompt(input);
        assert!(res.starts_with(BUILDER_DIFF_CONFIG_PROMPT));
        assert!(res.contains("fastethernet0/1 を設定する"));

        let res_twice = prepare_builder_prompt(&res);
        assert_eq!(res, res_twice);
    }
}
