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
    pub collected_choices: Vec<(String, String)>,
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
            
            Ok(Self { ctx: Some(ctx), collected_choices: Vec::new() })
        } else {
            Ok(Self { ctx: None, collected_choices: Vec::new() })
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

        if prompt.is_some() {
            self.collected_choices.clear();
        }

        if let (Some(label), Some(out)) = (&tool_label, &output) {
            if label.contains("ask_user_choice") || label.contains("ask_interface_choice") {
                self.collected_choices.push((label.clone(), out.clone()));
            }
        }

        // 未回答の質問があるかチェック
        let has_pending_choices = if let Some(w) = window {
            use tauri::Manager;
            let app = w.app_handle();
            let choice_mgr = app.state::<crate::mcp::config_helper::ChoiceManager>();
            let iface_mgr = app.state::<crate::mcp::config_helper::InterfaceChoiceManager>();
            
            let pending_choices = choice_mgr.txs.lock().map(|l| l.len()).unwrap_or(0);
            let pending_ifaces = iface_mgr.txs.lock().map(|l| l.len()).unwrap_or(0);
            
            pending_choices > 0 || pending_ifaces > 0
        } else {
            false
        };

        if has_pending_choices {
            log::info!("BuilderWorker: Other pending choices exist. Skipping inference.");
            return Ok("他の質問への回答を待っています...".to_string());
        }

        let modified_user_message = if !self.collected_choices.is_empty() {
            if let Some(ref original_msg) = user_message {
                let mut msg = original_msg.clone();
                msg.push_str("\n\n【ユーザーの追加回答（これまでの選択情報）】");
                for (label, val) in &self.collected_choices {
                    if label.starts_with("ask_user_choice:") {
                        let q_msg = label.strip_prefix("ask_user_choice:").unwrap().trim();
                        msg.push_str(&format!("\n- 「{}」のユーザ回答 : {}", q_msg, val));
                    } else if label.starts_with("ask_interface_choice:") {
                        let q_msg = label.strip_prefix("ask_interface_choice:").unwrap().trim();
                        msg.push_str(&format!("\n- 「{}」のユーザ回答 : {}", q_msg, val));
                    } else {
                        let type_name = if label.contains("ask_interface_choice") {
                            "選択されたインターフェース"
                        } else {
                            "選択された回答"
                        };
                        msg.push_str(&format!("\n- {}: {}", type_name, val));
                    }
                }
                Some(msg)
            } else {
                user_message.clone()
            }
        } else {
            user_message.clone()
        };

        log::info!("BuilderWorker: subsequent_task to be sent: {:?}", subsequent_task);

        // 1. Generate config text using standard LLM inference
        let worker_prompt = self.build_prompt(
            prompt.clone(),
            modified_user_message,
            tool_label.clone(),
            output.clone(),
            history_block.clone(),
            subsequent_task,
        );

        let initial_response = crate::llm::llm_manager::run_inference(
            self.context_mut(),
            &worker_prompt,
            window,
            temperature,
            repetition_penalty,
        ).map_err(|e| format!("Worker inference failed: {:?}", e))?;

        // 2. If the response contains a tool call, return it directly so the chat controller executes it
        if initial_response.contains("\"tool_name\":") || initial_response.contains("\"tool\":") {
            return Ok(initial_response);
        }

        // 3. Extract Cisco configuration block from the response
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
                    use tauri::Manager;
                    crate::mcp::config_helper::validate_cisco_config_impl(Some(w.app_handle().clone()), Some(val_task_id.clone()), config.clone()).await
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
                let mut is_cisco_ios = true;
                let app_handle = w.app_handle();
                if let Some(host) = crate::settings::load_settings(app_handle.clone())
                    .ok()
                    .and_then(|settings| settings.recent_ips.first().cloned())
                {
                    if let Ok(connections) = crate::connections::load_connections_raw(app_handle) {
                        if let Some(conn) = connections.iter().find(|c| c.hostname.eq_ignore_ascii_case(&host) || c.ip.to_string() == host) {
                            let device_type_str = conn.device_type.as_ref().map(|d| d.as_str().to_lowercase()).unwrap_or_default();
                            let vendor_type_str = conn.vendor_type.as_ref().map(|v| v.as_str().to_lowercase()).unwrap_or_default();
                            let is_cisco = device_type_str == "cisco_ios"
                                || vendor_type_str == "cisco_ios"
                                || device_type_str.contains("cisco")
                                || vendor_type_str.contains("cisco");
                            is_cisco_ios = is_cisco;
                        }
                    }
                }

                if !is_cisco_ios {
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
