use tauri::{AppHandle, Emitter, State, Window};
use serde_json::Value;
use std::time::Duration;

#[derive(serde::Serialize, Clone)]
struct ToolStartedPayload {
    #[serde(rename = "taskId")]
    task_id: String,
    #[serde(rename = "toolId")]
    tool_id: String,
    #[serde(rename = "toolLabel")]
    tool_label: String,
    args: Value,
    #[serde(rename = "resolvedHost")]
    resolved_host: Option<String>,
}

#[derive(serde::Serialize, Clone)]
struct ToolFinishedPayload {
    #[serde(rename = "taskId")]
    task_id: String,
    success: bool,
    output: String,
    #[serde(rename = "savedPath")]
    saved_path: Option<String>,
    #[serde(rename = "isCached")]
    is_cached: Option<bool>,
    #[serde(rename = "cacheTime")]
    cache_time: Option<String>,
}

#[derive(serde::Serialize, Clone)]
struct AnalysisStartedPayload {
    #[serde(rename = "taskId")]
    task_id: String,
    #[serde(rename = "analysisTaskId")]
    analysis_task_id: String,
}

#[derive(serde::Serialize, Clone)]
struct SummarySavedPayload {
    #[serde(rename = "taskId")]
    task_id: String,
    #[serde(rename = "summaryText")]
    summary_text: String,
    summary: crate::history::SummaryItem,
    content: String,
}

fn get_str_arg(args: &Value, keys: &[&str]) -> Option<String> {
    for &key in keys {
        if let Some(val) = args.get(key) {
            if let Some(s) = val.as_str() {
                if !s.trim().is_empty() {
                    return Some(s.to_string());
                }
            }
        }
    }
    None
}

fn get_usize_arg(args: &Value, keys: &[&str]) -> Option<usize> {
    for &key in keys {
        if let Some(val) = args.get(key) {
            if let Some(n) = val.as_u64() {
                return Some(n as usize);
            }
            if let Some(s) = val.as_str() {
                if let Ok(n) = s.parse::<usize>() {
                    return Some(n);
                }
            }
        }
    }
    None
}

fn get_u32_arg(args: &Value, keys: &[&str]) -> Option<u32> {
    for &key in keys {
        if let Some(val) = args.get(key) {
            if let Some(n) = val.as_u64() {
                return Some(n as u32);
            }
            if let Some(s) = val.as_str() {
                if let Ok(n) = s.parse::<u32>() {
                    return Some(n);
                }
            }
        }
    }
    None
}

fn get_bool_arg(args: &Value, keys: &[&str]) -> Option<bool> {
    for &key in keys {
        if let Some(val) = args.get(key) {
            if let Some(b) = val.as_bool() {
                return Some(b);
            }
            if let Some(s) = val.as_str() {
                if s.eq_ignore_ascii_case("true") {
                    return Some(true);
                }
                if s.eq_ignore_ascii_case("false") {
                    return Some(false);
                }
            }
        }
    }
    None
}

fn get_history_block_rust(items: &[crate::history::SummaryItem], limit: usize) -> String {
    if limit == 0 || items.is_empty() {
        return "".to_string();
    }
    let mut recent: Vec<crate::history::SummaryItem> = items.to_vec();
    recent.reverse();
    let limit_len = std::cmp::min(limit, recent.len());
    let recent_slice = &recent[0..limit_len];
    
    let mut text = String::new();
    for (i, item) in recent_slice.iter().enumerate() {
        if i > 0 {
            text.push('\n');
        }
        text.push_str(&format!("{}. {}", i + 1, item.content));
    }
    format!("\n\n<memory>\n{}\n</memory>", text)
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn execute_mcp_tool(
    app: AppHandle,
    window: Window,
    llama_state: State<'_, crate::llm::llm::LlamaState>,
    rag_state: State<'_, crate::mcp::rag::RagState>,
    taskId: String,
    toolId: String,
    toolLabel: String,
    userMessage: String,
    args: Value,
    summaries: Vec<crate::history::SummaryItem>,
    recentIps: Vec<String>,
    historyLimit: usize,
    mcpTimeout: u64,
) -> Result<String, String> {
    // 1. Normalize arguments by injecting userMessage/user_message
    let mut processed_args = args.clone();
    if let serde_json::Value::Object(ref mut map) = processed_args {
        map.insert("userMessage".to_string(), serde_json::Value::String(userMessage.clone()));
        map.insert("user_message".to_string(), serde_json::Value::String(userMessage.clone()));
    }

    // 2. Extract resolved host for recentIPs updates in the frontend
    let resolved_host = if ["fetch_config", "fetch_routing", "fetch_arp"].contains(&toolId.as_str()) {
        let device_name = get_str_arg(&processed_args, &["deviceName", "device_name"]);
        let device = get_str_arg(&processed_args, &["device"]);
        let host = get_str_arg(&processed_args, &["host"]);
        
        let resolved = crate::mcp::args::normalize_device_args(
            &app,
            device_name.clone(),
            device_name.clone(),
            device,
            host,
            Some(userMessage.clone()),
            Some(userMessage.clone()),
        ).ok();

        // Apply fallback if still empty
        if resolved.as_ref().map_or(true, |r| r.trim().is_empty()) {
            recentIps.first().cloned()
        } else {
            resolved
        }
    } else if ["self_network_ping", "self_network_traceroute"].contains(&toolId.as_str()) {
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
        ).ok();

        // Apply fallback if still empty
        if resolved.as_ref().map_or(true, |r| r.trim().is_empty()) {
            recentIps.first().cloned()
        } else {
            resolved
        }
    } else {
        None
    };

    // Emit started event
    let start_payload = ToolStartedPayload {
        task_id: taskId.clone(),
        tool_id: toolId.clone(),
        tool_label: toolLabel.clone(),
        args: processed_args.clone(),
        resolved_host: resolved_host.clone(),
    };
    let _ = window.emit("mcp-tool-started", start_payload);

    // 3. Match and execute the appropriate command in a future
    let execution_future = async {
        match toolId.as_str() {
            "self_network_ping" => {
                let host = get_str_arg(&processed_args, &["host"]);
                let device = get_str_arg(&processed_args, &["device"]);
                let device_name = get_str_arg(&processed_args, &["deviceName", "device_name"]);
                let ip = get_str_arg(&processed_args, &["ip"]);
                let size = get_usize_arg(&processed_args, &["size"]);
                let count = get_u32_arg(&processed_args, &["count"]);
                let df = get_bool_arg(&processed_args, &["df"]);
                match crate::mcp::ping::self_network_ping(
                    app.clone(),
                    host,
                    device,
                    device_name.clone(),
                    device_name,
                    ip,
                    size,
                    count,
                    df,
                ).await {
                    Ok(res) => Ok(crate::network::CommandResult {
                        success: res.success,
                        output: res.output,
                        saved_path: None,
                        is_cached: None,
                        cache_time: None,
                    }),
                    Err(e) => Err(e),
                }
            }
            "self_network_traceroute" => {
                let host = get_str_arg(&processed_args, &["host"]);
                let device = get_str_arg(&processed_args, &["device"]);
                let device_name = get_str_arg(&processed_args, &["deviceName", "device_name"]);
                let ip = get_str_arg(&processed_args, &["ip"]);
                match crate::mcp::traceroute::self_network_traceroute(
                    app.clone(),
                    host,
                    device,
                    device_name.clone(),
                    device_name,
                    ip,
                ).await {
                    Ok(res) => Ok(crate::network::CommandResult {
                        success: res.success,
                        output: res.output,
                        saved_path: None,
                        is_cached: None,
                        cache_time: None,
                    }),
                    Err(e) => Err(e),
                }
            }
            "fetch_config" => {
                let device_name = get_str_arg(&processed_args, &["deviceName", "device_name"]);
                let device = get_str_arg(&processed_args, &["device"]);
                let host = get_str_arg(&processed_args, &["host"]);
                let user_msg = get_str_arg(&processed_args, &["userMessage", "user_message"]);
                crate::mcp::fetch::fetch_config::fetch_config(
                    app.clone(),
                    device_name.clone(),
                    device_name,
                    device,
                    host,
                    user_msg.clone(),
                    user_msg,
                ).await
            }
            "fetch_routing" => {
                let device_name = get_str_arg(&processed_args, &["deviceName", "device_name"]);
                let device = get_str_arg(&processed_args, &["device"]);
                let host = get_str_arg(&processed_args, &["host"]);
                let user_msg = get_str_arg(&processed_args, &["userMessage", "user_message"]);
                crate::mcp::fetch::fetch_routing::fetch_routing(
                    app.clone(),
                    llama_state.clone(),
                    device_name.clone(),
                    device_name,
                    device,
                    host,
                    user_msg.clone(),
                    user_msg,
                ).await
            }
            "fetch_arp" => {
                let device_name = get_str_arg(&processed_args, &["deviceName", "device_name"]);
                let device = get_str_arg(&processed_args, &["device"]);
                let host = get_str_arg(&processed_args, &["host"]);
                let user_msg = get_str_arg(&processed_args, &["userMessage", "user_message"]);
                crate::mcp::fetch::fetch_arp::fetch_arp(
                    app.clone(),
                    llama_state.clone(),
                    device_name.clone(),
                    device_name,
                    device,
                    host,
                    user_msg.clone(),
                    user_msg,
                ).await
            }
            "query_nw_db" | "network_query_nw_db" => {
                let query = get_str_arg(&processed_args, &["query", "userMessage", "user_message"]).unwrap_or_default();
                let filter = get_str_arg(&processed_args, &["filter"]);
                match crate::mcp::rag::query_nw_db(
                    query,
                    filter,
                    rag_state.clone(),
                    app.clone(),
                ).await {
                    Ok(res) => Ok(crate::network::CommandResult {
                        success: res.success,
                        output: res.output,
                        saved_path: None,
                        is_cached: None,
                        cache_time: None,
                    }),
                    Err(e) => Err(e),
                }
            }
            "self_network_arp" => {
                match crate::mcp::arp::self_network_arp(app.clone()).await {
                    Ok(res) => Ok(crate::network::CommandResult {
                        success: res.success,
                        output: res.output,
                        saved_path: res.saved_path,
                        is_cached: None,
                        cache_time: None,
                    }),
                    Err(e) => Err(e),
                }
            }
            "self_network_route" => {
                match crate::mcp::route::self_network_route(app.clone()).await {
                    Ok(res) => Ok(crate::network::CommandResult {
                        success: res.success,
                        output: res.output,
                        saved_path: res.saved_path,
                        is_cached: None,
                        cache_time: None,
                    }),
                    Err(e) => Err(e),
                }
            }
            "network_get_hosts" => {
                match crate::mcp::hosts::network_get_hosts(app.clone()).await {
                    Ok(res) => Ok(crate::network::CommandResult {
                        success: res.success,
                        output: res.output,
                        saved_path: None,
                        is_cached: None,
                        cache_time: None,
                    }),
                    Err(e) => Err(e),
                }
            }
            "require_host_registered" => {
                match crate::mcp::hosts::require_host_registered() {
                    Ok(res) => Ok(crate::network::CommandResult {
                        success: res.success,
                        output: res.output,
                        saved_path: None,
                        is_cached: None,
                        cache_time: None,
                    }),
                    Err(e) => Err(e),
                }
            }
            "network_get_ip_info" => {
                let verbose = get_bool_arg(&processed_args, &["verbose"]);
                match crate::mcp::ip_info::network_get_ip_info(verbose).await {
                    Ok(res) => Ok(crate::network::CommandResult {
                        success: res.success,
                        output: res.output,
                        saved_path: None,
                        is_cached: None,
                        cache_time: None,
                    }),
                    Err(e) => Err(e),
                }
            }
            "network_list_serial_ports" => {
                match crate::mcp::console::network_list_serial_ports() {
                    Ok(res) => Ok(crate::network::CommandResult {
                        success: res.success,
                        output: res.output,
                        saved_path: None,
                        is_cached: None,
                        cache_time: None,
                    }),
                    Err(e) => Err(e),
                }
            }
            "network_send_console_message" => {
                let port = get_str_arg(&processed_args, &["port"]).unwrap_or_default();
                let baud_rate = get_u32_arg(&processed_args, &["baud_rate", "baudRate"]);
                let message = get_str_arg(&processed_args, &["message"]).unwrap_or_default();
                let timeout_ms = args.get("timeout_ms").or(args.get("timeoutMs")).and_then(|v| v.as_u64());
                match crate::mcp::console::network_send_console_message(
                    port,
                    baud_rate,
                    message,
                    timeout_ms,
                ).await {
                    Ok(res) => Ok(crate::network::CommandResult {
                        success: res.success,
                        output: res.output,
                        saved_path: None,
                        is_cached: None,
                        cache_time: None,
                    }),
                    Err(e) => Err(e),
                }
            }
            "network_show" => {
                let device = serde_json::from_value::<crate::network::NetmikoDeviceConfig>(
                    processed_args.get("device").cloned().unwrap_or(serde_json::Value::Null)
                ).map_err(|e| e.to_string())?;
                let command = get_str_arg(&processed_args, &["command"]).unwrap_or_default();
                crate::network::network_show(app.clone(), device, command).await
            }
            "network_config" => {
                let device = serde_json::from_value::<crate::network::NetmikoDeviceConfig>(
                    processed_args.get("device").cloned().unwrap_or(serde_json::Value::Null)
                ).map_err(|e| e.to_string())?;
                let commands = serde_json::from_value::<Vec<String>>(
                    processed_args.get("commands").cloned().unwrap_or(serde_json::Value::Null)
                ).map_err(|e| e.to_string())?;
                crate::network::network_config(app.clone(), device, commands).await
            }
            _ => Err(format!("Unknown tool ID: {}", toolId)),
        }
    };

    // Run execution with timeout
    let mcp_timeout_duration = Duration::from_secs(mcpTimeout);
    let result = match tokio::time::timeout(mcp_timeout_duration, execution_future).await {
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
    };

    // Emit finished event
    let finished_payload = ToolFinishedPayload {
        task_id: taskId.clone(),
        success: result.success,
        output: result.output.clone(),
        saved_path: result.saved_path.clone(),
        is_cached: result.is_cached,
        cache_time: result.cache_time.clone(),
    };
    let _ = window.emit("mcp-tool-finished", finished_payload);

    // 4. Analysis phase
    let history_block = get_history_block_rust(&summaries, historyLimit);
    let analysis_task_id = format!(
        "task_{}_{}",
        chrono::Utc::now().timestamp_millis(),
        uuid::Uuid::new_v4().to_string().chars().take(8).collect::<String>()
    );

    // Emit analysis started event
    let analysis_started_payload = AnalysisStartedPayload {
        task_id: taskId.clone(),
        analysis_task_id: analysis_task_id.clone(),
    };
    let _ = window.emit("mcp-analysis-started", analysis_started_payload);

    let is_rag = toolId == "query_nw_db" || toolId == "network_query_nw_db";
    let analyze_payload = crate::llm::llm::AnalyzePayload {
        user_message: userMessage.clone(),
        tool_label: toolLabel.clone(),
        output: result.output.clone(),
        is_rag,
        history_block: Some(history_block),
    };

    let response_str = crate::llm::llm::analyze_tool_output(
        window.clone(),
        analyze_payload,
        llama_state.clone(),
    ).await.unwrap_or_else(|e| format!("Analysis failed: {}", e));

    // 5. Generate and save summary
    let summary_prompt = format!(
        "以下の内容を要約してください。\n\nユーザー入力: {}\n実行ツール: {}\n分析結果: {}",
        userMessage, toolLabel, response_str
    );
    if let Ok(summary_text) = crate::llm::llm::ask_llm_background(
        summary_prompt,
        app.clone(),
        llama_state.clone(),
    ).await {
        let new_summary = crate::history::SummaryItem {
            timestamp: chrono::Utc::now().to_rfc3339(),
            content: summary_text.clone(),
        };
        let _ = crate::history::save_summary(app.clone(), new_summary.clone());

        // Emit summary saved event
        let summary_payload = SummarySavedPayload {
            task_id: analysis_task_id.clone(),
            summary_text,
            summary: new_summary,
            content: response_str.clone(),
        };
        let _ = window.emit("mcp-summary-saved", summary_payload);
    }

    Ok(response_str)
}
