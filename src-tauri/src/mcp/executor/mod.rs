pub mod extract;
pub mod flow;
pub mod registry;
pub mod tools;

pub use extract::*;
pub use flow::*;
pub use registry::*;

use tauri::{AppHandle, Emitter, Manager, State, Window};

use crate::mcp::protocol::ChatRequest;

#[tauri::command]
pub async fn execute_mcp_tool(
    app: AppHandle,
    window: Window,
    _llama_state: State<'_, crate::llm::llm::LlamaState>,
    payload: ExecuteMcpToolPayload,
) -> Result<String, String> {
    let tool_calls = vec![ToolCall {
        tool: payload.tool_id.clone(),
        args: payload.args.clone(),
    }];
    let is_builder_caller = std::str::FromStr::from_str(&payload.tool_id)
        .map_or(false, |k: crate::mcp::ToolKind| k.is_builder_tool());

    execute_mcp_tools_flow(
        app,
        window,
        payload.user_message,
        tool_calls,
        payload.summaries,
        payload.recent_ips,
        payload.history_limit,
        payload.mcp_timeout,
        0,
        is_builder_caller,
    )
    .await
}

#[tauri::command]
pub async fn handle_mcp_message(
    app: AppHandle,
    window: Window,
    llama_state: State<'_, crate::llm::llm::LlamaState>,
    payload: ChatRequest,
) -> Result<(), String> {
    handle_chat_request(app, window, &llama_state, payload)
        .await
        .map(|_| ())
}

/// Executes the chat pipeline used by both the desktop command and the CLI.
///
/// The Tauri command above preserves the GUI's event-driven contract. This
/// function additionally returns the final response so non-GUI adapters do
/// not have to recreate a chat timeline from `chat-event` notifications.
pub async fn handle_chat_request(
    app: AppHandle,
    window: Window,
    llama_state: &crate::llm::llm::LlamaState,
    payload: ChatRequest,
) -> Result<String, String> {
    crate::llm::llm::reset_cancel();
    // Keep foreground priority for the complete request, including routing,
    // device I/O, and response generation.
    let _foreground_guard = app
        .state::<crate::background_work::BackgroundWorkState>()
        .begin_foreground_query();

    let ChatRequest {
        user_message,
        summaries,
        recent_ips: _,
        history_limit,
        mcp_timeout: _,
        attachments,
    } = payload;

    if crate::llm::llm::is_cancelled() {
        return Ok(String::new());
    }

    let mut final_user_message = user_message.clone();
    let settings = crate::settings::load_settings(app.clone()).unwrap_or_default();

    let has_image_attachment = attachments.as_ref().map_or(false, |items| {
        items
            .iter()
            .any(|item| matches!(item.mime_type, crate::history::AttachmentType::Image))
    });

    // 1. FastRouter (Shortcut) routing: Execute without loading the LLM model
    if !has_image_attachment {
        if let Some(decision) = crate::llm::router::shortcut::detect_shortcut(&user_message) {
            if decision.confidence >= 0.8 {
                return run_shortcut_request(app, window, user_message, decision, llama_state).await;
            }
        }
    }

    // 2. LLM is required: Ensure the model is loaded on demand
    crate::llm::loader::ensure_model_loaded(&app, llama_state).await?;

    if let Some(att_list) = &attachments {
        for att in att_list {
            match att.mime_type {
                crate::history::AttachmentType::Text => {
                    if let Some(path) = &att.path {
                        final_user_message.push_str(&format!(
                            "\n\n--- 添付ファイル: {} (ローカルパス: {}) ---\n{}",
                            att.name,
                            path.display(),
                            att.content
                        ));
                    } else {
                        final_user_message.push_str(&format!(
                            "\n\n--- 添付ファイル: {} ---\n{}",
                            att.name, att.content
                        ));
                    }
                }
                crate::history::AttachmentType::Image => {
                    let analysis = crate::llm::vision::analyze_image_attachment(
                        &att.name,
                        att.mime_type.as_str(),
                        &att.content,
                        settings.vision_enabled,
                        settings.mmproj_path.as_deref(),
                        &*llama_state,
                    )
                    .await;
                    if let Some(path) = &att.path {
                        final_user_message.push_str(&format!(
                            "\n\n[添付画像: {} (ローカルパス: {})]\n{}",
                            att.name,
                            path.display(),
                            analysis.extracted_context
                        ));
                    } else {
                        final_user_message.push_str(&format!("\n\n{}", analysis.extracted_context));
                    }
                }
                crate::history::AttachmentType::File => {
                    if let Some(path) = &att.path {
                        let path_str = path.display();
                        final_user_message.push_str(&format!(
                            "\n\n--- 添付ファイル: {} (ローカルパス: {}) ---\n※バイナリまたは大容量ファイルのため内容は省略されています。",
                            att.name, path_str
                        ));
                    } else {
                        final_user_message.push_str(&format!(
                            "\n\n--- 添付ファイル: {} ---\n{}",
                            att.name, att.content
                        ));
                    }
                }
            }
        }
    }

    if crate::llm::llm::is_cancelled() {
        return Ok(String::new());
    }

    // Select from the user's request, never from attachment contents. An
    // attached command example must not by itself escalate a documentation
    // question into an autonomous device investigation.
    match crate::harness::dispatch::select_dispatch_mode_for_request(&app, &user_message) {
        crate::harness::dispatch::DispatchMode::Agent => {
            // AgentLoop owns live network observation and tool execution.
            // Its policy validator remains the only route to side effects.
            let mut agent_loop = crate::harness::agent_loop::AgentLoop::new(app, window, 10);
            agent_loop.run(final_user_message, llama_state).await
        }
        crate::harness::dispatch::DispatchMode::Worker => {
            run_worker_request(
                window,
                final_user_message,
                llama_state,
                crate::mcp::executor::extract::get_history_block_rust(&summaries, history_limit),
                attachments.as_ref().map_or(false, |items| {
                    items
                        .iter()
                        .any(|item| matches!(item.mime_type, crate::history::AttachmentType::Image))
                }),
            )
            .await
        }
    }
}

/// Runs the existing Router -> specialised Worker pipeline for bounded work.
/// Workers may provide guidance or a draft, but they do not execute MCP tools
/// from this entry point. Device I/O must enter through `AgentLoop` above.
async fn run_worker_request(
    window: Window,
    user_message: String,
    llama_state: &crate::llm::llm::LlamaState,
    history_block: String,
    has_attachments: bool,
) -> Result<String, String> {
    let task_id = uuid::Uuid::new_v4();
    let _ = window.emit(
        "chat-event",
        crate::mcp::protocol::ChatEvent::McpInitialStarted(
            crate::mcp::protocol::InitialStartedPayload {
                task_id,
                has_image: has_attachments,
            },
        ),
    );

    let prompt = format!("【ユーザー入力】\n{}{}", user_message, history_block);
    // Use the same timeline primitives as AgentLoop.  The constrained JSON
    // decision stays internal; the user sees progress, not protocol data.
    let _ = window.emit(
        "chat-event",
        crate::mcp::protocol::ChatEvent::AgentSelected("エージェントによる解析を開始".to_string()),
    );
    let _ = window.emit(
        "chat-event",
        crate::mcp::protocol::ChatEvent::LlmChunk(
            "\n```agent-step\nphase: planning\nstep: 1\n```\n".to_string(),
        ),
    );
    let response = match crate::llm::llm::ask_llm_initial_internal(
        window.clone(),
        prompt,
        llama_state,
    )
    .await
    {
        Ok((response, crate::llm::worker::Route::Knowledge)) => {
            run_knowledge_retrieval(
                window.clone(),
                user_message,
                history_block,
                response,
                llama_state,
            )
            .await
        }
        Ok((response, _)) => response,
        Err(error) => format!("回答の生成に失敗しました: {}", error),
    };

    let _ = window.emit(
        "chat-event",
        crate::mcp::protocol::ChatEvent::McpInitialFinished(
            crate::mcp::protocol::InitialFinishedPayload {
                task_id,
                content: response.clone(),
            },
        ),
    );
    Ok(response)
}

/// Completes the Knowledge Worker's constrained SEARCH decision without going
/// through the MCP tool-call parser.  The same retrieval implementation is
/// used directly, then the existing RAG answer worker turns the evidence into
/// a user-facing answer.
async fn run_knowledge_retrieval(
    window: Window,
    user_message: String,
    history_block: String,
    decision_json: String,
    llama_state: &crate::llm::llm::LlamaState,
) -> String {
    use crate::llm::worker::knowledge::{parse_knowledge_decision, KnowledgeAction};

    let decision = match parse_knowledge_decision(&decision_json) {
        Ok(decision) => decision,
        Err(error) => return format!("知識検索の判断を処理できませんでした: {error}"),
    };

    match decision.action {
        KnowledgeAction::Answer => decision.answer,
        KnowledgeAction::Search => {
            if decision.query.trim().is_empty() {
                return "知識検索のクエリが空のため、検索できませんでした。".to_string();
            }

            let query = decision.query;
            let keyword = query.trim().to_string();
            let _ = window.emit(
                "chat-event",
                crate::mcp::protocol::ChatEvent::LlmChunk(format!(
                    "\n```agent-decision\nstep: 1\naction: NW-DB検索\nobjective: キーワード「{}」で検索しています…\nreason: 確認可能な技術資料に基づいて回答するため\n```\n",
                    keyword.replace('\n', " ")
                )),
            );

            let app = window.app_handle();
            let rag_state = app.state::<crate::mcp::rag::RagState>();
            let result =
                match crate::mcp::rag::query_nw_db(query, None, rag_state, app.clone()).await {
                    Ok(result) => result,
                    Err(error) => return format!("NW-DB検索に失敗しました: {error}"),
                };

            // A search decision is not evidence that the database query
            // completed. Surface the actual result so users can distinguish
            // a zero-hit query from an unavailable database or a model-only
            // answer.
            let result_count = result.output.matches("--- 根拠 [").count();
            let result_status = if result_count == 0 {
                "NW-DBを照会しましたが、該当資料は0件でした。".to_string()
            } else {
                format!("NW-DBを照会し、該当資料を{}件取得しました。", result_count)
            };
            log::info!(
                "[KnowledgeWorker] NW-DB query completed: query={:?}, citations={}",
                keyword,
                result_count
            );
            let _ = window.emit(
                "chat-event",
                crate::mcp::protocol::ChatEvent::LlmChunk(format!(
                    "\n```agent-decision\nstep: 1\naction: NW-DB検索完了\nobjective: {}\nreason: 検索結果を確認済み\n```\n",
                    result_status
                )),
            );

            let graph = app.state::<crate::graph::SurrealDbState>();
            let previews = match crate::mcp::rag::previews_for_search_result(&result.output, &graph).await {
                Ok(previews) => previews,
                Err(error) => {
                    log::warn!("[KnowledgeWorker] Failed to load RAG document previews: {error}");
                    Vec::new()
                }
            };
            let settings = crate::settings::load_settings(app.clone()).unwrap_or_default();
            let selected_paths = {
                let shared = llama_state.shared.lock().await;
                shared.as_ref().and_then(|shared| {
                    crate::llm::worker::rag::select_documents(
                        &shared.model,
                        &shared.backend,
                        &user_message,
                        &previews,
                        settings.temperature,
                        settings.repetition_penalty,
                    ).map_err(|error| log::warn!("[KnowledgeWorker] {error}")).ok()
                })
            };
            // The selector must not make an empty or malformed decision hide
            // every source. Use the highest-ranked previews as a bounded,
            // observable fallback.
            let selected_paths = selected_paths.filter(|paths| !paths.is_empty()).unwrap_or_else(|| {
                previews.iter().take(3).map(|preview| preview.path.clone()).collect()
            });
            let expanded_documents = match crate::mcp::rag::expand_selected_documents(&selected_paths, &graph).await {
                Ok(documents) => documents,
                Err(error) => {
                    log::warn!("[KnowledgeWorker] Failed to expand selected RAG documents: {error}");
                    String::new()
                }
            };
            let _ = window.emit(
                "chat-event",
                crate::mcp::protocol::ChatEvent::LlmChunk(format!(
                    "\n```agent-decision\nstep: 2\naction: 資料展開\nobjective: LLMが選択した{}件の資料本文を参照しています…\nreason: 回答に必要な手順とコマンドを確認するため\n```\n",
                    selected_paths.len()
                )),
            );
            let answer_context = if expanded_documents.is_empty() {
                result.output
            } else {
                format!(
                    "検索時の根拠一覧:\n{}\n\nLLMが選択して展開した資料本文:\n{}",
                    result.output, expanded_documents
                )
            };

            crate::llm::llm::analyze_tool_output_internal(
                window,
                crate::llm::llm::AnalyzePayload {
                    user_message,
                    tool_label: "NW-DB検索".to_string(),
                    output: answer_context,
                    is_rag: true,
                    is_builder: Some(false),
                    history_block: Some(history_block),
                    subsequent_task: None,
                },
                llama_state,
            )
            .await
            .unwrap_or_else(|error| format!("検索結果の回答生成に失敗しました: {error}"))
        }
    }
}

async fn run_shortcut_request(
    app: AppHandle,
    window: Window,
    user_message: String,
    decision: crate::llm::router::RoutingDecision,
    llama_state: &crate::llm::llm::LlamaState,
) -> Result<String, String> {
    let planner = crate::harness::shortcut_planner::ShortcutPlanner::new(decision);
    let executor = crate::harness::ports::McpToolExecutorPort::new(app.clone(), window.clone(), llama_state);
    let reporter = crate::harness::ports::TauriReporterPort::new(window.clone());
    let mut agent_loop = crate::harness::agent_loop::AgentLoop::new(app, window, 5);
    agent_loop
        .run_with(user_message, &planner, &executor, &reporter)
        .await
}
