pub mod extract;
pub mod flow;
pub mod registry;
pub mod tools;

pub use extract::*;
pub use flow::*;
pub use registry::*;

use tauri::{AppHandle, State, Window};

use crate::mcp::protocol::ChatRequest;


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
) -> Result<(), String>
{
    crate::llm::llm::reset_cancel();

    let ChatRequest {
        user_message,
        summaries: _,
        recent_ips: _,
        history_limit: _,
        mcp_timeout: _,
        attachments,
    } = payload;

    if crate::llm::llm::is_cancelled()
    {
        return Ok(());
    }

    let mut final_user_message = user_message.clone();
    let settings = crate::settings::load_settings(app.clone()).unwrap_or_default();

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
                            att.name,
                            path.display(),
                            att.content
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
                            att.name,
                            path.display(),
                            analysis.extracted_context
                        ));
                    }
                    else
                    {
                        final_user_message.push_str(&format!("\n\n{}", analysis.extracted_context));
                    }
                }
                crate::history::AttachmentType::File =>
                {
                    if let Some(path) = &att.path
                    {
                        let path_str = path.display();
                        final_user_message.push_str(&format!(
                            "\n\n--- 添付ファイル: {} (ローカルパス: {}) ---\n※バイナリまたは大容量ファイルのため内容は省略されています。",
                            att.name, path_str
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

    if crate::llm::llm::is_cancelled()
    {
        return Ok(());
    }

    // Execute via AgentLoop (Network Agent Harness Core)
    let mut agent_loop = crate::harness::agent_loop::AgentLoop::new(app, window, 10);
    agent_loop.run(final_user_message, &*llama_state).await?;

    Ok(())
}

