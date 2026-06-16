use crate::llm::worker::{LlmWorker, build_common_worker_prompt};
use crate::llm::llm_manager::AgentContext;
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::llama_backend::LlamaBackend;
use crate::llm::llm::SYSTEM_PROMPT;
use tauri::Manager;

const KNOWLEDGE_WORKER_PROMPT: &str = include_str!("../prompts/knowledge_worker.txt");

const MAX_NEW_TOKENS: u32 = 2048;
const N_CTX: u32 = 4096;

pub struct KnowledgeWorker {
    pub ctx: Option<AgentContext<'static>>,
    pub active_vendor: Option<String>,
}

impl KnowledgeWorker {
    pub fn new(model: &LlamaModel, backend: &LlamaBackend, preload: bool) -> Result<Self, String> {
        if preload {
            let full_system_prompt = format!(
                "{}\n\n=== Current Role ===\nあなたは現在「Knowledge Expert (知識専門家)」として動作しています。以下の役割指示に特化してください:\n{}",
                SYSTEM_PROMPT,
                KNOWLEDGE_WORKER_PROMPT
            );
            let ctx = AgentContext::new(model, backend, &full_system_prompt, 2, MAX_NEW_TOKENS, N_CTX)
                .map_err(|e| format!("Failed to create Knowledge context: {:?}", e))?;
            
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
        _vendor: Option<String>,
    ) -> Result<(), String> {
        let needs_init = self.ctx.is_none();

        if needs_init {
            self.ctx = None;

            let role_desc = format!(
                "あなたは現在「Knowledge Expert (知識専門家)」として動作しています。以下の役割指示に特化してください:\n{}",
                KNOWLEDGE_WORKER_PROMPT
            );
            log::debug!("=== role_desc ===\n{}", role_desc);

            let full_system_prompt = format!(
                "{}\n\n=== Current Role ===\n{}",
                SYSTEM_PROMPT,
                role_desc
            );

            let ctx = AgentContext::new(model, backend, &full_system_prompt, 2, MAX_NEW_TOKENS, N_CTX)
                .map_err(|e| format!("Failed to create Knowledge context: {:?}", e))?;
            
            let ctx_static = unsafe {
                std::mem::transmute::<AgentContext<'_>, AgentContext<'static>>(ctx)
            };
            self.ctx = Some(ctx_static);
            self.active_vendor = None;
        }
        Ok(())
    }
}

impl LlmWorker for KnowledgeWorker {
    fn agent_name(&self) -> &'static str {
        "Knowledge Expert (知識専門家)"
    }

    fn context_mut(&mut self) -> &mut AgentContext<'static> {
        self.ctx.as_mut().expect("Knowledge context not initialized")
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
        // Detect connection details from prompt or user_message
        let mut matched_connection = None;
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
                            matched_connection = Some(conn);
                            break;
                        }
                    }
                }
            }
        }

        self.ensure_initialized_with_vendor(model, backend, None)?;

        let worker_prompt = self.build_prompt(
            prompt,
            user_message,
            tool_label,
            output,
            history_block,
            subsequent_task,
        );

        let (vendor_str, device_str, gateway_str) = if let Some(conn) = &matched_connection {
            let v = conn.vendor_type.as_ref().map(|vt| vt.as_str()).unwrap_or("Unknown").to_string();
            let d = conn.device_type.as_ref().map(|dt| dt.as_str()).unwrap_or("Unknown").to_string();
            let g = format!("{} ({})", conn.hostname.as_str(), conn.ip.to_string());
            (v, d, g)
        } else {
            ("Unknown".to_string(), "Unknown".to_string(), "Unknown".to_string())
        };

        let final_prompt = format!(
            "### System Metadata\n\
             ユーザーのクエリを処理するにあたり、以下の前提条件（対象環境）を考慮してください。\n\
             - Vendor: {}\n\
             - Device: {}\n\
             - Gateway Name (IP): {}\n\n\
             ### User Query\n\
             User: {}\n\
             MIKOMAI:",
            vendor_str,
            device_str,
            gateway_str,
            worker_prompt
        );

        crate::llm::llm_manager::run_inference(
            self.context_mut(),
            model,
            &final_prompt,
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
