use crate::llm::worker::{LlmWorker, build_common_worker_prompt};
use crate::llm::llm_manager::AgentContext;
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::llama_backend::LlamaBackend;
use crate::llm::llm::SYSTEM_PROMPT;
use tauri::Manager;

const INVESTIGATE_WORKER_PROMPT: &str = include_str!("../prompts/investigate_worker.txt");

const MAX_NEW_TOKENS: u32 = 256;
const N_CTX: u32 = 4096;

pub struct InvestigateWorker {
    pub ctx: Option<AgentContext<'static>>,
    pub active_vendor: Option<String>,
}

impl InvestigateWorker {
    pub fn new(model: &LlamaModel, backend: &LlamaBackend, preload: bool) -> Result<Self, String> {
        if preload {
            let full_system_prompt = format!(
                "{}\n\n=== Current Role ===\nあなたは現在「Investigator (調査員)」として動作しています。以下の役割指示に特化してください:\n{}",
                SYSTEM_PROMPT,
                INVESTIGATE_WORKER_PROMPT
            );
            let ctx = AgentContext::new(model, backend, &full_system_prompt, 1, MAX_NEW_TOKENS, N_CTX)
                .map_err(|e| format!("Failed to create Investigate context: {:?}", e))?;
            
            let ctx_static = unsafe {
                std::mem::transmute::<AgentContext<'_>, AgentContext<'static>>(ctx)
            };
            
            Ok(Self { ctx: Some(ctx_static), active_vendor: None })
        } else {
            Ok(Self { ctx: None, active_vendor: None })
        }
    }

    pub fn ensure_initialized_with_vendor(
        &mut self,
        model: &LlamaModel,
        backend: &LlamaBackend,
        vendor: Option<String>,
    ) -> Result<(), String> {
        let needs_init = match &self.ctx {
            None => true,
            Some(_) => self.active_vendor != vendor,
        };

        if needs_init {
            self.ctx = None;

            let role_desc = if let Some(ref v) = vendor {
                format!(
                    "あなたは現在「Investigator (調査員)」として動作しています。対象機種: {}\n以下の役割指示に特化してください:\n{}",
                    v,
                    INVESTIGATE_WORKER_PROMPT
                )
            } else {
                format!(
                    "あなたは現在「Investigator (調査員)」として動作しています。以下の役割指示に特化してください:\n{}",
                    INVESTIGATE_WORKER_PROMPT
                )
            };

            let full_system_prompt = format!(
                "{}\n\n=== Current Role ===\n{}",
                SYSTEM_PROMPT,
                role_desc
            );

            let ctx = AgentContext::new(model, backend, &full_system_prompt, 1, MAX_NEW_TOKENS, N_CTX)
                .map_err(|e| format!("Failed to create Investigate context: {:?}", e))?;
            
            let ctx_static = unsafe {
                std::mem::transmute::<AgentContext<'_>, AgentContext<'static>>(ctx)
            };
            self.ctx = Some(ctx_static);
            self.active_vendor = vendor;
        }
        Ok(())
    }
}

impl LlmWorker for InvestigateWorker {
    fn agent_name(&self) -> &'static str {
        "Investigator (調査員)"
    }

    fn context_mut(&mut self) -> &mut AgentContext<'static> {
        self.ctx.as_mut().expect("Investigate context not initialized")
    }

    fn ensure_initialized(
        &mut self,
        model: &LlamaModel,
        backend: &LlamaBackend,
    ) -> Result<(), String> {
        self.ensure_initialized_with_vendor(model, backend, None)
    }

    fn ask(
        &mut self,
        model: &llama_cpp_2::model::LlamaModel,
        backend: &llama_cpp_2::llama_backend::LlamaBackend,
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
        // Detect vendor from prompt or user_message
        let mut vendor = None;
        if let Some(w) = window {
            let app = w.app_handle();
            let text_to_check = match (&prompt, &user_message) {
                (Some(p), _) => p.clone(),
                (_, Some(um)) => um.clone(),
                _ => String::new(),
            };
            if !text_to_check.is_empty() {
                if let Ok(connections) = crate::connections::load_connections_raw(app) {
                    let lower_text = text_to_check.to_lowercase();
                    for conn in connections {
                        let hostname = conn.hostname.as_str().to_lowercase();
                        let ip = conn.ip.to_string();
                        if (!hostname.is_empty() && lower_text.contains(&hostname)) || lower_text.contains(&ip) {
                            if let Some(vt) = conn.vendor_type {
                                let vt_str = vt.as_str().trim();
                                if !vt_str.is_empty() {
                                    vendor = Some(vt_str.to_string());
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        self.ensure_initialized_with_vendor(model, backend, vendor)?;

        let worker_prompt = self.build_prompt(
            prompt,
            user_message,
            tool_label,
            output,
            history_block,
            subsequent_task,
        );
        crate::llm::llm_manager::run_inference(
            self.context_mut(),
            model,
            &worker_prompt,
            window,
            temperature,
            repetition_penalty,
        ).map_err(|e| format!("Worker inference failed: {:?}", e))
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
        build_common_worker_prompt(prompt, user_message, tool_label, output, history_block, subsequent_task)
    }
}
