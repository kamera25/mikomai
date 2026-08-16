pub mod extract;
pub mod flow;
pub mod registry;
pub mod tools;

pub use extract::*;
pub use flow::*;
pub use registry::*;

use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, State, Window};

use crate::mcp::protocol::{ChatEvent, ChatRequest, InitialFinishedPayload, InitialStartedPayload};

#[tauri::command]
pub async fn execute_mcp_tool(
    app: AppHandle,
    window: Window,
    _llama_state: State<'_, crate::llm::llm::LlamaState>,
    payload: ExecuteMcpToolPayload,
) -> Result<String, String>
{
    let tool_calls = vec![ToolCall {
        tool: payload.tool_id.clone(),
        args: payload.args.clone(),
    }];
    let is_builder_caller = payload.tool_id == "ask_user_choice"
        || payload.tool_id == "ask_interface_choice"
        || payload.tool_id == "ask_ipaddress_choice"
        || payload.tool_id == "validate_cisco_config"
        || payload.tool_id == "convert_cisco_config"
        || payload.tool_id == "self_network_nwdiag";

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
) -> Result<(), String>
{
    crate::llm::llm::reset_cancel();

    let ChatRequest {
        user_message,
        summaries,
        recent_ips,
        history_limit,
        mcp_timeout,
        attachments,
    } = payload;

    if crate::llm::llm::is_cancelled()
    {
        return Ok(());
    }

    // 1. Generate thinkingTaskId and emit mcp-initial-started
    let thinking_task_id = format!("task_think_{}", chrono::Utc::now().timestamp_millis());

    let has_image = attachments.as_ref().map_or(false, |atts| {
        atts.iter()
            .any(|att| att.mime_type == crate::history::AttachmentType::Image)
    });

    let _ = window.emit(
        "chat-event",
        ChatEvent::McpInitialStarted(InitialStartedPayload {
            task_id: thinking_task_id.clone(),
            has_image,
        }),
    );

    // 2. Build history block and prompt
    let settings = crate::settings::load_settings(app.clone()).unwrap_or_default();
    let history_block = get_history_block_rust(&summaries, history_limit);
    let mut final_user_message = user_message.clone();
    if let Some(att_list) = &attachments
    {
        for att in att_list
        {
            match att.mime_type
            {
                crate::history::AttachmentType::Text =>
                {
                    if let Some(path) = &att.path
                    {
                        final_user_message.push_str(&format!(
                            "\n\n--- 添付ファイル: {} (ローカルパス: {}) ---\n{}",
                            att.name, path, att.content
                        ));
                    }
                    else
                    {
                        final_user_message.push_str(&format!(
                            "\n\n--- 添付ファイル: {} ---\n{}",
                            att.name, att.content
                        ));
                    }
                }
                crate::history::AttachmentType::Image =>
                {
                    let analysis = crate::llm::vision::analyze_image_attachment(
                        &att.name,
                        att.mime_type.as_str(),
                        &att.content,
                        settings.vision_enabled,
                        settings.mmproj_path.as_deref(),
                        &*llama_state,
                    )
                    .await;
                    if let Some(path) = &att.path
                    {
                        final_user_message.push_str(&format!(
                            "\n\n[添付画像: {} (ローカルパス: {})]\n{}",
                            att.name, path, analysis.extracted_context
                        ));
                    }
                    else
                    {
                        final_user_message.push_str(&format!("\n\n{}", analysis.extracted_context));
                    }
                }
                crate::history::AttachmentType::File =>
                {
                    // Binary or large file
                    if let Some(path) = &att.path
                    {
                        final_user_message.push_str(&format!(
                            "\n\n--- 添付ファイル: {} (ローカルパス: {}) ---\n※バイナリまたは大容量ファイルのため内容は省略されています。機器へのアップロード等のツール実行時は local_file 引数に '{}' を指定してください。",
                            att.name, path, path
                        ));
                    }
                    else
                    {
                        final_user_message.push_str(&format!(
                            "\n\n--- 添付ファイル: {} ---\n{}",
                            att.name, att.content
                        ));
                    }
                }
            }
        }
    }
    let prompt_with_context = format!("【ユーザー入力】\n{}{}", final_user_message, history_block);

    if crate::llm::llm::is_cancelled()
    {
        return Ok(());
    }

    // 3. Call ask_llm_initial_internal to get the route along with response
    let (response, route) = match crate::llm::llm::ask_llm_initial_internal(
        window.clone(),
        prompt_with_context,
        &*llama_state,
    )
    .await
    {
        Ok(res) => res,
        Err(e) =>
        {
            return Err(e.to_string());
        }
    };

    if crate::llm::llm::is_cancelled()
    {
        return Ok(());
    }

    let _ = window.emit(
        "chat-event",
        ChatEvent::McpInitialFinished(InitialFinishedPayload {
            task_id: thinking_task_id.clone(),
            content: response.clone(),
        }),
    );

    // 4. Extract and parse tool calls
    let json_blocks = extract_json_blocks(&response);

    let mut tool_calls = Vec::new();
    for block in json_blocks
    {
        if let Ok(parsed) = serde_json::from_str::<Value>(&block)
        {
            let tool = parsed
                .get("tool_name")
                .or_else(|| parsed.get("tool"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let args = parsed
                .get("params")
                .or_else(|| parsed.get("args"))
                .cloned()
                .unwrap_or(Value::Object(serde_json::Map::new()));
            if let Some(t) = tool
            {
                tool_calls.push(ToolCall { tool: t, args });
            }
        }
    }

    if !tool_calls.is_empty()
    {
        let is_builder_caller = route == crate::llm::worker::Route::Builder
            || route == crate::llm::worker::Route::Plotter
            || tool_calls.iter().any(|t| {
                t.tool == "self_network_nwdiag"
                    || t.tool == "validate_cisco_config"
                    || t.tool == "convert_cisco_config"
                    || t.tool == "ask_user_choice"
                    || t.tool == "ask_interface_choice"
                    || t.tool == "ask_ipaddress_choice"
            });
        let _ = execute_mcp_tools_flow(
            app.clone(),
            window.clone(),
            final_user_message.clone(),
            tool_calls,
            summaries.clone(),
            recent_ips.clone(),
            history_limit,
            mcp_timeout,
            0,
            is_builder_caller,
        )
        .await;
    }
    else
    {
        // No tools called: perform summarizeAndSave for the initial response.
        let app_c = app.clone();
        let window_c = window.clone();
        let thinking_task_id_c = thinking_task_id.clone();
        let user_message_c = final_user_message.clone();
        let response_c = response.clone();

        tokio::spawn(async move {
            let llama_state_bg = app_c.state::<crate::llm::llm::LlamaState>();
            let content_to_summarize =
                format!("ユーザー入力: {}\n回答: {}", user_message_c, response_c);
            let summary_prompt =
                format!("以下の内容を要約してください。\n\n{}", content_to_summarize);
            if let Ok(summary_text) =
                crate::llm::llm::ask_llm_background(summary_prompt, app_c.clone(), llama_state_bg)
                    .await
            {
                let new_summary = crate::history::SummaryItem {
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    content: summary_text.clone(),
                };
                let _ = crate::history::save_summary(app_c.clone(), new_summary.clone());

                let summary_payload = crate::mcp::protocol::SummarySavedPayload {
                    task_id: thinking_task_id_c,
                    summary_text,
                    summary: new_summary,
                    content: response_c,
                };
                let _ = window_c.emit("chat-event", ChatEvent::McpSummarySaved(summary_payload));
            }
        });
    }

    Ok(())
}
