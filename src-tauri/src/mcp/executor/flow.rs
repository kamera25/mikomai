use serde_json::Value;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, Window};

use super::extract::*;
use super::registry::{get_tool_label, get_tool_registry};
use crate::mcp::protocol::{
    AnalysisStartedPayload, ChatEvent, SummarySavedPayload, ToolFinishedPayload, ToolStartedPayload,
};

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
pub struct ToolCall
{
    pub tool: String,
    pub args: Value,
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteMcpToolPayload
{
    pub task_id: String,
    pub tool_id: String,
    pub tool_label: String,
    pub user_message: String,
    pub args: Value,
    pub summaries: Vec<crate::history::SummaryItem>,
    pub recent_ips: Vec<String>,
    pub history_limit: usize,
    pub mcp_timeout: u64,
}

pub async fn execute_mcp_tool_raw(
    app: AppHandle,
    window: Window,
    task_id: String,
    tool_id: String,
    tool_label: String,
    user_message: String,
    args: Value,
    recent_ips: Vec<String>,
    mcp_timeout: u64,
) -> Result<crate::network::CommandResult, String>
{
    // 1. Normalize arguments by injecting userMessage/user_message
    let mut processed_args = args.clone();
    if let serde_json::Value::Object(ref mut map) = processed_args
    {
        map.insert(
            "userMessage".to_string(),
            serde_json::Value::String(user_message.clone()),
        );
        map.insert(
            "user_message".to_string(),
            serde_json::Value::String(user_message.clone()),
        );
        map.insert(
            "task_id".to_string(),
            serde_json::Value::String(task_id.clone()),
        );
    }

    // 2. Extract resolved host for recentIPs updates in the frontend
    let resolved_host = if ["fetch_config", "fetch_routing", "fetch_arp"]
        .contains(&tool_id.as_str())
    {
        let device_name = get_str_arg(&processed_args, &["deviceName", "device_name"]);
        let device = get_str_arg(&processed_args, &["device"]);
        let host = get_str_arg(&processed_args, &["host"]);

        let resolved = crate::mcp::args::normalize_device_args(
            &app,
            device_name.clone(),
            device_name.clone(),
            device,
            host,
            Some(user_message.clone()),
            Some(user_message.clone()),
        )
        .ok();

        if resolved.as_ref().map_or(true, |r| r.trim().is_empty())
        {
            recent_ips.first().cloned()
        }
        else
        {
            resolved
        }
    }
    else if [
        "self_network_ping",
        "self_network_traceroute",
        "self_network_test_connection",
        "self_network_test_net_connection",
        "network_ftp_download",
        "network_ftp_upload",
        "network_tftp_download",
        "network_tftp_upload",
    ]
    .contains(&tool_id.as_str())
    {
        let host = get_str_arg(&processed_args, &["host"]);
        let device = get_str_arg(&processed_args, &["device"]);
        let device_name = get_str_arg(&processed_args, &["deviceName", "device_name"]);
        let ip = get_str_arg(&processed_args, &["ip"]);

        let resolved = crate::mcp::args::normalize_host_args(
            &app,
            host,
            device,
            device_name.clone(),
            device_name.clone(),
            ip,
        )
        .ok();

        if resolved.as_ref().map_or(true, |r| r.trim().is_empty())
        {
            recent_ips.first().cloned()
        }
        else
        {
            resolved
        }
    }
    else
    {
        None
    };

    // Emit started event
    let start_payload = ToolStartedPayload {
        task_id: task_id.clone(),
        tool_id: tool_id.clone(),
        tool_label: tool_label.clone(),
        args: processed_args.clone(),
        resolved_host: resolved_host.clone(),
    };
    let _ = window.emit("chat-event", ChatEvent::McpToolStarted(start_payload));

    // 3. Match and execute the appropriate command in a future
    let execution_future = async {
        if let Some(tool) = get_tool_registry().get(&tool_id)
        {
            tool.execute(app.clone(), processed_args.clone()).await
        }
        else
        {
            Err(format!("Unknown tool ID: {}", tool_id))
        }
    };

    // Run execution with timeout (bypass timeout for user choice prompts)
    let is_choice_tool = tool_id == "ask_user_choice"
        || tool_id == "ask_interface_choice"
        || tool_id == "ask_ipaddress_choice";
    let result = if is_choice_tool
    {
        match execution_future.await
        {
            Ok(res) => res,
            Err(e) => crate::network::CommandResult {
                success: false,
                output: format!("Execution failed: {}", e),
                saved_path: None,
                is_cached: None,
                cache_time: None,
            },
        }
    }
    else
    {
        let is_heavy_network_tool = [
            "fetch_config",
            "fetch_routing",
            "fetch_arp",
            "network_show",
            "apply_config",
        ]
        .contains(&tool_id.as_str());
        let effective_timeout = if is_heavy_network_tool
        {
            std::cmp::max(mcp_timeout, 120)
        }
        else
        {
            mcp_timeout
        };
        let mcp_timeout_duration = Duration::from_secs(effective_timeout);
        match tokio::time::timeout(mcp_timeout_duration, execution_future).await
        {
            Ok(Ok(res)) => res,
            Ok(Err(e)) => crate::network::CommandResult {
                success: false,
                output: format!("Execution failed: {}", e),
                saved_path: None,
                is_cached: None,
                cache_time: None,
            },
            Err(_) => crate::network::CommandResult {
                success: false,
                output: "MCP execution timed out".to_string(),
                saved_path: None,
                is_cached: None,
                cache_time: None,
            },
        }
    };

    // Emit finished event
    let finished_payload = ToolFinishedPayload {
        task_id: task_id.clone(),
        success: result.success,
        output: result.output.clone(),
        saved_path: result.saved_path.clone(),
        is_cached: result.is_cached,
        cache_time: result.cache_time.clone(),
    };
    let _ = window.emit("chat-event", ChatEvent::McpToolFinished(finished_payload));

    Ok(result)
}

pub fn execute_mcp_tools_flow(
    app: AppHandle,
    window: Window,
    user_message: String,
    tool_calls: Vec<ToolCall>,
    summaries: Vec<crate::history::SummaryItem>,
    recent_ips: Vec<String>,
    history_limit: usize,
    mcp_timeout: u64,
    depth: usize,
    is_builder_caller: bool,
) -> futures::future::BoxFuture<'static, Result<String, String>>
{
    Box::pin(async move {
        if depth >= 5
        {
            return Err("Max nested depth reached".to_string());
        }

        let llama_state = app.state::<crate::llm::llm::LlamaState>();

        // 1. Run all tool executions in parallel
        let mut execution_futures = Vec::new();
        for tc in &tool_calls
        {
            let app_c = app.clone();
            let window_c = window.clone();
            let user_message_c = user_message.clone();
            let tc_tool = tc.tool.clone();
            let tc_label = get_tool_label(&tc_tool);
            let tc_args = tc.args.clone();
            let recent_ips_c = recent_ips.clone();

            let task_id = format!(
                "task_{}_{}",
                chrono::Utc::now().timestamp_millis(),
                uuid::Uuid::new_v4()
                    .to_string()
                    .chars()
                    .take(8)
                    .collect::<String>()
            );

            execution_futures.push(async move {
                let res = execute_mcp_tool_raw(
                    app_c,
                    window_c,
                    task_id,
                    tc_tool.clone(),
                    tc_label.clone(),
                    user_message_c,
                    tc_args,
                    recent_ips_c,
                    mcp_timeout,
                )
                .await;
                (tc_tool, tc_label, res)
            });
        }

        let raw_results = futures::future::join_all(execution_futures).await;

        // Separate successful results
        let mut execution_results = Vec::new();
        for (tool_id, tool_label, res) in raw_results
        {
            match res
            {
                Ok(cmd_res) =>
                {
                    execution_results.push((tool_id, tool_label, cmd_res));
                }
                Err(e) =>
                {
                    execution_results.push((
                        tool_id,
                        tool_label,
                        crate::network::CommandResult {
                            success: false,
                            output: format!("Execution failed: {}", e),
                            saved_path: None,
                            is_cached: None,
                            cache_time: None,
                        },
                    ));
                }
            }
        }

        // 2. Generate a custom label for each tool result, and push choice tools to collected_choices
        let choice_mgr = app.state::<crate::mcp::config_helper::ChoiceManager>();
        let iface_mgr = app.state::<crate::mcp::config_helper::InterfaceChoiceManager>();
        let ip_mgr = app.state::<crate::mcp::config_helper::IpAddressChoiceManager>();
        let pending_choices = choice_mgr.txs.lock().map(|l| l.len()).unwrap_or(0);
        let pending_ifaces = iface_mgr.txs.lock().map(|l| l.len()).unwrap_or(0);
        let pending_ips = ip_mgr.txs.lock().map(|l| l.len()).unwrap_or(0);

        let has_choice_tool = execution_results.iter().any(|(tool_id, _, _)| {
            *tool_id == "ask_user_choice"
                || *tool_id == "ask_interface_choice"
                || *tool_id == "ask_ipaddress_choice"
        });

        // Generate custom labels
        let mut execution_info = Vec::new();
        for (tool_id, tool_label, result) in &execution_results
        {
            let custom_tool_label = if *tool_id == "ask_user_choice"
            {
                let q_msg = get_str_arg(
                    &tool_calls
                        .iter()
                        .find(|t| t.tool == *tool_id)
                        .map(|t| &t.args)
                        .unwrap_or(&Value::Null),
                    &["message"],
                )
                .unwrap_or_default();
                format!("ask_user_choice: {}", q_msg)
            }
            else if *tool_id == "ask_interface_choice"
            {
                let q_msg = get_str_arg(
                    &tool_calls
                        .iter()
                        .find(|t| t.tool == *tool_id)
                        .map(|t| &t.args)
                        .unwrap_or(&Value::Null),
                    &["message"],
                )
                .unwrap_or_default();
                format!("ask_interface_choice: {}", q_msg)
            }
            else if *tool_id == "ask_ipaddress_choice"
            {
                let q_msg = get_str_arg(
                    &tool_calls
                        .iter()
                        .find(|t| t.tool == *tool_id)
                        .map(|t| &t.args)
                        .unwrap_or(&Value::Null),
                    &["message"],
                )
                .unwrap_or_default();
                format!("ask_ipaddress_choice: {}", q_msg)
            }
            else
            {
                tool_label.clone()
            };
            execution_info.push((tool_id.clone(), custom_tool_label, result));
        }

        let mut synthesized_task = None;
        if has_choice_tool && pending_choices == 0 && pending_ifaces == 0 && pending_ips == 0
        {
            let collected_choices = {
                let shared_opt = llama_state.shared.lock().await;
                if let Some(shared) = &*shared_opt
                {
                    let mut builder = shared.builder.lock().unwrap();
                    for (tool_id, custom_label, result) in &execution_info
                    {
                        if (*tool_id == "ask_user_choice"
                            || *tool_id == "ask_interface_choice"
                            || *tool_id == "ask_ipaddress_choice")
                            && result.output.trim() != "cancelled"
                        {
                            builder
                                .collected_choices
                                .push((custom_label.clone(), result.output.clone()));
                        }
                    }
                    builder.collected_choices.clone()
                }
                else
                {
                    Vec::new()
                }
            };

            if !collected_choices.is_empty()
            {
                let answers_block = collected_choices
                    .iter()
                    .map(|(label, val)| {
                        if label.starts_with("ask_user_choice:")
                        {
                            let q_msg = label.strip_prefix("ask_user_choice:").unwrap().trim();
                            format!("- 「{}」の回答: {}", q_msg, val)
                        }
                        else if label.starts_with("ask_interface_choice:")
                        {
                            let q_msg = label.strip_prefix("ask_interface_choice:").unwrap().trim();
                            format!("- 「{}」の回答: {}", q_msg, val)
                        }
                        else if label.starts_with("ask_ipaddress_choice:")
                        {
                            let q_msg = label.strip_prefix("ask_ipaddress_choice:").unwrap().trim();
                            format!("- 「{}」の回答: {}", q_msg, val)
                        }
                        else
                        {
                            format!("- {}: {}", label, val)
                        }
                    })
                    .collect::<Vec<String>>()
                    .join("\n");

                let synthesized_query =
                    format!("{}。追加の確定条件：\n{}", user_message, answers_block);
                log::info!("Synthesized task (template): {}", synthesized_query);
                synthesized_task = Some(synthesized_query);
            }
        }

        // 3. Combine outputs
        let mut combined_output = String::new();
        let mut combined_label_parts = Vec::new();
        let mut has_rag = false;
        for (tool_id, custom_label, result) in &execution_info
        {
            if *tool_id == "query_nw_db"
                || *tool_id == "network_query_nw_db"
                || *tool_id == "query_rag"
            {
                has_rag = true;
            }
            combined_label_parts.push(custom_label.clone());

            if !combined_output.is_empty()
            {
                combined_output.push_str("\n\n");
            }

            let formatted_result_output = if *tool_id == "self_network_nwdiag" && result.success
            {
                "Success: Network diagram generated successfully and saved to artifact.".to_string()
            }
            else
            {
                result.output.clone()
            };

            combined_output.push_str(&format!(
                "【{}の実行結果】:\n{}",
                custom_label, formatted_result_output
            ));
        }

        let combined_tool_label = combined_label_parts.join(", ");
        let history_block = get_history_block_rust(&summaries, history_limit);

        // 4. Analysis phase (comprehensive LLM request)
        let analysis_task_id = format!(
            "task_{}_{}",
            chrono::Utc::now().timestamp_millis(),
            uuid::Uuid::new_v4()
                .to_string()
                .chars()
                .take(8)
                .collect::<String>()
        );

        let first_task_id = format!(
            "task_{}",
            uuid::Uuid::new_v4()
                .to_string()
                .chars()
                .take(8)
                .collect::<String>()
        );

        let analysis_started_payload = AnalysisStartedPayload {
            task_id: first_task_id,
            analysis_task_id: analysis_task_id.clone(),
        };
        let _ = window.emit(
            "chat-event",
            ChatEvent::McpAnalysisStarted(analysis_started_payload),
        );

        let is_builder_context = is_builder_caller
            || execution_info.iter().any(|(tool_id, _, _)| {
                *tool_id == "ask_user_choice"
                    || *tool_id == "ask_interface_choice"
                    || *tool_id == "ask_ipaddress_choice"
                    || *tool_id == "validate_cisco_config"
                    || *tool_id == "convert_cisco_config"
                    || *tool_id == "self_network_nwdiag"
            });

        let analyze_payload = crate::llm::llm::AnalyzePayload {
            user_message: user_message.clone(),
            tool_label: combined_tool_label.clone(),
            output: combined_output,
            is_rag: has_rag,
            is_builder: Some(is_builder_context),
            history_block: Some(history_block),
            subsequent_task: synthesized_task,
        };

        let response_str = crate::llm::llm::analyze_tool_output(
            window.clone(),
            analyze_payload,
            llama_state.clone(),
        )
        .await
        .unwrap_or_else(|e| format!("Analysis failed: {}", e));

        // 5. Generate and save summary
        let mut next_summaries = summaries.clone();
        if response_str == "PENDING_DECISION"
        {
            let summary_payload = SummarySavedPayload {
                task_id: analysis_task_id.clone(),
                summary_text: "".to_string(),
                summary: crate::history::SummaryItem {
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    content: "PENDING_DECISION".to_string(),
                },
                content: response_str.clone(),
            };
            let _ = window.emit("chat-event", ChatEvent::McpSummarySaved(summary_payload));
        }
        else
        {
            let summary_prompt = format!(
                "以下の内容を要約してください。\n\nユーザー入力: {}\n実行ツール: {}\n分析結果: {}",
                user_message, combined_tool_label, response_str
            );
            if let Ok(summary_text) = crate::llm::llm::ask_llm_background(
                summary_prompt,
                app.clone(),
                llama_state.clone(),
            )
            .await
            {
                let new_summary = crate::history::SummaryItem {
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    content: summary_text.clone(),
                };
                let _ = crate::history::save_summary(app.clone(), new_summary.clone());
                next_summaries.push(new_summary.clone());

                let summary_payload = SummarySavedPayload {
                    task_id: analysis_task_id.clone(),
                    summary_text,
                    summary: new_summary,
                    content: response_str.clone(),
                };
                let _ = window.emit("chat-event", ChatEvent::McpSummarySaved(summary_payload));
            }
        }

        // 6. Check for nested tool calls (nested MCP)
        let has_nwdiag = execution_info
            .iter()
            .any(|(tool_id, _, _)| *tool_id == "self_network_nwdiag");
        let max_depth = if has_nwdiag { 3 } else { 5 };

        if is_builder_context && depth < max_depth
        {
            let json_blocks = extract_json_blocks(&response_str);
            let mut nested_tool_calls = Vec::new();
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
                        nested_tool_calls.push(ToolCall { tool: t, args });
                    }
                }
            }

            if !nested_tool_calls.is_empty()
            {
                log::info!(
                    "Executing nested tools comprehensively: {:?}",
                    nested_tool_calls
                        .iter()
                        .map(|t| &t.tool)
                        .collect::<Vec<_>>()
                );
                let nested_response = execute_mcp_tools_flow(
                    app.clone(),
                    window.clone(),
                    user_message.clone(),
                    nested_tool_calls,
                    next_summaries.clone(),
                    recent_ips.clone(),
                    history_limit,
                    mcp_timeout,
                    depth + 1,
                    is_builder_context,
                )
                .await;
                return nested_response;
            }
        }

        Ok(response_str)
    })
}
