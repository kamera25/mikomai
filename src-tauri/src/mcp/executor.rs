use tauri::{AppHandle, Emitter, State, Window, Manager};
use serde_json::Value;
use std::time::Duration;
use std::collections::HashMap;
use std::sync::OnceLock;

use crate::mcp::protocol::{
    ChatEvent, ChatRequest, ToolStartedPayload, ToolFinishedPayload,
    AnalysisStartedPayload, SummarySavedPayload, InitialStartedPayload,
    InitialFinishedPayload,
};

// McpTool Trait definition
pub trait McpTool: Send + Sync {
    fn name(&self) -> &'static str;
    fn execute(
        &self,
        app: tauri::AppHandle,
        args: serde_json::Value,
    ) -> futures::future::BoxFuture<'static, Result<crate::network::CommandResult, String>>;
}

// Macros for defining and registering tools
macro_rules! define_tool {
    ($struct_name:ident, $tool_name:expr, |$app:ident, $args:ident| $body:expr) => {
        pub struct $struct_name;
        impl McpTool for $struct_name {
            fn name(&self) -> &'static str {
                $tool_name
            }
            fn execute(
                &self,
                $app: tauri::AppHandle,
                $args: serde_json::Value,
            ) -> futures::future::BoxFuture<'static, Result<crate::network::CommandResult, String>> {
                Box::pin(async move {
                    $body
                })
            }
        }
    };
}

macro_rules! register_tools {
    ($($tool_type:ident),* $(,)?) => {
        {
            let mut registry = HashMap::new();
            $(
                let tool = $tool_type;
                registry.insert(tool.name().to_string(), Box::new(tool) as Box<dyn McpTool>);
            )*
            registry
        }
    };
}

// Helper argument extractors
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

// Define individual tools
define_tool!(PingTool, "self_network_ping", |app, args| {
    let host = get_str_arg(&args, &["host"]);
    let device = get_str_arg(&args, &["device"]);
    let device_name = get_str_arg(&args, &["deviceName", "device_name"]);
    let ip = get_str_arg(&args, &["ip"]);
    let size = get_usize_arg(&args, &["size"]);
    let count = get_u32_arg(&args, &["count"]);
    let df = get_bool_arg(&args, &["df"]);
    crate::mcp::ping::self_network_ping(
        app,
        host,
        device,
        device_name.clone(),
        device_name,
        ip,
        size,
        count,
        df,
    ).await.map(Into::into)
});

define_tool!(TracerouteTool, "self_network_traceroute", |app, args| {
    let host = get_str_arg(&args, &["host"]);
    let device = get_str_arg(&args, &["device"]);
    let device_name = get_str_arg(&args, &["deviceName", "device_name"]);
    let ip = get_str_arg(&args, &["ip"]);
    crate::mcp::traceroute::self_network_traceroute(
        app,
        host,
        device,
        device_name.clone(),
        device_name,
        ip,
    ).await.map(Into::into)
});

define_tool!(FetchConfigTool, "fetch_config", |app, args| {
    let device_name = get_str_arg(&args, &["deviceName", "device_name"]);
    let device = get_str_arg(&args, &["device"]);
    let host = get_str_arg(&args, &["host"]);
    let user_msg = get_str_arg(&args, &["userMessage", "user_message"]);
    crate::mcp::fetch::fetch_config::fetch_config(
        app,
        device_name.clone(),
        device_name,
        device,
        host,
        user_msg.clone(),
        user_msg,
    ).await
});

define_tool!(FetchRoutingTool, "fetch_routing", |app, args| {
    let llama_state = app.state::<crate::llm::llm::LlamaState>();
    let device_name = get_str_arg(&args, &["deviceName", "device_name"]);
    let device = get_str_arg(&args, &["device"]);
    let host = get_str_arg(&args, &["host"]);
    let user_msg = get_str_arg(&args, &["userMessage", "user_message"]);
    crate::mcp::fetch::fetch_routing::fetch_routing(
        app.clone(),
        llama_state,
        device_name.clone(),
        device_name,
        device,
        host,
        user_msg.clone(),
        user_msg,
    ).await
});

define_tool!(FetchArpTool, "fetch_arp", |app, args| {
    let llama_state = app.state::<crate::llm::llm::LlamaState>();
    let device_name = get_str_arg(&args, &["deviceName", "device_name"]);
    let device = get_str_arg(&args, &["device"]);
    let host = get_str_arg(&args, &["host"]);
    let user_msg = get_str_arg(&args, &["userMessage", "user_message"]);
    crate::mcp::fetch::fetch_arp::fetch_arp(
        app.clone(),
        llama_state,
        device_name.clone(),
        device_name,
        device,
        host,
        user_msg.clone(),
        user_msg,
    ).await
});

define_tool!(QueryNwDbTool, "query_nw_db", |app, args| {
    let query = get_str_arg(&args, &["query", "userMessage", "user_message"]).unwrap_or_default();
    let filter = get_str_arg(&args, &["filter"]);
    let rag_state = app.state::<crate::mcp::rag::RagState>();
    crate::mcp::rag::query_nw_db(
        query,
        filter,
        rag_state,
        app.clone(),
    ).await.map(Into::into)
});

define_tool!(QueryNwDbAliasTool, "network_query_nw_db", |app, args| {
    let query = get_str_arg(&args, &["query", "userMessage", "user_message"]).unwrap_or_default();
    let filter = get_str_arg(&args, &["filter"]);
    let rag_state = app.state::<crate::mcp::rag::RagState>();
    crate::mcp::rag::query_nw_db(
        query,
        filter,
        rag_state,
        app.clone(),
    ).await.map(Into::into)
});

define_tool!(QueryRagTool, "query_rag", |app, args| {
    let query = get_str_arg(&args, &["query", "userMessage", "user_message"]).unwrap_or_default();
    let filter = get_str_arg(&args, &["filter"]);
    let rag_state = app.state::<crate::mcp::rag::RagState>();
    crate::mcp::rag::query_nw_db(
        query,
        filter,
        rag_state,
        app.clone(),
    ).await.map(Into::into)
});

define_tool!(SelfNetworkArpTool, "self_network_arp", |app, _args| {
    crate::mcp::arp::self_network_arp(app).await.map(Into::into)
});

define_tool!(SelfNetworkRouteTool, "self_network_route", |app, _args| {
    crate::mcp::route::self_network_route(app).await.map(Into::into)
});

define_tool!(NetworkGetHostsTool, "network_get_hosts", |app, _args| {
    crate::mcp::hosts::network_get_hosts(app).await.map(Into::into)
});

define_tool!(RequireHostRegisteredTool, "require_host_registered", |_app, _args| {
    crate::mcp::hosts::require_host_registered().map(Into::into)
});

define_tool!(NetworkGetIpInfoTool, "network_get_ip_info", |_app, args| {
    let verbose = get_bool_arg(&args, &["verbose"]);
    crate::mcp::ip_info::network_get_ip_info(verbose).await.map(Into::into)
});

define_tool!(NetworkListSerialPortsTool, "network_list_serial_ports", |_app, _args| {
    crate::mcp::console::network_list_serial_ports().map(Into::into)
});

define_tool!(NetworkSendConsoleMessageTool, "network_send_console_message", |_app, args| {
    let port = get_str_arg(&args, &["port"]).unwrap_or_default();
    let baud_rate = get_u32_arg(&args, &["baud_rate", "baudRate"]);
    let message = get_str_arg(&args, &["message"]).unwrap_or_default();
    let timeout_ms = args.get("timeout_ms").or(args.get("timeoutMs")).and_then(|v| v.as_u64());
    crate::mcp::console::network_send_console_message(
        port,
        baud_rate,
        message,
        timeout_ms,
    ).await.map(Into::into)
});

define_tool!(NetworkShowTool, "network_show", |app, args| {
    let device = serde_json::from_value::<crate::network::NetmikoDeviceConfig>(
        args.get("device").cloned().unwrap_or(serde_json::Value::Null)
    ).map_err(|e| e.to_string())?;
    let command = get_str_arg(&args, &["command"]).unwrap_or_default();
    crate::network::network_show(app, device, command).await.map_err(|e| e.to_string())
});

define_tool!(NetworkConfigTool, "network_config", |app, args| {
    let device = serde_json::from_value::<crate::network::NetmikoDeviceConfig>(
        args.get("device").cloned().unwrap_or(serde_json::Value::Null)
    ).map_err(|e| e.to_string())?;
    let commands = serde_json::from_value::<Vec<String>>(
        args.get("commands").cloned().unwrap_or(serde_json::Value::Null)
    ).map_err(|e| e.to_string())?;
    crate::network::network_config(app, device, commands).await.map_err(|e| e.to_string())
});

define_tool!(NwDiagTool, "self_network_nwdiag", |app, args| {
    let schema = get_str_arg(&args, &["schema"]).unwrap_or_default();
    crate::mcp::nwdiag::self_network_nwdiag(app, schema).await
});

define_tool!(ValidateCiscoConfigTool, "validate_cisco_config", |app, args| {
    let id = get_str_arg(&args, &["task_id"]);
    let config = get_str_arg(&args, &["config"]).unwrap_or_default();
    crate::mcp::config_helper::validate_cisco_config_impl(Some(app), id, config).await
});

define_tool!(ConvertCiscoConfigTool, "convert_cisco_config", |_app, args| {
    let config = get_str_arg(&args, &["config"]).unwrap_or_default();
    let target_vendor = get_str_arg(&args, &["target_vendor", "targetVendor"]).unwrap_or_default();
    crate::mcp::config_helper::convert_cisco_config(config, target_vendor).await
});

define_tool!(AskUserChoiceTool, "ask_user_choice", |app, args| {
    let id = get_str_arg(&args, &["task_id"]);
    let title = get_str_arg(&args, &["title"]).unwrap_or_default();
    let message = get_str_arg(&args, &["message"]).unwrap_or_default();
    
    let options: Vec<String> = if let Some(opt_val) = args.get("options") {
        if let Some(arr) = opt_val.as_array() {
            arr.iter().map(|v| v.as_str().unwrap_or("").to_string()).collect()
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    match crate::mcp::config_helper::ask_user_choice(app.clone(), id, title, message, options).await {
        Ok(res) => Ok(crate::network::CommandResult {
            success: true,
            output: res,
            saved_path: None,
            is_cached: None,
            cache_time: None,
        }),
        Err(e) => Err(e),
    }
});

define_tool!(AskInterfaceChoiceTool, "ask_interface_choice", |app, args| {
    let id = get_str_arg(&args, &["task_id"]);
    let vendor = get_str_arg(&args, &["vendor"]).unwrap_or_default();
    let message = get_str_arg(&args, &["message"]);

    match crate::mcp::config_helper::ask_interface_choice(app.clone(), id, vendor, message).await {
        Ok(res) => Ok(crate::network::CommandResult {
            success: true,
            output: res,
            saved_path: None,
            is_cached: None,
            cache_time: None,
        }),
        Err(e) => Err(e),
    }
});

// Tool registry
pub fn get_tool_registry() -> &'static HashMap<String, Box<dyn McpTool>> {
    static REGISTRY: OnceLock<HashMap<String, Box<dyn McpTool>>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        register_tools![
            PingTool,
            TracerouteTool,
            FetchConfigTool,
            FetchRoutingTool,
            FetchArpTool,
            QueryNwDbTool,
            QueryNwDbAliasTool,
            QueryRagTool,
            SelfNetworkArpTool,
            SelfNetworkRouteTool,
            NetworkGetHostsTool,
            RequireHostRegisteredTool,
            NetworkGetIpInfoTool,
            NetworkListSerialPortsTool,
            NetworkSendConsoleMessageTool,
            NetworkShowTool,
            NetworkConfigTool,
            NwDiagTool,
            ValidateCiscoConfigTool,
            ConvertCiscoConfigTool,
            AskUserChoiceTool,
            AskInterfaceChoiceTool,
        ]
    })
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteMcpToolPayload {
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

fn execute_mcp_tool_internal(
    app: AppHandle,
    window: Window,
    payload: ExecuteMcpToolPayload,
    depth: usize,
) -> futures::future::BoxFuture<'static, Result<String, String>> {
    Box::pin(async move {
        let llama_state = app.state::<crate::llm::llm::LlamaState>();
        let ExecuteMcpToolPayload {
            task_id,
            tool_id,
            tool_label,
            user_message,
            args,
            summaries,
            recent_ips,
            history_limit,
            mcp_timeout,
        } = payload.clone();

        // 1. Normalize arguments by injecting userMessage/user_message
        let mut processed_args = args.clone();
        if let serde_json::Value::Object(ref mut map) = processed_args {
            map.insert("userMessage".to_string(), serde_json::Value::String(user_message.clone()));
            map.insert("user_message".to_string(), serde_json::Value::String(user_message.clone()));
            map.insert("task_id".to_string(), serde_json::Value::String(task_id.clone()));
        }

        // 2. Extract resolved host for recentIPs updates in the frontend
        let resolved_host = if ["fetch_config", "fetch_routing", "fetch_arp"].contains(&tool_id.as_str()) {
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
            ).ok();

            // Apply fallback if still empty
            if resolved.as_ref().map_or(true, |r| r.trim().is_empty()) {
                recent_ips.first().cloned()
            } else {
                resolved
            }
        } else if ["self_network_ping", "self_network_traceroute"].contains(&tool_id.as_str()) {
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
                recent_ips.first().cloned()
            } else {
                resolved
            }
        } else {
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
            if let Some(tool) = get_tool_registry().get(&tool_id) {
                tool.execute(app.clone(), processed_args.clone()).await
            } else {
                Err(format!("Unknown tool ID: {}", tool_id))
            }
        };

        // Run execution with timeout (bypass timeout for user choice prompts)
        let is_choice_tool = tool_id == "ask_user_choice" || tool_id == "ask_interface_choice";
        let result = if is_choice_tool {
            match execution_future.await {
                Ok(res) => res,
                Err(e) => crate::network::CommandResult {
                    success: false,
                    output: format!("Execution failed: {}", e),
                    saved_path: None,
                    is_cached: None,
                    cache_time: None,
                },
            }
        } else {
            let mcp_timeout_duration = Duration::from_secs(mcp_timeout);
            match tokio::time::timeout(mcp_timeout_duration, execution_future).await {
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

        // 4. Analysis phase
        let history_block = get_history_block_rust(&summaries, history_limit);
        let analysis_task_id = format!(
            "task_{}_{}",
            chrono::Utc::now().timestamp_millis(),
            uuid::Uuid::new_v4().to_string().chars().take(8).collect::<String>()
        );

        // Emit analysis started event
        let analysis_started_payload = AnalysisStartedPayload {
            task_id: task_id.clone(),
            analysis_task_id: analysis_task_id.clone(),
        };
        let _ = window.emit("chat-event", ChatEvent::McpAnalysisStarted(analysis_started_payload));

        let is_rag = tool_id == "query_nw_db" || tool_id == "network_query_nw_db" || tool_id == "query_rag";
        
        let custom_tool_label = if tool_id == "ask_user_choice" {
            let q_msg = get_str_arg(&processed_args, &["message"]).unwrap_or_default();
            format!("ask_user_choice: {}", q_msg)
        } else if tool_id == "ask_interface_choice" {
            let q_msg = get_str_arg(&processed_args, &["message"]).unwrap_or_default();
            format!("ask_interface_choice: {}", q_msg)
        } else {
            tool_label.clone()
        };

        // 4.1. Check if all choices are answered and synthesize true query
        let choice_mgr = app.state::<crate::mcp::config_helper::ChoiceManager>();
        let iface_mgr = app.state::<crate::mcp::config_helper::InterfaceChoiceManager>();
        let pending_choices = choice_mgr.txs.lock().map(|l| l.len()).unwrap_or(0);
        let pending_ifaces = iface_mgr.txs.lock().map(|l| l.len()).unwrap_or(0);

        let mut synthesized_task = None;
        let is_choice_tool = tool_id == "ask_user_choice" || tool_id == "ask_interface_choice";

        if is_choice_tool && pending_choices == 0 && pending_ifaces == 0 {
            // すべての回答が揃った最後のタイミングで真のユーザーの質問を再生成する
            let mut collected_choices = {
                let shared_opt = llama_state.shared.lock().await;
                if let Some(shared) = &*shared_opt {
                    let builder = shared.builder.lock().unwrap();
                    builder.collected_choices.clone()
                } else {
                    Vec::new()
                }
            };
            
            // 今回の回答もリストに含める
            collected_choices.push((custom_tool_label.clone(), result.output.clone()));

            if !collected_choices.is_empty() {
                let answers_block = collected_choices.iter().map(|(label, val)| {
                    if label.starts_with("ask_user_choice:") {
                        let q_msg = label.strip_prefix("ask_user_choice:").unwrap().trim();
                        format!("- 「{}」の回答: {}", q_msg, val)
                    } else if label.starts_with("ask_interface_choice:") {
                        let q_msg = label.strip_prefix("ask_interface_choice:").unwrap().trim();
                        format!("- 「{}」の回答: {}", q_msg, val)
                    } else {
                        format!("- {}: {}", label, val)
                    }
                }).collect::<Vec<String>>().join("\n");

                let synth_system_prompt = "あなたは優秀なネットワーク要件定義アナリストです。\n元のユーザーの曖昧な質問と、その後に対話によって得られた具体的な回答パラメータを組み合わせて、1つの明確で詳細な「ネットワーク設定要望」の文章（日本語）を再構成してください。\n解説や前置きは一切出力せず、再構成された要望の文章のみを直接出力してください。";
                let synth_prompt = format!("元のユーザーの質問:\n{}\n\n得られた回答リスト:\n{}", user_message, answers_block);
                
                log::info!("Synthesizing true user query from choices (async)...");
                if let Ok(synthesized_query) = crate::llm::llm::ask_llm_internal(
                    &synth_prompt,
                    synth_system_prompt,
                    &app,
                    &llama_state,
                ).await {
                    log::info!("Synthesized Query: {}", synthesized_query);
                    synthesized_task = Some(synthesized_query);
                }
            }
        }

        let analyze_payload = crate::llm::llm::AnalyzePayload {
            user_message: user_message.clone(),
            tool_label: custom_tool_label,
            output: if tool_id == "self_network_nwdiag" && result.success {
                "Success: Network diagram generated successfully and saved to artifact.".to_string()
            } else {
                result.output.clone()
            },
            is_rag,
            history_block: Some(history_block),
            subsequent_task: synthesized_task,
        };

        let response_str = crate::llm::llm::analyze_tool_output(
            window.clone(),
            analyze_payload,
            llama_state.clone(),
        ).await.unwrap_or_else(|e| format!("Analysis failed: {}", e));

        // 5. Generate and save summary
        let mut next_summaries = summaries.clone();
        let summary_prompt = format!(
            "以下の内容を要約してください。\n\nユーザー入力: {}\n実行ツール: {}\n分析結果: {}",
            user_message, tool_label, response_str
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
            next_summaries.push(new_summary.clone());

            // Emit summary saved event
            let summary_payload = SummarySavedPayload {
                task_id: analysis_task_id.clone(),
                summary_text,
                summary: new_summary,
                content: response_str.clone(),
            };
            let _ = window.emit("chat-event", ChatEvent::McpSummarySaved(summary_payload));
        }

        // 6. Check for nested tool calls (nested MCP) for Builder
        let is_builder_context = tool_id == "ask_user_choice"
            || tool_id == "ask_interface_choice"
            || tool_id == "validate_cisco_config"
            || tool_id == "convert_cisco_config";

        if is_builder_context && depth < 5 {
            let json_blocks = extract_json_blocks(&response_str);
            let mut tool_calls = Vec::new();
            for block in json_blocks {
                if let Ok(parsed) = serde_json::from_str::<Value>(&block) {
                    let tool = parsed.get("tool_name")
                        .or_else(|| parsed.get("tool"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    let args = parsed.get("params")
                        .or_else(|| parsed.get("args"))
                        .cloned()
                        .unwrap_or(Value::Object(serde_json::Map::new()));
                    if let Some(t) = tool {
                        tool_calls.push((t, args));
                    }
                }
            }

            if !tool_calls.is_empty() {
                let mut futures = Vec::new();
                for (nested_tool_id, nested_args) in tool_calls {
                    let app_c = app.clone();
                    let window_c = window.clone();
                    
                    let next_task_id = format!(
                        "task_{}_{}",
                        chrono::Utc::now().timestamp_millis(),
                        uuid::Uuid::new_v4().to_string().chars().take(8).collect::<String>()
                    );
                    
                    let next_tool_label = get_tool_label(&nested_tool_id);
                    let user_message_c = user_message.clone();
                    let summaries_c = next_summaries.clone();
                    let recent_ips_c = recent_ips.clone();
                    let history_limit_c = history_limit;
                    let mcp_timeout_c = mcp_timeout;
                    let next_depth = depth + 1;

                    futures.push(async move {
                        let _ = execute_mcp_tool_internal(
                            app_c,
                            window_c,
                            ExecuteMcpToolPayload {
                                task_id: next_task_id,
                                tool_id: nested_tool_id,
                                tool_label: next_tool_label,
                                user_message: user_message_c,
                                args: nested_args,
                                summaries: summaries_c,
                                recent_ips: recent_ips_c,
                                history_limit: history_limit_c,
                                mcp_timeout: mcp_timeout_c,
                            },
                            next_depth,
                        ).await;
                    });
                }
                futures::future::join_all(futures).await;
            }
        }

        Ok(response_str)
    })
}

#[tauri::command]
pub async fn execute_mcp_tool(
    app: AppHandle,
    window: Window,
    _llama_state: State<'_, crate::llm::llm::LlamaState>,
    payload: ExecuteMcpToolPayload,
) -> Result<String, String> {
    execute_mcp_tool_internal(app, window, payload, 0).await
}

fn extract_json_blocks(text: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;
    while i < len {
        if chars[i] == '{' {
            let mut success = false;
            for j in (i + 1..len).rev() {
                if chars[j] == '}' {
                    let candidate: String = chars[i..=j].iter().collect();
                    if serde_json::from_str::<Value>(&candidate).is_ok() {
                        blocks.push(candidate);
                        i = j;
                        success = true;
                        break;
                    }
                }
            }
            if success {
                i += 1;
                continue;
            }
        }
        i += 1;
    }
    blocks
}

fn get_tool_label(tool_name: &str) -> String {
    match tool_name {
        "self_network_ping" => "Ping".to_string(),
        "self_network_traceroute" => "Traceroute".to_string(),
        "network_get_hosts" => "Host List".to_string(),
        "network_query_nw_db" | "query_nw_db" | "query_rag" => "NWDB検索".to_string(),
        "self_network_arp" => "ARP Table".to_string(),
        "self_network_route" => "Route Table".to_string(),
        "network_get_ip_info" => "IP Info".to_string(),
        "network_list_serial_ports" => "Serial Ports".to_string(),
        "network_send_console_message" => "Console Message".to_string(),
        "network_show" => "Show Command".to_string(),
        "fetch_config" => "Fetch Config".to_string(),
        "fetch_routing" => "Fetch Routing".to_string(),
        "fetch_arp" => "Fetch ARP".to_string(),
        "require_host_registered" => "ホスト登録要求".to_string(),
        "self_network_nwdiag" => "ネットワーク図生成".to_string(),
        _ => tool_name.to_string(),
    }
}

#[tauri::command]
pub async fn handle_mcp_message(
    app: AppHandle,
    window: Window,
    llama_state: State<'_, crate::llm::llm::LlamaState>,
    payload: ChatRequest,
) -> Result<(), String> {
    let ChatRequest {
        user_message,
        summaries,
        recent_ips,
        history_limit,
        mcp_timeout,
    } = payload;

    // 1. Generate thinkingTaskId and emit mcp-initial-started
    let thinking_task_id = format!("task_think_{}", chrono::Utc::now().timestamp_millis());
    
    let _ = window.emit("chat-event", ChatEvent::McpInitialStarted(InitialStartedPayload {
        task_id: thinking_task_id.clone(),
    }));

    // 2. Build history block and prompt
    let history_block = get_history_block_rust(&summaries, history_limit);
    let prompt_with_context = format!("【ユーザー入力】\n{}{}", user_message, history_block);

    // 3. Call ask_llm_initial
    let payload_initial = crate::llm::llm::AskInitialPayload {
        prompt: prompt_with_context,
    };
    
    let response = match crate::llm::llm::ask_llm_initial(window.clone(), payload_initial, llama_state.clone()).await {
        Ok(res) => res,
        Err(e) => {
            return Err(e.to_string());
        }
    };

    let _ = window.emit("chat-event", ChatEvent::McpInitialFinished(InitialFinishedPayload {
        task_id: thinking_task_id.clone(),
        content: response.clone(),
    }));

    // 4. Extract and parse tool calls
    let json_blocks = extract_json_blocks(&response);
    
    struct ToolCall {
        tool: String,
        args: Value,
    }
    
    let mut tool_calls = Vec::new();
    for block in json_blocks {
        if let Ok(parsed) = serde_json::from_str::<Value>(&block) {
            let tool = parsed.get("tool_name")
                .or_else(|| parsed.get("tool"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let args = parsed.get("params")
                .or_else(|| parsed.get("args"))
                .cloned()
                .unwrap_or(Value::Object(serde_json::Map::new()));
            if let Some(t) = tool {
                tool_calls.push(ToolCall { tool: t, args });
            }
        }
    }

    if !tool_calls.is_empty() {
        let mut futures = Vec::new();
        for tool_call in tool_calls {
            let app_c = app.clone();
            let window_c = window.clone();
            let llama_state_c = llama_state.clone();
            
            let task_id = format!(
                "task_{}_{}",
                chrono::Utc::now().timestamp_millis(),
                uuid::Uuid::new_v4().to_string().chars().take(8).collect::<String>()
            );
            
            let tool_id = tool_call.tool;
            let tool_label = get_tool_label(&tool_id);
            let user_message_c = user_message.clone();
            let args_c = tool_call.args;
            let summaries_c = summaries.clone();
            let recent_ips_c = recent_ips.clone();
            let history_limit_c = history_limit;
            let mcp_timeout_c = mcp_timeout;

            futures.push(async move {
                let _ = execute_mcp_tool(
                    app_c,
                    window_c,
                    llama_state_c,
                    ExecuteMcpToolPayload {
                        task_id,
                        tool_id,
                        tool_label,
                        user_message: user_message_c,
                        args: args_c,
                        summaries: summaries_c,
                        recent_ips: recent_ips_c,
                        history_limit: history_limit_c,
                        mcp_timeout: mcp_timeout_c,
                    },
                ).await;
            });
        }
        futures::future::join_all(futures).await;
    } else {
        // No tools called: perform summarizeAndSave for the initial response.
        let app_c = app.clone();
        let window_c = window.clone();
        let thinking_task_id_c = thinking_task_id.clone();
        let user_message_c = user_message.clone();
        let response_c = response.clone();

        tokio::spawn(async move {
            let llama_state_bg = app_c.state::<crate::llm::llm::LlamaState>();
            let content_to_summarize = format!("ユーザー入力: {}\n回答: {}", user_message_c, response_c);
            let summary_prompt = format!("以下の内容を要約してください。\n\n{}", content_to_summarize);
            if let Ok(summary_text) = crate::llm::llm::ask_llm_background(
                summary_prompt,
                app_c.clone(),
                llama_state_bg,
            ).await {
                let new_summary = crate::history::SummaryItem {
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    content: summary_text.clone(),
                };
                let _ = crate::history::save_summary(app_c.clone(), new_summary.clone());

                let summary_payload = SummarySavedPayload {
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_extract_json_blocks() {
        let text = "Here is some text with { \"tool\": \"test\" } and another { \"abc\": 123 } block.";
        let blocks = extract_json_blocks(text);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0], "{ \"tool\": \"test\" }");
        assert_eq!(blocks[1], "{ \"abc\": 123 }");

        let text_no_json = "No JSON blocks here.";
        assert!(extract_json_blocks(text_no_json).is_empty());
    }

    #[test]
    fn test_get_tool_label() {
        assert_eq!(get_tool_label("self_network_ping"), "Ping");
        assert_eq!(get_tool_label("network_query_nw_db"), "NWDB検索");
        assert_eq!(get_tool_label("unknown_tool"), "unknown_tool");
    }

    #[test]
    fn test_get_str_arg() {
        let args = json!({
            "host": "192.168.1.1",
            "empty": "   "
        });
        assert_eq!(get_str_arg(&args, &["host"]), Some("192.168.1.1".to_string()));
        assert_eq!(get_str_arg(&args, &["empty", "host"]), Some("192.168.1.1".to_string()));
        assert_eq!(get_str_arg(&args, &["nonexistent"]), None);
    }

    #[test]
    fn test_get_usize_arg() {
        let args = json!({
            "size": 64,
            "size_str": "128"
        });
        assert_eq!(get_usize_arg(&args, &["size"]), Some(64));
        assert_eq!(get_usize_arg(&args, &["size_str"]), Some(128));
        assert_eq!(get_usize_arg(&args, &["nonexistent"]), None);
    }

    #[test]
    fn test_get_u32_arg() {
        let args = json!({
            "count": 5,
            "count_str": "10"
        });
        assert_eq!(get_u32_arg(&args, &["count"]), Some(5));
        assert_eq!(get_u32_arg(&args, &["count_str"]), Some(10));
        assert_eq!(get_u32_arg(&args, &["nonexistent"]), None);
    }

    #[test]
    fn test_get_bool_arg() {
        let args = json!({
            "df_bool": true,
            "df_str_true": "true",
            "df_str_false": "FALSE"
        });
        assert_eq!(get_bool_arg(&args, &["df_bool"]), Some(true));
        assert_eq!(get_bool_arg(&args, &["df_str_true"]), Some(true));
        assert_eq!(get_bool_arg(&args, &["df_str_false"]), Some(false));
        assert_eq!(get_bool_arg(&args, &["nonexistent"]), None);
    }

    #[test]
    fn test_get_history_block_rust() {
        let items = vec![
            crate::history::SummaryItem {
                timestamp: "2023-10-27".to_string(),
                content: "First summary".to_string(),
            },
            crate::history::SummaryItem {
                timestamp: "2023-10-28".to_string(),
                content: "Second summary".to_string(),
            },
        ];
        let block = get_history_block_rust(&items, 2);
        assert!(block.contains("1. Second summary"));
        assert!(block.contains("2. First summary"));

        let empty_block = get_history_block_rust(&items, 0);
        assert_eq!(empty_block, "");
    }
}

