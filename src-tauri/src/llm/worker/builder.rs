use crate::llm::worker::LlmWorker;
use crate::llm::llm_manager::AgentContext;
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::llama_backend::LlamaBackend;
use std::sync::Arc;
use crate::llm::llm::SYSTEM_PROMPT;
use tauri::{Emitter, Manager};

const BUILDER_WORKER_PROMPT: &str = include_str!("../prompts/builder_worker.txt");

const MAX_NEW_TOKENS: u32 = 2048;
const N_CTX: u32 = 4096;

pub struct BuilderWorker {
    pub ctx: Option<AgentContext>,
}

impl BuilderWorker {
    pub fn new(model: &Arc<LlamaModel>, backend: &Arc<LlamaBackend>, preload: bool) -> Result<Self, String> {
        if preload {
            let full_system_prompt = format!(
                "{}\n\n=== Current Role ===\nあなたは現在「Builder (構築者)」として動作しています。以下の役割指示に特化してください:\n{}",
                SYSTEM_PROMPT,
                BUILDER_WORKER_PROMPT
            );
            let ctx = AgentContext::new(model.clone(), backend.clone(), &full_system_prompt, 7, MAX_NEW_TOKENS, N_CTX)
                .map_err(|e| format!("Failed to create Builder context: {:?}", e))?;
            
            Ok(Self { ctx: Some(ctx) })
        } else {
            Ok(Self { ctx: None })
        }
    }
}

impl LlmWorker for BuilderWorker {
    fn agent_name(&self) -> &'static str {
        "Builder (構築者)"
    }

    fn context_mut(&mut self) -> &mut AgentContext {
        self.ctx.as_mut().expect("Builder context not initialized")
    }

    fn ensure_initialized(
        &mut self,
        model: &Arc<LlamaModel>,
        backend: &Arc<LlamaBackend>,
    ) -> Result<(), String> {
        if self.ctx.is_none() {
            let full_system_prompt = format!(
                "{}\n\n=== Current Role ===\nあなたは現在「Builder (構築者)」として動作しています。以下の役割指示に特化してください:\n{}",
                SYSTEM_PROMPT,
                BUILDER_WORKER_PROMPT
            );
            let ctx = AgentContext::new(model.clone(), backend.clone(), &full_system_prompt, 7, MAX_NEW_TOKENS, N_CTX)
                .map_err(|e| format!("Failed to create Builder context: {:?}", e))?;
            
            self.ctx = Some(ctx);
        }
        Ok(())
    }

    fn max_new_tokens(&self) -> u32 {
        MAX_NEW_TOKENS
    }

    fn ask(
        &mut self,
        model: &Arc<LlamaModel>,
        backend: &Arc<LlamaBackend>,
        prompt: Option<String>,
        user_message: Option<String>,
        tool_label: Option<String>,
        output: Option<String>,
        history_block: Option<String>,
        subsequent_task: Option<&str>,
        window: Option<&tauri::Window>,
        temperature: f32,
        repetition_penalty: f32,
    ) -> Result<String, String> {
        self.ensure_initialized(model, backend)?;

        // 1. Detect if this is a VLAN configuration request and ask user if needed
        let query_lower = user_message.as_ref().map(|s| s.to_lowercase()).unwrap_or_default();
        let prompt_lower = prompt.as_ref().map(|s| s.to_lowercase()).unwrap_or_default();
        let is_vlan_request = (query_lower.contains("vlan") && (query_lower.contains("追加") || query_lower.contains("add") || query_lower.contains("作成") || query_lower.contains("create") || query_lower.contains("設定") || query_lower.contains("config")))
            || (prompt_lower.contains("vlan") && (prompt_lower.contains("追加") || prompt_lower.contains("add") || prompt_lower.contains("作成") || prompt_lower.contains("create") || prompt_lower.contains("設定") || prompt_lower.contains("config")));

        let mut user_choice = None;
        if is_vlan_request {
            if let Some(w) = window {
                let app = w.app_handle();
                let choice_manager = app.state::<crate::mcp::config_helper::ChoiceManager>();
                
                let (tx, rx) = tokio::sync::oneshot::channel();
                {
                    let mut lock = choice_manager.tx.lock().unwrap();
                    *lock = Some(tx);
                }

                // Emit event to request user choice
                let payload = serde_json::json!({
                    "title": "VLAN接続モードの選択",
                    "message": "追加するVLANポートの接続モードを選択してください：",
                    "options": [
                        "Access Mode (エンドデバイス/PCを接続する場合)",
                        "Trunk Mode (他のスイッチやルータとトランキングする場合)"
                    ]
                });
                let _ = w.emit("request-user-choice", payload);

                // Wait for frontend response
                let rt = tauri::async_runtime::handle();
                let choice = rt.block_on(async {
                    match rx.await {
                        Ok(c) => c,
                        Err(_) => "cancelled".to_string(),
                    }
                });

                if choice != "cancelled" {
                    user_choice = Some(choice);
                }
            }
        }

        // 2. Generate Cisco config text using standard LLM inference
        let mut worker_prompt = self.build_prompt(
            prompt.clone(),
            user_message.clone(),
            tool_label.clone(),
            output.clone(),
            history_block.clone(),
            subsequent_task,
        );

        if let Some(ref choice) = user_choice {
            worker_prompt.push_str(&format!(
                "\n\n[重要 - ユーザー選択結果]: ユーザーはVLANのポートタイプとして「{}」を選択しました。このモードに合致した正しいスイッチポート設定（Accessの場合は switchport mode access / switchport access vlan X、Trunkの場合は switchport mode trunk / switchport trunk allowed vlan add X など）を出力するConfigに必ず含めて作成してください。",
                choice
            ));
        }

        let initial_response = crate::llm::llm_manager::run_inference(
            self.context_mut(),
            &worker_prompt,
            window,
            temperature,
            repetition_penalty,
        ).map_err(|e| format!("Worker inference failed: {:?}", e))?;

        // 2. Extract Cisco configuration block from the response
        let config_to_validate = extract_config_block(&initial_response);

        if let Some(config) = config_to_validate {
            if let Some(w) = window {
                let rt = tauri::async_runtime::handle();

                // Step A: Validate Cisco Config
                let val_task_id = format!("task_val_{}", uuid::Uuid::new_v4());
                let start_payload_val = crate::mcp::protocol::ChatEvent::McpToolStarted(
                    crate::mcp::protocol::ToolStartedPayload {
                        task_id: val_task_id.clone(),
                        tool_id: "validate_cisco_config".to_string(),
                        tool_label: "validate_cisco_config".to_string(),
                        args: serde_json::json!({ "config": config }),
                        resolved_host: None,
                    }
                );
                let _ = w.emit("chat-event", start_payload_val);

                // Run validate_cisco_config
                let val_res = rt.block_on(async {
                    crate::mcp::config_helper::validate_cisco_config(config.clone()).await
                });

                let (val_success, val_output) = match val_res {
                    Ok(res) => (res.success, res.output),
                    Err(e) => (false, format!("Validation error: {}", e)),
                };

                let finish_payload_val = crate::mcp::protocol::ChatEvent::McpToolFinished(
                    crate::mcp::protocol::ToolFinishedPayload {
                        task_id: val_task_id,
                        success: val_success,
                        output: val_output,
                        saved_path: None,
                        is_cached: None,
                        cache_time: None,
                    }
                );
                let _ = w.emit("chat-event", finish_payload_val);

                // Step B: Convert Cisco Config (to Juniper and Arista)
                let user_msg_lower = user_message.as_ref().map(|s| s.to_lowercase()).unwrap_or_default();
                let prompt_lower = prompt.as_ref().map(|s| s.to_lowercase()).unwrap_or_default();
                
                let target_vendors = if user_msg_lower.contains("arista") || prompt_lower.contains("arista") {
                    vec!["arista".to_string()]
                } else if user_msg_lower.contains("juniper") || prompt_lower.contains("juniper") {
                    vec!["juniper".to_string()]
                } else {
                    vec!["juniper".to_string(), "arista".to_string()]
                };

                for vendor in target_vendors {
                    let conv_task_id = format!("task_conv_{}", uuid::Uuid::new_v4());
                    let start_payload_conv = crate::mcp::protocol::ChatEvent::McpToolStarted(
                        crate::mcp::protocol::ToolStartedPayload {
                            task_id: conv_task_id.clone(),
                            tool_id: "convert_cisco_config".to_string(),
                            tool_label: format!("convert_cisco_config ({})", vendor),
                            args: serde_json::json!({ "config": config.clone(), "target_vendor": vendor.clone() }),
                            resolved_host: None,
                        }
                    );
                    let _ = w.emit("chat-event", start_payload_conv);

                    // Run convert_cisco_config
                    let conv_res = rt.block_on(async {
                        crate::mcp::config_helper::convert_cisco_config(config.clone(), vendor.clone()).await
                    });

                    let (conv_success, conv_output) = match conv_res {
                        Ok(res) => (res.success, res.output),
                        Err(e) => (false, format!("Conversion error: {}", e)),
                    };

                    let finish_payload_conv = crate::mcp::protocol::ChatEvent::McpToolFinished(
                        crate::mcp::protocol::ToolFinishedPayload {
                            task_id: conv_task_id,
                            success: conv_success,
                            output: conv_output,
                            saved_path: None,
                            is_cached: None,
                            cache_time: None,
                        }
                    );
                    let _ = w.emit("chat-event", finish_payload_conv);
                }
            }
        }

        Ok(initial_response)
    }

    fn build_prompt(
        &self,
        prompt: Option<String>,
        user_message: Option<String>,
        tool_label: Option<String>,
        output: Option<String>,
        history_block: Option<String>,
        subsequent_task: Option<&str>,
    ) -> String {
        crate::llm::worker::build_common_worker_prompt(prompt, user_message, tool_label, output, history_block, subsequent_task)
    }
}

fn extract_config_block(text: &str) -> Option<String> {
    if let Some(start_idx) = text.find("```") {
        let rest = &text[start_idx + 3..];
        let content_start = if let Some(newline_idx) = rest.find('\n') {
            let lang = rest[..newline_idx].trim();
            if lang == "cisco" || lang == "ios" || lang == "config" || lang.is_empty() {
                start_idx + 3 + newline_idx + 1
            } else {
                start_idx + 3
            }
        } else {
            start_idx + 3
        };
        
        let remaining_text = &text[content_start..];
        if let Some(end_idx) = remaining_text.find("```") {
            return Some(remaining_text[..end_idx].trim().to_string());
        }
    }
    
    if text.contains("interface") || text.contains("hostname") || text.contains("vlan ") {
        return Some(text.trim().to_string());
    }
    
    None
}
