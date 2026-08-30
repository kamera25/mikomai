pub mod extract;
pub mod flow;
pub mod registry;
pub mod tools;

pub use extract::*;
pub use flow::*;
pub use registry::*;

use tauri::{AppHandle, Emitter, State, Window};

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
    crate::llm::llm::reset_cancel();

    let ChatRequest {
        user_message,
        summaries,
        recent_ips: _,
        history_limit,
        mcp_timeout: _,
        attachments,
    } = payload;

    if crate::llm::llm::is_cancelled() {
        return Ok(());
    }

    let mut final_user_message = user_message.clone();
    let settings = crate::settings::load_settings(app.clone()).unwrap_or_default();

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
        return Ok(());
    }

    // Select from the user's request, never from attachment contents. An
    // attached command example must not by itself escalate a documentation
    // question into an autonomous device investigation.
    match crate::harness::dispatch::select_dispatch_mode_for_request(&app, &user_message) {
        crate::harness::dispatch::DispatchMode::Agent => {
            // AgentLoop owns live network observation and tool execution.
            // Its policy validator remains the only route to side effects.
            let mut agent_loop = crate::harness::agent_loop::AgentLoop::new(app, window, 10);
            agent_loop.run(final_user_message, &*llama_state).await?;
        }
        crate::harness::dispatch::DispatchMode::Worker => {
            run_worker_request(
                window,
                final_user_message,
                &*llama_state,
                crate::mcp::executor::extract::get_history_block_rust(&summaries, history_limit),
                attachments.as_ref().map_or(false, |items| {
                    items
                        .iter()
                        .any(|item| matches!(item.mime_type, crate::history::AttachmentType::Image))
                }),
            )
            .await?;
        }
    }

    Ok(())
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
) -> Result<(), String> {
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
    let response = crate::llm::llm::ask_llm_initial_internal(window.clone(), prompt, llama_state)
        .await
        .map(|(response, _route)| response)
        .unwrap_or_else(|error| format!("回答の生成に失敗しました: {}", error));

    let _ = window.emit(
        "chat-event",
        crate::mcp::protocol::ChatEvent::McpInitialFinished(
            crate::mcp::protocol::InitialFinishedPayload {
                task_id,
                content: response,
            },
        ),
    );
    Ok(())
}
